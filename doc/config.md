# Configuration

nlock loads configuration files from two locations:

- `/etc/nlock/nlock.toml`, system-wide configuration
- `~/.config/nlock/nlock.toml`, user configuration

Configuration files are TOML formatted. The following example demonstrates all
the available configuration options (this might not be up-to-date).

```toml
# The values defined in this example are the defaults used by nlock

# General configuration options
[general]
allowEmptyPassword = false      # allow a blank password to be validated

# Colors section configures, well, colors.
[colors]
# Colors are in either #RRGGBBAA or #RRGGBB format,
# both #RRGGBBAA in this case.
background = "#000000FF"            # background color, same as "#000000"
text = "#FFFFFFFF"                  # text color, same as "#FFFFFF"
inputBackground = "#000000FF"       # input box background color
inputBorder = "#000000FF"           # input box border color
frameBorderIdle = "#00000000"       # frame border idle color
frameBorderSuccess = "#00000000"    # frame border success color
frameBorderFail = "#FF0000FF"       # frame border error color

# Font section configures text display.
[font]
size = 72.0     # font size, in points

# Font family uses Cairo's toy font API, generally, standard CSS names
# like `monospace`, `serif`, etc. should work. Leaving it empty (or invalid)
# will select the system default.
family = ""

slant = "normal"    # font slant, either "normal", "italic", or "oblique"
weight = "normal"   # font weight, either "normal", or "bold"

# Input section configures the password input box.
[input]
maskChar = "*"      # character displayed in place of password characters
width = 0.5         # width of the input box, relative to display width
paddingX = 0.05     # input box horizontal padding, relative to display width
paddingY = 0.05     # input box vertical padding, relative to display height
radius = 0.0        # radius of input box corners, relative to total box height
border = 0.0        # width of input box border, absolute, typically pixels

# Frame section configures everything around the input box.
[frame]
border = 25.0   # width of frame border, absolute units, typically pixels
radius = 0.0    # radius of frame border, absolute units, typically pixels
```

Further configuration values are likely to be added in the future. Hopefully,
the above example will be updated when they are.

If you specify invalid values in a configuration file, nlock will either show
an error, or continue with defaults if possible.
