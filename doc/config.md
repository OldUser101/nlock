# Configuration

nlock loads configuration files from two locations:

- `/etc/nlock/nlock.toml`, system-wide configuration
- `~/.config/nlock/nlock.toml`, user configuration

Configuration files are TOML formatted. The example configuration file can
be found [here](../examples/default.toml) demonstrating all available
configuration options.

If you specify invalid values in a configuration file, nlock will either show
an error, or continue with defaults if possible.
