# nix/modules/home-manager.nix — auto-generated from lava-api-forge.caixa.lisp
{ config, lib, pkgs, ... }:
let cfg = config.programs.lava-api-forge; in {
  options.programs.lava-api-forge = {
    enable = lib.mkEnableOption "lava-api-forge";
    package = lib.mkOption { type = lib.types.package; default = pkgs.lava-api-forge or null; };
  };
  config = lib.mkIf cfg.enable { home.packages = [ cfg.package ]; };
}
