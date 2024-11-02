{ pkgs, ... }:
pkgs.rustPlatform.buildRustPackage
rec {
  pname = "wonnx";
  version = "v0.5.1";

  doCheck = false;

  nativeBuildInputs = with pkgs; [ pkg-config openssl_1_1 ];
  buildInputs = [ ] ++ pkgs.lib.optionals (pkgs.stdenv.isDarwin) (with pkgs; with darwin.apple_sdk.frameworks; [
    llvmPackages.libcxxStdenv
    llvmPackages.libcxxClang
    llvmPackages.libcxx
    libiconv
    vulkan-loader
  ])
  ++ pkgs.lib.optionals (pkgs.stdenv.isDarwin) (with pkgs; with darwin.apple_sdk.frameworks; [
    darwin.libobjc
    darwin.libiconv
    Security
    SystemConfiguration
    AppKit
    WebKit
    CoreFoundation
  ]);

  # OPENSSL_NO_VENDOR = "1";
  OPENSSL_LIB_DIR = "${pkgs.openssl_1_1.out}/lib";
  OPENSSL_INCLUDE_DIR = "${pkgs.openssl_1_1.dev}/include";

  src = pkgs.fetchFromGitHub {
    owner = "webonnx";
    repo = pname;
    rev = version;
    hash = "sha256-1h9Sif7eDTouwFssEN8bPxFLGMakXLm0nM75tN2nnJ4=";
  };

  cargoHash = "sha256-tQ0mREfUG3gY+nPNg15BJB6SrvnP7cqCd4OZJvhyH1M=";
}
