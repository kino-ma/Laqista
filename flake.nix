{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";

    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
      in
      {
        devShell = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [ cargo rustc rust-analyzer ];

          RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
        };

        packages = rec {
          mless = pkgs.rustPlatform.buildRustPackage
            {
              inherit (cargoToml.package) name version;
              src = ./.;
              cargoLock.lockFile = ./Cargo.lock;
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
