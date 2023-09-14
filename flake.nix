{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rusttoolchain =
          pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        # needed to run kotlin tests
        kotlinx-coroutines = builtins.fetchurl {
          name = "kotlinx-coroutines-core-jvm.jar";
          url =
            "https://repo1.maven.org/maven2/org/jetbrains/kotlinx/kotlinx-coroutines-core-jvm/1.6.4/kotlinx-coroutines-core-jvm-1.6.4.jar";
          sha256 =
            "sha256:1gi0r3a16mb7xxx1jywh71v60q8cbrz1ll3i72lw885kgfr8nk62";
        };
        jna = pkgs.jna.overrideAttrs (finalAttrs: previousAttrs: {
          meta = previousAttrs.meta // {
            # JNA produces native jars for these darwin platforms, aarch64 has been tested by us
            platforms = previousAttrs.meta.platforms ++ [ "x86_64-darwin" "aarch64-darwin" "i686-darwin" ];
          };
        });
      in
      rec {
        # `nix develop`
        devShell = pkgs.mkShell {
          buildInputs = with pkgs;
            [
              rusttoolchain
              pkg-config
              clang
              gcc
              # used when generating kotlin bindings in core
              pkgs.ktlint
              # used when running kotlin tests
              pkgs.kotlin
              pkgs.jdk17_headless
              jna
              # used when generating python bindings in core
              pkgs.yapf
              pkgs.curl
              # used when running python tests
              pkgs.python310
              # used when running ruby tests
              ruby
              rubyPackages.ffi
              # used when running swift tests
              # TODO(murph): cannot get swift tests consistently building. fails with /nix/store/<hash>/bin/ld.gold: error: cannot open crti.o: No such file or directory
              swift
              swiftPackages.Foundation
            ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin
              [ pkgs.darwin.apple_sdk.frameworks.Security ];

          shellHook = ''
            export CLASSPATH="$CLASSPATH:${kotlinx-coroutines}";
            export C_INCLUDE_PATH="${pkgs.swift}/lib/swift/clang/include";
            export C_PLUS_INCLUDE_PATH="${pkgs.swift}/lib/swift/clang/include";
            export LIBRARY_PATH="${pkgs.lib.makeLibraryPath [pkgs.icu pkgs.libgcc pkgs.libuuid pkgs.glibc]}";
            # overrides existing `gcc`
            # export CC="clang"; already seems to be using clang without this
          '';
        };

      });
}
