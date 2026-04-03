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

        # The actual package
        rust-app-menu = pkgs.rustPlatform.buildRustPackage {
          pname = "rust-app-menu";
          version = "0.1.0";
          src = ./.;

          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = with pkgs; [
            pkg-config
            wayland-scanner
          ];

          buildInputs = buildDeps ++ dlopenLibs;

          # Patch the rpath so dlopen finds wayland/vulkan at runtime
          postFixup = ''
            patchelf \
              --set-rpath "${pkgs.lib.makeLibraryPath dlopenLibs}" \
              $out/bin/rust-app-menu
          '';

          env = {
            WAYLAND_PROTOCOLS = "${pkgs.wayland-protocols}/share/wayland-protocols";
            WAYLAND_SCANNER = "${pkgs.wayland-scanner}/bin/wayland-scanner";
          };
        };
      in
      {
        # `nix build`
        packages.default = rust-app-menu;
        packages.rust-app-menu = rust-app-menu;

        # `nix run`
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
          ];
          buildInputs = buildDeps ++ dlopenLibs;
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath dlopenLibs;
          WAYLAND_PROTOCOLS = "${pkgs.wayland-protocols}/share/wayland-protocols";
          shellHook = ''
            export WAYLAND_SCANNER="${pkgs.wayland-scanner}/bin/wayland-scanner"
            echo "🚀 app-launcher dev shell ready"
            echo "   rust: $(rustc --version)"
            echo "   cargo: $(cargo --version)"
          '';
        };
      }
    );
}
