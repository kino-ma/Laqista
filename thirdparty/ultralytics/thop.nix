{ pkgs, ... }:

let
  pname = "ultralytics_thop";
  version = "2.0.0";
in
pkgs.python311Packages.buildPythonPackage rec {
  inherit pname version;
  src = pkgs.fetchPypi
    {
      inherit pname version;
      sha256 = "sha256-Se4fLDfZLi4DtAfGEOcYTcPepoqj2RJ9JpTKHqQSCIk=";
    };
  doCheck = false;
  format = "pyproject";

  propagatedBuildInputs = with pkgs.python311Packages; [ numpy torch setuptools ];
}
