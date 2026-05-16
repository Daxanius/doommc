{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  packages = with pkgs; [
    rustc
    cargo
    pkg-config
    cmake
    rust-analyzer
    lldb
    clang
    llvmPackages.libclang
    stdenv.cc
  ];

  shellHook = ''
    export LIBCLANG_PATH=${pkgs.llvmPackages.libclang.lib}/lib
    export BINDGEN_EXTRA_CLANG_ARGS="--sysroot=${pkgs.stdenv.cc.libc.dev}"
    export CFLAGS="-std=gnu11"
  '';
}
