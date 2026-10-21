{
  description = "kartenn: declarative network reference with blocking consistency checks";

  # One nixpkgs for devenv and the flake (spec 015/E1, ADR 0029): flake.lock pins the
  # commit of devenv.lock, and checks.lock-sync fails when the two drift apart.
  inputs.nixpkgs.url = "github:cachix/devenv-nixpkgs/rolling";

  outputs = {
    self,
    nixpkgs,
  }: let
    systems = ["x86_64-linux" "aarch64-linux"];
    forAll = f:
      builtins.listToAttrs (map (system: {
          name = system;
          value = f nixpkgs.legacyPackages.${system};
        })
        systems);
  in {
    packages = forAll (pkgs: rec {
      kartenn-server = pkgs.callPackage ./nix/package/server.nix {};
      default = kartenn-server;
    });

    checks = forAll (pkgs: {
      lock-sync = pkgs.callPackage ./nix/checks/lock-sync.nix {};
    });
  };
}
