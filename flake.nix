# This file is pretty general, and you can adapt it in your project replacing
# only `name` and `description` below.
{
  description = "Tibber local HTTP API to Prometheus bridge";

  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    rust-overlay,
    ...
  }:
    flake-utils.lib.eachDefaultSystem
    (
      system: let
        overlays = [rust-overlay.overlays.default];
        pkgs = import nixpkgs {inherit system overlays;};
        rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        build_deps = [pkgs.openssl];
        build_tools = [pkgs.pkg-config];
      in {
        formatter = pkgs.alejandra;

        devShells.default = pkgs.mkShell {
          packages = [rust pkgs.cargo pkgs.rustfmt pkgs.rust-analyzer] ++ build_deps ++ build_tools;
        };

        packages = rec {
          tibber-prometheus-bridge = pkgs.rustPlatform.buildRustPackage {
            name = "tibber-prometheus-bridge";
            src = ./.;
            cargoLock = {
              lockFile = ./Cargo.lock;
            };
            buildInputs = build_deps;
            nativeBuildInputs = build_tools;
          };

          default = tibber-prometheus-bridge;
        };

        apps = rec {
          tibber-prometheus-bridge = flake-utils.lib.mkApp {drv = self.packages.${system}.tibber-prometheus-bridge;};
          default = tibber-prometheus-bridge;
        };
      }
    )
    // {
      nixosModules = rec {
        tibber-prometheus-bridge = {
          lib,
          config,
          pkgs,
          ...
        }:
          with lib; let
            cfg = config.services.tibber-prometheus-bridge;
            default_user_group = "tibber-bridge";
          in {
            options.services.tibber-prometheus-bridge = {
              enable = mkEnableOption "the Tibber Prometheus Bridge";
              bind-address = mkOption {
                type = types.str;
                default = "127.0.0.1";
                description = "The address to listen on for incoming Prometheus requests.";
              };
              bind-port = mkOption {
                type = types.port;
                default = 8018;
                description = "The port number to listen on for incoming Prometheus requests.";
              };
              tibber-host = mkOption {
                type = types.str;
                default = "tibber-host";
                description = "The upstream tibber gateway host name. You must enable the local HTTP interface in the gateway.";
              };
              tibber-admin-password-file = mkOption {
                type = types.str;
                description = "A path to a file containing the password for your local tibber host.";
              };

              enable-prometheus-scrape = mkEnableOption "local Prometheus scrape configuration for Tibber.";
              prometheus-scrape-job-name = mkOption {
                type = types.str;
                default = "tibber";
                description = "The job name for the local Prometheus scrape job.";
              };

              user = mkOption {
                type = types.str;
                default = default_user_group;
                description = "The user to run as.";
              };

              group = mkOption {
                type = types.str;
                default = default_user_group;
                description = "The group to run as";
              };
            };

            config = mkIf cfg.enable {
              users.users = mkIf (cfg.user == default_user_group) {
                "${default_user_group}" = {
                  inherit (cfg) group;
                  isSystemUser = true;
                };
              };

              users.groups = mkIf (cfg.group == default_user_group) {
                "${default_user_group}" = {};
              };

              systemd.services.tibber-prometheus-bridge = {
                enable = true;
                description = "Tibber Prometheus Bridge";
                after = ["network.target"];
                wantedBy = ["multi-user.target"];
                serviceConfig = {
                  Type = "simple";
                  User = cfg.user;
                  Group = cfg.group;
                  ExecStart = "${self.packages.${pkgs.system}.tibber-prometheus-bridge}/bin/tibber-prometheus-bridge -t ${cfg.tibber-host} -b ${cfg.bind-address}:${toString cfg.bind-port} -p ${cfg.tibber-admin-password-file}";
                  Restart = "on-failure";
                };
              };

              services.prometheus.scrapeConfigs = mkIf cfg.enable-prometheus-scrape [
                {
                  job_name = cfg.prometheus-scrape-job-name;
                  static_configs = [
                    {
                      targets = ["127.0.0.1:${toString cfg.bind-port}"];
                    }
                  ];
                }
              ];
            };
          };

        default = tibber-prometheus-bridge;
      };
    };
}
