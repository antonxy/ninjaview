let
  pkgs = import <nixpkgs> {};
  ninja = (import ../ninja);
in pkgs.rustPlatform.buildRustPackage rec {
  pname = "ninjaview";
  version = "0.0.1";

  src = ./.;

  postInstall = ''
  cp ${ninja}/bin/ninja $out/bin
  '';
  
  cargoSha256 = "sha256-/FtpSsRSvA6g9RbYtfRIUHoyAMRnGca4WN2FrmS14Hw=";
}
