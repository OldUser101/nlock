# Configuration

nlock loads configuration files from two locations:

- `/usr/share/nlock/nlock.toml`, system-wide configuration
- `~/.config/nlock/nlock.toml`, user configuration

Configuration files are TOML formatted. The following example demonstrates all
the available configuration options (this might not be up-to-date).

```toml
# The values defined in this example are the defaults used by nlock

# Colors section configures, well, colors.
[colors]
# Colors are in either #RRGGBBAA or #RRGGBB format,
# both #RRGGBBAA in this case.
background = "#000000FF"    # background color, same as "#000000"
text = "#FFFFFFFF"          # text color, same as "#FFFFFF"

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
```

Further configuration values are likely to be added in the future. Hopefully,
the above example will be updated when they are.

If you specify invalid values in a configuration file, nlock will either show
an error, or continue with defaults if possible.
