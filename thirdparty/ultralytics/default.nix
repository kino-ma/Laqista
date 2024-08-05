{ pkgs, ... }:

let
  pname = "ultralytics";
  version = "8.2.73";

  ultralytics-thop = pkgs.callPackage ./thop.nix { };
  opencv = pkgs.callPackage ./opencv.nix { };
in
pkgs.python311Packages.buildPythonPackage rec {
  inherit pname version;
  src = pkgs.fetchPypi
    {
      inherit pname version;
      sha256 = "sha256-NDPcLKEKKG7bkE95Yruun3SKrr1Uc1ompbFMyaXM+N8=";
    };
  doCheck = false;
  pyproject = true;
  dontUseCmakeConfigure = true;
  dontUseCmakeBuildDir = true;
  # format = "pyproject";
  # sourceRoot = "./opencv-python-4.10.0.8/opencv";
  sourceRoot = "${src.name}/opencv-python-4.10.0.8/opencv";

  propagatedBuildInputs = with pkgs.python311Packages;
    [
      setuptools
      numpy
      matplotlib
      pillow
      pyyaml
      requests
      scipy
      torch
      torchvision
      tqdm
      psutil
      py-cpuinfo
      pandas
      seaborn

      ultralytics-thop
      opencv
    ];
}
