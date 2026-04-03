{
  description = "App launcher — Rust + iced + iced_layershell";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };
  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [
            "rust-src"
            "rust-analyzer"
            "clippy"
            "rustfmt"
          ];
        };
        dlopenLibs = with pkgs; [
          libxkbcommon
          vulkan-loader
          wayland
        ];
        buildDeps = with pkgs; [
          wayland
          pkg-config
          openssl
        ];
        src = pkgs.lib.cleanSourceWith {
          src = ./.;
          filter =
            path: type:
            let
              baseName = builtins.baseNameOf path;
            in
            pkgs.lib.cleanSourceFilter path type && baseName != "target" && baseName != "result";
        };
        rust-app-menu = pkgs.rustPlatform.buildRustPackage {
          pname = "rust-app-menu";
          version = "0.1.0";
          inherit src;
          cargoLock.lockFile = ./Cargo.lock;
          nativeBuildInputs = with pkgs; [
            pkg-config
            wayland-scanner
            makeWrapper
          ];
          buildInputs = buildDeps ++ dlopenLibs;
          postFixup = ''
            wrapProgram $out/bin/rust-app-menu \
              --set LD_LIBRARY_PATH "${pkgs.lib.makeLibraryPath dlopenLibs}"
          '';
          env = {
            WAYLAND_PROTOCOLS = "${pkgs.wayland-protocols}/share/wayland-protocols";
            WAYLAND_SCANNER = "${pkgs.wayland-scanner}/bin/wayland-scanner";
          };
        };
      in
      {
        packages.default = rust-app-menu;
        packages.rust-app-menu = rust-app-menu;
        apps.default = {
          type = "app";
          program = "${rust-app-menu}/bin/rust-app-menu";
        };
        devShells.default = pkgs.mkShell {
          name = "app-launcher-dev";
          nativeBuildInputs = with pkgs; [
            rustToolchain
            pkg-config
            cargo-watch
            cargo-expand
            sccache
            mold
          ];
          buildInputs = buildDeps ++ dlopenLibs;
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath dlopenLibs;
          WAYLAND_PROTOCOLS = "${pkgs.wayland-protocols}/share/wayland-protocols";
          RUSTFLAGS = "-C link-arg=-fuse-ld=mold";
          shellHook = ''
            export WAYLAND_SCANNER="${pkgs.wayland-scanner}/bin/wayland-scanner"
            export RUSTC_WRAPPER="${pkgs.sccache}/bin/sccache"
            export SCCACHE_CACHE_SIZE="10G"
            echo "🚀 app-launcher dev shell ready"
            echo "   rust: $(rustc --version)"
            echo "   cargo: $(cargo --version)"
          '';
        };
      }
    );
}
