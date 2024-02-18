let
  pkgs = import <nixpkgs> {};
  ninja = (import ../ninja);
in pkgs.mkShell rec {
  buildInputs = [
    pkgs.llvmPackages.clang
    ninja
  ];
}
