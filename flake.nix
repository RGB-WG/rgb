{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
    };
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
    };

    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, crane, rust-overlay, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        commonPackages = with pkgs; [
            openssl
            pkg-config
        ];
        workspaceToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
        craneLib = (crane.mkLib pkgs).overrideToolchain pkgs.rust-bin.stable.latest.default;

        stableWithLlvm = pkgs.rust-bin.nightly.latest.default.override {
          extensions = [ "rustfmt" "llvm-tools-preview" ];
          targets = [ ];
        };
      in
      with pkgs;
      {
        packages = rec {
          default = rgb;
          rgb = craneLib.buildPackage rec {
            pname = "rgb";
            cargoToml = ./Cargo.toml;
            nativeBuildInputs = commonPackages;
            cargoExtraArgs = "-p rgb-wallet";
            outputHashes = {};
            src = self;
            buildInputs = with pkgs; [
              openssl
            ];
            cargoArtifacts = craneLib.buildDepsOnly {
              inherit src cargoToml buildInputs nativeBuildInputs cargoExtraArgs outputHashes;
            };
            strictDeps = true;
            doCheck = false;
          };
        };

        devShells = rec {
          default = msrv;

          msrv = mkShell {
            buildInputs = commonPackages ++ [
              rust-bin.stable."${workspaceToml.workspace.package."rust-version"}".default
            ];
          };

          stable = mkShell {
            buildInputs = commonPackages ++ [
              rust-bin.stable.latest.default
            ];
          };

          beta = mkShell {
            buildInputs = commonPackages ++ [
              rust-bin.beta.latest.default
            ];
          };

          nightly = mkShell {
            buildInputs = commonPackages ++ [
              rust-bin.nightly.latest.default
            ];
          };

          codecov = mkShell {
            buildInputs = commonPackages ++ [
              stableWithLlvm
            ];
            CARGO_INCREMENTAL = "0";
            RUSTFLAGS = "-Cinstrument-coverage";
            RUSTDOCFLAGS = "-Cinstrument-coverage";
          };
        };
      }
    );
}
