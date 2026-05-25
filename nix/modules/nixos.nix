# nix/modules/nixos.nix — auto-generated from lava-api-forge.caixa.lisp
# description: "OpenAPI / AWS botocore service spec → typed tatara-lisp (defapi-operation ...) bindings. Skips the Terraform provider middleman; generates directly from cloud API specs. Output consumed by lava-provider-gen to autogenerate full tfplugin5/6-compatible magma providers."
{ config, lib, pkgs, ... }:
let
  cfg = config.services.lava-api-forge;
in {
  options.services.lava-api-forge = {
    enable = lib.mkEnableOption "lava-api-forge";
    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.lava-api-forge or null;
    };
  };
  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];
  };
}
