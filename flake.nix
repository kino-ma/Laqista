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

        rust-components = pkgs.fenix.complete.toolchain;
        rust-linux-components = pkgs.targets.x86_64-unknown-linux-gnu.complete.withComponents [ "rust-src" "rustc" ];
        rustPlatform = pkgs.makeRustPlatform { cargo = rust-components; rustc = rust-components; };

        cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);

        system-specific-pkgs = if system == "x86_64-linux" then [ pkgs.radeontop ] else [];
      in
      {
        devShell = pkgs.mkShell {
          nativeBuildInputs = with pkgs;
            [
              llvmPackages.libclang.lib
              clang
              bison
              flex
              fontforge
              makeWrapper
              pkg-config
              gnumake
              ml
              gcc
              libiconv
              autoconf
              automake
              libtool

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
            ] 
            ++ system-specific-pkgs;


          RUST_SRC_PATH = "${pkgs.fenix.complete.rust-src}/lib/rustlib/src/rust/";
          LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
        };

        packages = rec {
          mless = rustPlatform.buildRustPackage
            {
              inherit (cargoToml.package) name version;
              src = ./.;
              cargoLock.lockFile = ./Cargo.lock;
              nativeBuildInputs = with pkgs; [ libclang clang protobuf pkgs-stable.opencv pkg-config ];
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
