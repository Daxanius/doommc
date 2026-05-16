{
  description = "Rust development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    {
      self,
      nixpkgs,
    }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
        packages = with pkgs; [
          cargo
          clippy
          rustc
          rustfmt
          rust-analyzer
          rustPlatform.bindgenHook
          pkg-config
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
      };
    };
}
