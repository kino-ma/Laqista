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

        system-specific-pkgs = if system == "x86_64-linux" then [ pkgs.radeontop ] else [ ];
        staticOpencv = pkgs.callPackage ./thirdparty/opencv { };
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
              staticOpencv
              pkg-config
              python311
              python311Packages.grpcio-tools
              jq
              gnuplot
            ]
            ++ system-specific-pkgs;


          RUST_SRC_PATH = "${pkgs.fenix.complete.rust-src}/lib/rustlib/src/rust/";
          LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [ pkgs.stdenv.cc.cc pkgs.libclang staticOpencv ];
        };

        packages = rec {
          mless = rustPlatform.buildRustPackage
            {
              inherit (cargoToml.package) name version;
              src = ./.;

              cargoLock.lockFile = ./Cargo.lock;

              # Inputs for both of build&runtime environment
              nativeBuildInputs = with pkgs; [ zlib ocl-icd pcre boost gflags protobuf_21 libclang libclang.lib clang protobuf staticOpencv pkg-config ];
              buildInputs = with pkgs; [ stdenv.cc.cc staticOpencv stdenv.cc.cc.lib lld zlib ocl-icd ];

              RUST_SRC_PATH = "${pkgs.fenix.complete.rust-src}/lib/rustlib/src/rust/";
              PROTOC = "${pkgs.protobuf}/bin/protoc";
              LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
              LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath ([ pkgs.stdenv.cc.cc ] ++ (with pkgs; [ zlib ocl-icd pcre boost gflags protobuf_21 libclang libclang.lib clang protobuf staticOpencv pkg-config ]));
              CLANG_PATH = "${pkgs.clang}/bin/clang";
              OPENCV_INCLUDE_PATHS = "${staticOpencv}/include/opencv4";
              OPENCV_LINK_PATHS = "${staticOpencv}/lib";
              OPENCV_LINK_LIBS = "+opencv_face";
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
