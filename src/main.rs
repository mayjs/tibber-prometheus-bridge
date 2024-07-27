use std::fmt::Write;

use axum::{
    extract::State,
    http::StatusCode,
    routing::get,
    Router,
};
use error_chain::error_chain;
use rust_decimal::prelude::*;
use sml_rs::parser::common::{ListEntry, Value};
use sml_rs::parser::complete::{GetListResponse, Message, MessageBody};
use sml_rs::parser::ParseError;
use sml_rs::transport::DecodeErr;
use clap::Parser;
use std::sync::Arc;

error_chain! {
    foreign_links {
        Io(std::io::Error);
        HttpRequest(reqwest::Error);
        SmlParseError(ParseError);
        SmlDecodeError(DecodeErr);
    }

    errors {
        NoConsumptionData {
            description("Could not find consumption data in response")
        }
    }
}

struct TibberHostCfg {
    tibber_host: String,
    tibber_password: String,
    tibber_node: u32,
}

async fn get_raw_tibber_data(tibber_cfg: &TibberHostCfg) -> Result<Vec<u8>> {
    let url = format!("http://{}/data.json?node_id={}", tibber_cfg.tibber_host, tibber_cfg.tibber_node);
    let username = "admin";

    let client = reqwest::Client::new();
    let req = client.get(url).basic_auth(username, Some(tibber_cfg.tibber_password.clone()));
    let res = req.send().await?;

    let data = res.bytes().await?;

    Ok(data.to_vec())
}

#[derive(Debug, Clone)]
struct ConsumptionData {
    current_power_w: Decimal,
    total_consumption_wh: Decimal,
}

fn find_by_obis<'a>(list: &'a GetListResponse<'a>, obis: &[u8]) -> Option<&'a ListEntry<'a>> {
    list.val_list.iter().find(|e| e.obj_name == obis)
}

fn sml_value_to_decimal(val: &Value) -> Option<Decimal> {
    match val {
        Value::Bool(_) => None,
        Value::Bytes(_) => None,
        Value::List(_) => None,
        Value::I8(v) => Some(Decimal::from(*v)),
        Value::I16(v) => Some(Decimal::from(*v)),
        Value::I32(v) => Some(Decimal::from(*v)),
        Value::I64(v) => Some(Decimal::from(*v)),
        Value::U8(v) => Some(Decimal::from(*v)),
        Value::U16(v) => Some(Decimal::from(*v)),
        Value::U32(v) => Some(Decimal::from(*v)),
        Value::U64(v) => Some(Decimal::from(*v)),
    }
}

fn get_scaled_value(entry: &ListEntry) -> Option<Decimal> {
    sml_value_to_decimal(&entry.value)
        .map(|v| v * Decimal::TEN.powi(entry.scaler.unwrap_or(0i8) as i64))
}

// See https://wiki.volkszaehler.org/software/obis for relevant IDs
fn get_current_power_in_watts(r: &GetListResponse) -> Option<Decimal> {
    get_scaled_value(find_by_obis(r, &[1, 0, 16, 7, 0, 255])?)
}

fn get_total_consumption_in_watt_hours(r: &GetListResponse) -> Option<Decimal> {
    get_scaled_value(find_by_obis(r, &[1, 0, 1, 8, 0, 255])?)
}

fn get_consumption_data(msg: &Message) -> Option<ConsumptionData> {
    match &msg.message_body {
        MessageBody::GetListResponse(r) => {
            let current_power = get_current_power_in_watts(&r)?;
            let total_consumption = get_total_consumption_in_watt_hours(&r)?;
            Some(ConsumptionData {
                current_power_w: current_power,
                total_consumption_wh: total_consumption,
            })
        }
        _ => None,
    }
}

async fn fetch_consumption_data(tibber_cfg: &TibberHostCfg) -> Result<ConsumptionData> {
    let data = get_raw_tibber_data(tibber_cfg).await?;

    for r in sml_rs::transport::decode(&data) {
        let decoded = r?;
        let f = sml_rs::parser::complete::parse(&decoded)?;
        let consumption_data = f.messages.iter().find_map(|msg| get_consumption_data(msg));
        if let Some(c) = consumption_data {
            return Ok(c);
        }
    }
    Err(ErrorKind::NoConsumptionData.into())
}

#[derive(Debug, Clone, Copy)]
enum MetricKind {
    Gauge,
    Counter,
}

impl MetricKind {
    fn as_str(&self) -> &'static str {
        match self {
            MetricKind::Gauge => "gauge",
            MetricKind::Counter => "counter",
        }
    }
}

struct MetricsWriter<T> {
    writer: T,
    current_metric: Option<String>,
}

impl<T> MetricsWriter<T>
where
    T: Write,
{
    fn new(writer: T) -> Self {
        Self {
            writer,
            current_metric: None,
        }
    }

    fn start_metric(&mut self, name: String, help: &str, kind: MetricKind) -> std::result::Result<(), std::fmt::Error> {
        if self.current_metric.is_some() {
            writeln!(self.writer, "")?;
        }
        writeln!(self.writer, "# HELP {} {}", name, help)?;
        writeln!(self.writer, "# TYPE {} {}", name, kind.as_str())?;

        self.current_metric = Some(name);

        Ok(())
    }

    fn write_value(&mut self, value: Decimal, labels: &Vec<(String, String)>) -> std::result::Result<(), std::fmt::Error> {
        let label_str = labels.iter().map(|(label, value)| format!("{}=\"{}\"", label, value)).reduce(|mut acc, s| {
            acc.push(',');
            acc.push_str(&s);
            acc
        }).unwrap_or_default();
        writeln!(self.writer, "{} {{{}}} {}", self.current_metric.as_ref().unwrap(), label_str, value)?;
        Ok(())
    }

    fn finalize(self) -> T {
        self.writer
    }
}

struct AppState {
    tibber_cfg: TibberHostCfg,
}

async fn metrics(State(state): State<Arc<AppState>>) -> (StatusCode, String) {
    let fetch_result = fetch_consumption_data(&state.tibber_cfg).await;

    match fetch_result {
        Ok(data) => {
            let mut metrics_writer = MetricsWriter::new(String::new());
            metrics_writer.start_metric("smartmeter_consumption_wh_total".to_string(), "The total consumption value in Wh", MetricKind::Counter).unwrap();
            metrics_writer.write_value(data.total_consumption_wh, &Vec::new()).unwrap();

            metrics_writer.start_metric("smartmeter_power_w".to_string(), "The current power in W", MetricKind::Gauge).unwrap();
            metrics_writer.write_value(data.current_power_w, &Vec::new()).unwrap();

            let metrics = metrics_writer.finalize();

            (StatusCode::OK, metrics)
        },
        Err(err) => {
            (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
        },
    }
}

/// Tibber local HTTP API to Prometheus metrics bridge
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Hostname of the tibber bridge
    #[arg(short, long)]
    tibber_host: String,

    /// Node ID to read
    #[arg(short, long, default_value_t = 1)]
    node: u32,

    /// The file to read the tibber host password from
    #[arg(short, long)]
    password_file: String,

    /// The bind address for the metrics server
    #[arg(short, long, default_value = "127.0.0.1:8080")]
    bind_address: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let password = std::fs::read_to_string(args.password_file).expect("Could not read password file");
    let tibber_cfg = TibberHostCfg { tibber_host: args.tibber_host.clone(), tibber_node: args.node, tibber_password: password.trim().to_owned()};
    let state = Arc::new(AppState { tibber_cfg });

    let app = Router::new().route("/metrics", get(metrics)).with_state(state);

    let listener = tokio::net::TcpListener::bind(args.bind_address).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

