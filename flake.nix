{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    nixpkgs-stable.url = "github:nixos/nixpkgs/nixos-23.11";
    flake-utils.url = "github:numtide/flake-utils";

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, nixpkgs-stable, flake-utils, fenix }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ fenix.overlays.default ];
        pkgs = import nixpkgs { inherit system overlays; };
        pkgs-stable = import nixpkgs-stable { inherit system overlays; };

        rust-components = pkgs.fenix.complete.withComponents [ "cargo" "rust-src" "rustc" "rustfmt" ];
        rust-linux-components = pkgs.targets.x86_64-unknown-linux-gnu.complete.withComponents [ "rust-src" "rustc" ];

        cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
      in
      {
        devShell = pkgs.mkShell {
          nativeBuildInputs = with pkgs;
            [
              rust-components
              rust-analyzer-nightly
              protobuf
              iconv
              grpcurl
              pkgs-stable.opencv
              pkg-config
              python311
              python311Packages.grpcio-tools
              jq
              gnuplot
            ];


          RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
        };

        packages = rec {
          mless = pkgs.rustPlatform.buildRustPackage
            {
              inherit (cargoToml.package) name version;
              src = ./.;
              cargoLock.lockFile = ./Cargo.lock;
              nativeBuildInputs = with pkgs; [ protobuf pkgs-stable.opencv pkg-config ];
            };

          default = mless;
        };

        apps = rec {
          mless = {
            type = "app";
            program = "${self.packages.${system}.mless}/bin/mless";
          };

          default = mless;
        };
      });
}
