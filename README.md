# Tibber Prometheus Bridge

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

## Integrating with NixOS

This repository also provides a flake with a NixOS module for easy integration with NixOS systems.

If you configure your system using flakes, you can enable the bridge in your `flake.nix` like this:

```nix
inputs.tibber-prometheus-bridge.url = "github:mayjs/tibber-prometheus-bridge";

outputs = { tibber-prometheus-bridge } : {
  nixosConfigurations.yourSystem = nixpkgs.lib.nixosSystem {
    tibber-prometheus-bridge.nixosModules.default
    ({config, ... }: {
      age.secrets.tibber_pw = {
        file = ./secrets/tibber_pw.age;
        owner = config.services.tibber-prometheus-bridge.user;
      };

      services.tibber-prometheus-bridge = {
        enable = true; # Enable the bridge service itself
        enable-prometheus-scrape = true; # Enable the Prometheus scrape config to locally scrape the Prometheus data
        tibber-admin-password-file = config.age.secrets.tibber_pw.path;
      };
    })
  };
};
```

This assumes that you use [agenix](https://github.com/ryantm/agenix) to manage your secrets.
If you don't use `agenix`, you can substitute your preferred way to get the path to your Tibber admin password in `tibber-admin-password-file`.

## Visualizing in Grafana

You can find an example configuration for a Grafana dashboard in [example_grafana_dashboard.json](./example_grafana_dashboard.json).

