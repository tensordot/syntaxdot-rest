{ lib
, rustPlatform
, symlinkJoin
, cmake
, pkg-config
, libtorch
, openssl
}:

let
  version = (builtins.fromTOML (builtins.readFile ../Cargo.toml)).package.version;
in rustPlatform.buildRustPackage {
  inherit version;

  pname = "syntaxdot-rest";

  src = builtins.path {
    name = "syntaxdot-rest";
    path = ../.;
  };

  cargoLock = {
    lockFile = ../Cargo.lock;
  };

  nativeBuildInputs = [ cmake pkg-config ];

  buildInputs = [ openssl ];

  LIBTORCH = symlinkJoin {
    name = "torch-join";
    paths = [ libtorch.dev libtorch.out ];
  };

  meta = with lib; {
    description = "SyntaxDot REST server";
    homepage = "https://github.com/tensordot/syntaxdot";
    license = licenses.blueOak100;
    platforms = [ "x86_64-linux" ];
    maintainers = with maintainers; [ danieldk ];
  };
}
