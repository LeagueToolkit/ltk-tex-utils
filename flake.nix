{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";

    systems.url = "github:nix-systems/default";
  };

  outputs = {
    nixpkgs,
    systems,
    ...
  } @ inputs: let
    eachSystem = f:
      nixpkgs.lib.genAttrs (import systems) (
        system:
          f (import nixpkgs {
            inherit system;
            overlays = [inputs.rust-overlay.overlays.default];
          })
      );

    rustToolchain = eachSystem (pkgs: (pkgs.rust-bin.stable.latest.default.override {
      extensions = ["rust-src"];
    }));
  in {
    devShells = eachSystem (pkgs: {
      default = pkgs.mkShell {
        RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
        packages =
          [
            rustToolchain.${pkgs.system}
          ]
          ++ (with pkgs; [
            rust-analyzer-unwrapped
            cargo
            cargo-insta
            cargo-hack
            bacon
          ]);
      };
    });
  };
}
