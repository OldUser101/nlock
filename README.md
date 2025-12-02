# nlock

nlock is a customisable, minimalist screen locker for Wayland compositors.
nlock uses the `ext-session-lock-v1` protocol, and should be compatible with
any compositor that correctly implements it.

## Usage

See [the documentation](doc/toc.md).

## Building

nlock is written in Rust, and uses Cargo as it's build system. It should
compile with the latest stable Rust, I haven't tested older versions.

In addition, you'll need development libraries for the following, which can
probably be installed via your system package manager:

- Clang
- GLib
- PAM
- Cairo
- xkbcommon

With all of that, you should just be able to clone this repository, and run:

```sh
$ cargo build --release
```

The generated binary should then be located at `target/release/nlock`.

## Credits

Several other projects have been very helpful during development of nlock:

- [swaylock](https://github.com/swaywm/swaylock) - rendering with Cairo,
    and general architecture.
- [where-is-my-sddm-theme](https://github.com/stepanzubkov/where-is-my-sddm-theme) -
    the SDDM theme that nlock is based on.

## License

nlock is licensed under the GNU General Public License Version 3, or later.
See [LICENSE](LICENSE) for details.
