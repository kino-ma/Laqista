{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    nix-gl = {
      url = "github:nix-community/nixGL";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, fenix, nix-gl }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ fenix.overlays.default nix-gl.overlay ];
        pkgs = import nixpkgs {
          inherit system overlays;
          config.permittedInsecurePackages = [ "openssl-1.1.1w" ];
        };

        rust-components = with pkgs.fenix; combine [
          default.rustc
          default.cargo
          default.rust-std
          default.rust-docs
          default.rustfmt-preview
          default.clippy-preview
          latest.rust-src
          targets.wasm32-unknown-unknown.latest.rust-std
        ];
        rustPlatform = pkgs.makeRustPlatform {
          cargo = rust-components;
          rustc = rust-components;
        };

        wonnx = pkgs.callPackage ./thirdparty/wonnx/default.nix { };

        cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);

      in
      {
        devShell = pkgs.mkShell rec {
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
              gcc
              libiconv
              autoconf
              automake
              libtool
              lld

              rust-components
              rust-analyzer-nightly
              protobuf
              iconv
              grpcurl
              pkg-config
              python311
              poetry
              python311Packages.cmake
              python311Packages.grpcio-tools
              python311Packages.onnx
              python311Packages.onnxruntime
              jq
              gnuplot
              ghz
              k6

              cargo-generate
              wasm-pack
              wonnx
            ]
            ++ pkgs.lib.optionals (system == "x86_64-linux") [
              pkgs.radeontop
              pkgs.openssl
              pkgs.vulkan-loader
              pkgs.libdrm

              libdrm
              xorg.libxcb
              expat
              zstd
              xorg.libxshmfence
              pkgs.stdenv.cc.cc.lib
              libGLU
              llvmPackages_12.libllvm
              mesa

              pciutils
              vulkan-tools
              glib

              pkgs.nixgl.auto.nixGLDefault
            ]
            ++ pkgs.lib.optionals (pkgs.stdenv.isDarwin) (with pkgs; with darwin.apple_sdk.frameworks; [
              llvmPackages.libcxxStdenv
              llvmPackages.libcxxClang
              llvmPackages.libcxx
              darwin.libobjc
              darwin.libiconv
              libiconv
              Security
              SystemConfiguration
              AppKit
              WebKit
              CoreFoundation
            ]);


          RUST_SRC_PATH = "${pkgs.fenix.complete.rust-src}/lib/rustlib/src/rust/";
          LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
          LD_LIBRARY_PATH = (pkgs.lib.makeLibraryPath (nativeBuildInputs ++ [ pkgs.stdenv.cc.cc pkgs.libclang pkgs.vulkan-loader pkgs.openssl ])) + ":/home/kino-ma/lib";
        };

        packages = rec {
          laqista = rustPlatform.buildRustPackage
            {
              inherit (cargoToml.package) name version;
              src = ./.;

              cargoLock.lockFile = ./Cargo.lock;

              # Inputs for both of build&runtime environment
              nativeBuildInputs = with pkgs; [ libclang libclang.lib clang protobuf pkg-config ];
              buildInputs = with pkgs; [ stdenv.cc.cc stdenv.cc.cc.lib lld ];

              RUST_SRC_PATH = "${pkgs.fenix.complete.rust-src}/lib/rustlib/src/rust/";
              PROTOC = "${pkgs.protobuf}/bin/protoc";
              LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
              LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [ pkgs.stdenv.cc.cc ] ++ (pkgs.lib.optionals (system == "x86_64-linux") [ pkgs.openssl ]);
              CLANG_PATH = "${pkgs.clang}/bin/clang";
            };

          default = laqista;
        };

        apps = rec {
          laqista = {
            type = "app";
            program = "${self.packages.${system}.laqista}/bin/laqista";
          };

          default = laqista;
        };
      });
}
