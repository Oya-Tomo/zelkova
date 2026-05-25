{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rustToolchain = pkgs.rust-bin.stable.latest.default;
      in
      {
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            rustToolchain
            cargo-watch
            pkg-config

            # X11
            xorg.libX11
            xorg.libXcursor
            xorg.libXrandr
            xorg.libXi
            xorg.libxcb

            # Keyboard
            libxkbcommon

            # Font
            fontconfig
            freetype

            # Compression/crypto
            zlib
            openssl

            # Vulkan (for blade-graphics)
            vulkan-loader
            vulkan-headers

            # Wayland (optional but included for future use)
            wayland
          ];

          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (with pkgs; [
            xorg.libX11
            xorg.libXcursor
            xorg.libXi
            xorg.libxcb
            libxkbcommon
            fontconfig
            freetype
            zlib
            vulkan-loader
            wayland
          ]);

          # Allow cargo to find fontconfig etc.
          FONTCONFIG_FILE = pkgs.makeFontsConf { fontDirectories = [ pkgs.dejavu_fonts ]; };
        };
      }
    );
}
