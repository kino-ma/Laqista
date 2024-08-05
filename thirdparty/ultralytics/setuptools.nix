{ stdenv
, lib
, buildPythonPackage
, fetchFromGitHub
, python
, wheel
,
}:

buildPythonPackage rec {
  pname = "setuptools";
  version = "59.2.0";
  format = "pyproject";

  src = fetchFromGitHub {
    owner = "pypa";
    repo = "setuptools";
    rev = "refs/tags/v${version}";
    hash = "sha256-X0ntFlDIhUjxtWzz0LxybQSuxhRpHlMeBYtOGwqDl4A=";
  };

  nativeBuildInputs = [ wheel ];

  preBuild = lib.optionalString (!stdenv.hostPlatform.isWindows) ''
    export SETUPTOOLS_INSTALL_WINDOWS_SPECIFIC_FILES=0
  '';

  # Requires pytest, causing infinite recursion.
  doCheck = false;
}
