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

      shortRev = self.shortRev or self.dirtyShortRev or "unknown";
      overlays = import ./nix/overlays.nix { inherit shortRev; };

      pkgsFor = eachSystem (
        system:
        import nixpkgs {
          inherit system;
          overlays = [ overlays.nlock ];
        }
      );

      nixosModule = import ./nix/nixos-module.nix self;
      hmModule = import ./nix/hm-module.nix self;
    in
    {
      inherit overlays;

      packages = eachSystem (system: {
        default = self.packages.${system}.nlock;
        inherit (pkgsFor.${system}) nlock;
      });

      nixosModules = {
        default = self.nixosModules.nlock;
        nlock = nixosModule;
      };

      homeManagerModules = {
        default = self.homeManagerModules.nlock;
        nlock = hmModule;
      };

      devShells = eachSystem (system: {
        default =
          with pkgsFor.${system};
          mkShell {
            buildInputs = [
              cargo
              rustc
              rustfmt
              rust-analyzer
              pre-commit
              rustPackages.clippy

              cairo
              clang
              gdk-pixbuf
              glib
              libxkbcommon
              pam
              pango
              pkg-config
            ];

            RUST_SRC_PATH = rustPlatform.rustLibSrc;
            LIBCLANG_PATH = "${clang.cc.lib}/lib";
          };
      });
    };
}
