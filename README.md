# Tibber Prometheus Adapter

This is a little utility to get data from the Tibber power meter into Prometheus.
It uses the local Tibber HTTP API.

This project would not have been possible to implement so quickly without the great work by [SonnenladenGmbH](https://github.com/SonnenladenGmbH/tibber_local_lib).

## Setup

You need to enable the local HTTP API of your Tibber bridge.
This is described in detail at https://github.com/SonnenladenGmbH/tibber_local_lib?tab=readme-ov-file#setting-up-tibber-pulse-for-local-access.

## Usage

```
Tibber local HTTP API to Prometheus metrics bridge

Usage: tibber-prometheus-bridge [OPTIONS] --tibber-host <TIBBER_HOST> --password-file <PASSWORD_FILE>

Options:
  -t, --tibber-host <TIBBER_HOST>      Hostname of the tibber bridge
  -n, --node <NODE>                    Node ID to read [default: 1]
  -p, --password-file <PASSWORD_FILE>  The file to read the tibber host password from
  -b, --bind-address <BIND_ADDRESS>    The bind address for the metrics server [default: 127.0.0.1:8080]
  -h, --help                           Print help
  -V, --version                        Print version
```

For example, assuming you stored your admin password in a file called `tibber_pw`, you can start the bridge like this: `tibber-prometheus-bridge -p tibber_pw`.
You should then be able to access your consumption data: http://localhost:8080/metrics .

## On NixOS

This repository also provides a flake with a NixOS module for easy integration with NixOS systems.
Details need to be added for this.

