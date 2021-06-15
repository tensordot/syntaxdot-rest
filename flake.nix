{
  description = "SyntaxDot REST server";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, utils }:
    utils.lib.eachSystem  [ "x86_64-linux" ] (system:
    let
      allowLibTorch = pkgs: pkg: builtins.elem (pkgs.lib.getName pkg) [
        "libtorch"
      ];
      pkgs = import nixpkgs {
        inherit system;
        config = {
          allowUnfreePredicate = allowLibTorch pkgs;
        };
      };
    in {
      defaultPackage = self.packages.${system}.syntaxdot-rest;

      devShell = with pkgs; mkShell (models // {
        nativeBuildInputs = [ cmake pkg-config rustup ];

        buildInputs = [ openssl ];

        LIBTORCH = symlinkJoin {
          name = "torch-join";
          paths = [ libtorch-bin.dev libtorch-bin.out ];
        };
      });

      packages = {
        syntaxdot-rest = pkgs.callPackage nix/syntaxdot-rest.nix {
          libtorch = pkgs.libtorch-bin;
        };
      };
    }
  );
}
