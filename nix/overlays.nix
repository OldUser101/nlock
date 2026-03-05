{
  ...
}:
let
  cargoToml = builtins.fromTOML (builtins.readFile ../Cargo.toml);
  version = cargoToml.package.version;
in
rec {
  default = nlock;

  nlock = final: prev: {
    nlock = prev.callPackage ./default.nix {
      inherit version;
    };
  };
}
