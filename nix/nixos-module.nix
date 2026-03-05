self:
{
  pkgs,
  config,
  lib,
  ...
}:

with lib;
let
  cfg = config.programs.nlock;
in
{
  _class = "nixos";

  options.programs.nlock = {
    enable = mkEnableOption "nlock";

    package = mkOption {
      type = types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.nlock;
      description = "The nlock package to use";
    };

    settings = {
      general = {
        allowEmptyPassword = mkOption {
          type = types.bool;
          default = false;
          description = "Allow a blank password to be validated";
        };

        hideCursor = mkOption {
          type = types.bool;
          default = true;
          description = "Whether to hide the mouse cursor when locked";
        };

        backgroundType = mkOption {
          type = types.enum [
            "color"
            "image"
          ];
          default = "color";
          description = "Either color or image background type";
        };
      };

      colors = {
        background = mkOption {
          type = types.str;
          default = "000000FF";
          description = "Background color";
        };

        text = mkOption {
          type = types.str;
          default = "FFFFFFFF";
          description = "Text color";
        };

        inputBackground = mkOption {
          type = types.str;
          default = "000000FF";
          description = "Input box background color";
        };

        inputBorder = mkOption {
          type = types.str;
          default = "000000FF";
          description = "Input box border color";
        };

        frameBorderIdle = mkOption {
          type = types.str;
          default = "00000000";
          description = "Frame border idle color";
        };

        frameBorderSuccess = mkOption {
          type = types.str;
          default = "00000000";
          description = "Frame border success color";
        };

        frameBorderFail = mkOption {
          type = types.str;
          default = "FF0000FF";
          description = "Frame border fail color";
        };
      };

      font = {
        size = mkOption {
          type = types.float;
          default = 72.0;
          description = "Font size, in points";
        };

        useDpiScaling = mkOption {
          type = types.bool;
          default = false;
          description = "Whether to scale font size based on output DPI";
        };

        family = mkOption {
          type = types.nullOr types.str;
          default = null;
          description = "Font family";
        };

        slant = mkOption {
          type = types.enum [
            "normal"
            "italic"
            "oblique"
          ];
          default = "normal";
          description = "Font slant";
        };

        weight = mkOption {
          type = types.enum [
            "normal"
            "bold"
          ];
          default = "normal";
          description = "Font weight";
        };
      };

      input = {
        maskChar = mkOption {
          type = types.str;
          default = "*";
          description = "Character displayed in place of password characters";
        };

        width = mkOption {
          type = types.float;
          default = 0.5;
          description = "Width of the input box, relative to display width";
        };

        paddingX = mkOption {
          type = types.float;
          default = 0.05;
          description = "Input box horizontal padding, relative to display width";
        };

        paddingY = mkOption {
          type = types.float;
          default = 0.05;
          description = "Input box vertical padding, relative to display width";
        };

        radius = mkOption {
          type = types.float;
          default = 0.0;
          description = "Radius of input box corners, relative to total box height";
        };

        border = mkOption {
          type = types.float;
          default = 0.0;
          description = "Width of input box border, absolute, typically pixels";
        };

        hideWhenEmpty = mkOption {
          type = types.bool;
          default = false;
          description = "Whether to hide the input box if password is empty";
        };

        fitToContent = mkOption {
          type = types.bool;
          default = false;
          description = "Whether to resize input box to fit password, up to width";
        };
      };

      frame = {
        border = mkOption {
          type = types.float;
          default = 25.0;
          description = "Width of frame border, absolute, typically pixels";
        };

        radius = mkOption {
          type = types.float;
          default = 0.0;
          description = "Radius of frame border, absolute, typically pixels";
        };
      };

      image = {
        path = mkOption {
          type = types.nullOr types.externalPath;
          default = null;
          description = "Path to background image";
        };

        scale = mkOption {
          type = types.enum [
            "center"
            "fit"
            "fill"
            "stretch"
            "tile"
          ];
          default = "fill";
          description = "Background image scaling mode";
        };
      };
    };
  };

  config = mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];

    security.pam.services.nlock = { };

    environment.etc."nlock/nlock.toml".source =
      let
        nullToEmpty = v: if v == null then "" else v;

        settings = cfg.settings // {
          font = cfg.settings.font // {
            family = nullToEmpty cfg.settings.font.family;
          };
          image = cfg.settings.image // {
            path = nullToEmpty cfg.settings.image.path;
          };
        };
      in
      pkgs.writers.writeTOML "nlock.toml" settings;
  };
}
