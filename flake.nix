{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    nixpkgs-stable.url = "github:nixos/nixpkgs/nixos-23.11";
    flake-utils.url = "github:numtide/flake-utils";

    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, nixpkgs-stable, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        pkgs-stable = import nixpkgs-stable { inherit system overlays; };

        cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
      in
      {
        devShell = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [ cargo rustc rustfmt rust-analyzer protobuf iconv grpcurl pkgs-stable.opencv pkg-config ];

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
