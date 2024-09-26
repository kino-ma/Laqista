{ pkgs, ... }:

let
  pname = "onnxsim";
  version = "0.4.36";
in

pkgs.python310Packages.buildPythonPackage rec {
  inherit pname version;
  # src = pkgs.fetchPypi {
  #   inherit pname version;
  #   sha256 = "sha256-bg7p1tSoMEK973MZ++WDUtn9pfJTOGvismfHwn8GOO4=";
  # };
  src = pkgs.fetchFromGitHub {
    owner = "daquexian";
    repo = "onnx-simplifier";
    rev = "fbf1ca8e26ba29200f6572194391b148c0695254";
    hash = "sha256-rN7fA46Jd0l1oQH2mb37SxtEWDq+yqhcxQfX7eDUog0=";
    fetchSubmodules = true;
  };

  nativeBuildInputs = builtins.trace src (with pkgs; [
    cmake
    stdenv
  ]);

  doCheck = false;
}
