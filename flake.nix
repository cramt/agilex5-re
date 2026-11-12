{
  description = "agilex5-re: open bitstream database and tools for Altera Agilex 5 (DE25-Nano)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    { nixpkgs, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in
      {
        packages.db = pkgs.callPackage ./nix/db.nix { };
      }
    );
}
