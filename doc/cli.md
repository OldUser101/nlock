# Command Line

Most, if not all, nlock configuration options can also be specified as
command line arguments.

The following options are only available as command line arguments:

- `-l`/`--log-level`, log level verbosity, defaults to `info`, can be `trace`,
    `debug`, `info`, `warn`, or `error`.
- `-c`/`--config-file`, configuration file path. A configuration file specified
    here is the **only** one loaded, any other configuration files on disk will
    be ignored. Options specified in here can still be overriden by command
    line options.

The following correspond directly to configuration options. See
[configuration file documentation](config.md) for more information about these.

- `--bg-color <COLOR>`, sets the background color
- `--text-color <COLOR>`, sets the text color
- `--input-bg-color <COLOR>`, sets the input background color
- `--input-border-color <COLOR>`, sets the input border color
- `--frame-border-idle-color <COLOR>`, sets the idle frame border color
- `--frame-border-success-color <COLOR>`, sets the success frame border color
- `--frame-border-fail-color <COLOR>`, sets the fail frame border color
- `--font-size <FLOAT>`, sets the font size, in points
- `--font-family <STRING>`, sets the font family
- `--font-slant <SLANT>`, sets the font slant
- `--font-weight <WEIGHT>`, sets the font weight
- `--use-dpi-scaling <BOOL>`, scale font size by display output DPI
- `--mask-char <STRING>`, sets the mask character for the input box
- `--input-width <FLOAT>`, sets tthe relative width of the input box
- `--input-padding_x <FLOAT>`, sets the relative horizontal padding of the input box
- `--input-padding_y <FLOAT>`, sets the relative vertical padding of the input box
- `--input-radius <FLOAT>`, sets the relative border radius of the input box
- `--input-border <FLOAT>`, sets the border width of the input box
- `--input-hide-when-empty <BOOL>`, hide the input box when empty
- `--input-fit-to-content <BOOL>`, resize the input box to fit password
- `--frame-radius <FLOAT>`, sets the border radius of the frame
- `--frame-border <FLOAT>`, sets the border width of the frame
- `--allow-empty-password <BOOL>`, validate empty passwords
- `--hide-cursor <BOOL>`, hide the mouse cursor
- `--bg-type <BACKGROUND TYPE>`, sets the background type
- `--image-path <PATH>`, path to a background image
- `--image-scale <SCALE MODE>`, sets the image scaling mode

