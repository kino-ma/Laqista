{ pkgs, ... }:

let
  pname = "opencv-python";
  version = "4.6.0.66";

  # opencv-python requires setuptools==59.2.0, so hand-craft
  setuptools = pkgs.callPackage ./setuptools.nix (with pkgs.python311Packages; {
    inherit buildPythonPackage wheel;
  });
in
pkgs.python311Packages.buildPythonPackage rec {
  inherit pname version;
  src = pkgs.fetchPypi
    {
      inherit pname version;
      sha256 = "sha256-xb+uQa1AMeZrsQ7EoKL/0+UU0JJlJ4HosayY0bWfEVg=";
    };
  doCheck = false;
  sourceRoot = "./opencv-python-4.6.0.66/opencv";

  nativeBuildInputs = with pkgs;
    [ cmake ];
  propagatedBuildInputs = with pkgs.python311Packages;
    [
      cmake
      numpy
      pip
      scikit-build
      setuptools
    ];
}
