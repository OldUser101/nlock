{
  description = "Customisable, minimalist screen locker for Wayland";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs =
    {
      self,
      nixpkgs,
    }:
    let
      inherit (nixpkgs) lib;
      systems = [
        "aarch64-linux"
        "x86_64-linux"
      ];
      eachSystem = lib.genAttrs systems;

      pkgsFor = eachSystem (
        system:
        import nixpkgs {
          inherit system;
          overlays = with self.overlays; [ nlock ];
        }
      );
    in
    {
      overlays = import ./nix/overlays.nix { };

      packages = eachSystem (system: {
        default = self.packages.${system}.nlock;
        inherit (pkgsFor.${system}) nlock;
      });

      devShells = eachSystem (system: {
        default =
          with pkgsFor.${system};
          mkShell {
            buildInputs = [
              cargo
              rustc
              rustfmt
              pre-commit
              rustPackages.clippy

              cairo
              clang
              gdk-pixbuf
              glib
              libxkbcommon
              pam
              pkg-config
            ];

            RUST_SRC_PATH = rustPlatform.rustLibSrc;
            LIBCLANG_PATH = "${clang.cc.lib}/lib";
          };
      });
    };
}
