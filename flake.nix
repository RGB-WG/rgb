{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
    };

    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, rust-overlay, nixpkgs, flake-utils }:
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
        cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
      in
      with pkgs;
      {
        devShells = rec {
          default = msrv;

          msrv = mkShell {
            buildInputs = commonPackages ++ [
              rust-bin.stable."${cargoToml.workspace.package."rust-version"}".default
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
        };
      }
    );
}
