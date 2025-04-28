{
  description = "omzet";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
      ...
    }:

    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
      in
      {

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            pkg-config
            ffmpeg-full
            clang
            sqlite
          ];

          packages = with pkgs; [
            pkg-config
            (rust-bin.stable.latest.default.override {
              targets = [
                "x86_64-unknown-linux-gnu"
                "x86_64-unknown-linux-musl"
              ];
            })
          ];

          PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib"; # Usually not needed if clang is in nativeBuildInputs
        };
      }
    );

}
