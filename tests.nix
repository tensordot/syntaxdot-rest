{
  pkgs ? import (import ./nix/sources.nix).nixpkgs {
    config = {
      allowUnfreePredicate = pkg: builtins.elem (pkgs.lib.getName pkg) [
        "libtorch"
      ];
    };
  }
}:

let
  sources = import ./nix/sources.nix;
  sourceByRegex = pkgs.callPackage nix/source-by-regex.nix {};
  crateOverrides = with pkgs; defaultCrateOverrides // {
    hdf5-sys = attr: {
      HDF5_DIR = symlinkJoin { name = "hdf5-join"; paths = [ hdf5.dev hdf5.out ]; };
    };

    sticker2 = attr: {
      buildInputs = [ libtorch-bin ] ++
        lib.optional stdenv.isDarwin darwin.Security;
    };

    sticker2-rest = attr: {
      buildInputs = [ libtorch-bin ] ++
        lib.optional stdenv.isDarwin darwin.Security;
    };

    sentencepiece-sys = attr: {
      nativeBuildInputs = [ pkgconfig ];

      buildInputs = [ sentencepiece ];
    };

    torch-sys = attr: {
      buildInputs = lib.optional stdenv.isDarwin curl;

      LIBTORCH = "${libtorch-bin.dev}";
    };
  };
  buildRustCrate = pkgs.buildRustCrate.override {
    defaultCrateOverrides = crateOverrides;
  };
  crateTools = pkgs.callPackage "${sources.crate2nix}/tools.nix" {};
  cargoNix = pkgs.callPackage (crateTools.generatedCargoNix {
    name = "sticker2-rest";
    src = sourceByRegex ./. [
      "^Cargo\.toml$"
      "^Cargo\.lock$"
      ".*/[a-z_]+\.rs"
    ];
  }) {
    inherit buildRustCrate;
  };
in cargoNix.rootCrate.build
