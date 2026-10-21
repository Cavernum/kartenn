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
      kartenn-web = pkgs.callPackage ./nix/package/web.nix {};
      kartenn = pkgs.callPackage ./nix/package {inherit kartenn-server kartenn-web;};
      default = kartenn;
      oci = pkgs.callPackage ./nix/oci.nix {inherit kartenn;};
    });

    checks = forAll (pkgs: let
      packages = self.packages.${pkgs.stdenv.hostPlatform.system};
    in {
      lock-sync = pkgs.callPackage ./nix/checks/lock-sync.nix {};
      version = pkgs.callPackage ./nix/checks/version.nix {
        inherit (packages) kartenn kartenn-server kartenn-web;
      };
      oci = pkgs.callPackage ./nix/checks/oci.nix {inherit (packages) oci;};
      # A virtual machine: needs KVM; `nix flake check` runs it, not k-check.
      vm = pkgs.callPackage ./nix/checks/vm.nix {module = self.nixosModules.default;};
    });

    nixosModules.default = import ./nix/module.nix self;
  };
}
