// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026, Nathan Gill

use std::path::{Path, PathBuf};

use clap::{
    Command, CommandFactory, FromArgMatches, Parser, Subcommand,
    builder::{
        Styles,
        styling::{AnsiColor, Effects},
    },
};
use clap_complete::{Shell, aot::generate as generate_completions};

use crate::util::{BackgroundImageScale, BackgroundType, FontSlant, FontWeight, LogLevel, Rgba};

/// Customisable, minimalist screen locker for Wayland
#[derive(Parser, Debug)]
pub struct NLockArgs {
    #[command(subcommand)]
    pub subcommand: Option<NLockSubcommands>,

    /// Log verbosity
    #[arg(short, long, default_value = "info")]
    pub log_level: LogLevel,
    /// Configuration file path
    #[arg(short, long)]
    pub config_file: Option<String>,

    /// Sets the background color
    #[arg(long)]
    pub bg_color: Option<Rgba>,
    /// Sets the text color
    #[arg(long)]
    pub text_color: Option<Rgba>,
    /// Sets the input background color
    #[arg(long)]
    pub input_bg_color: Option<Rgba>,
    /// Sets the input border color
    #[arg(long)]
    pub input_border_color: Option<Rgba>,
    /// Sets the idle frame border color
    #[arg(long)]
    pub frame_border_idle_color: Option<Rgba>,
    /// Sets the success frame border color
    #[arg(long)]
    pub frame_border_success_color: Option<Rgba>,
    /// Sets the fail frame border color
    #[arg(long)]
    pub frame_border_fail_color: Option<Rgba>,

    /// Sets the font size, in points
    #[arg(long)]
    pub font_size: Option<f64>,
    /// Sets the font family
    #[arg(long)]
    pub font_family: Option<String>,
    /// Sets the font slant
    #[arg(long)]
    pub font_slant: Option<FontSlant>,
    /// Sets the font weight
    #[arg(long)]
    pub font_weight: Option<FontWeight>,
    /// Scale font size by display output DPI
    #[arg(long)]
    pub use_dpi_scaling: Option<bool>,

    /// Sets the mask character for the input box
    #[arg(long)]
    pub mask_char: Option<String>,
    /// Sets the relative width of the input box
    #[arg(long)]
    pub input_width: Option<f64>,
    /// Sets the relative horizontal padding of the input box
    #[arg(long)]
    pub input_padding_x: Option<f64>,
    /// Sets the relative vertical padding of the input box
    #[arg(long)]
    pub input_padding_y: Option<f64>,
    /// Sets the relative border radius of the input box
    #[arg(long)]
    pub input_radius: Option<f64>,
    /// Sets tne border width of the input box
    #[arg(long)]
    pub input_border: Option<f64>,
    /// Hide the input box when empty
    #[arg(long)]
    pub hide_when_empty: Option<bool>,
    /// Resize the input box to fit the entered password
    #[arg(long)]
    pub fit_to_content: Option<bool>,

    /// Sets the border radius of the frame
    #[arg(long)]
    pub frame_radius: Option<f64>,
    /// Sets the border width of the frame
    #[arg(long)]
    pub frame_border: Option<f64>,

    /// Validate empty passwords
    #[arg(long)]
    pub pwd_allow_empty: Option<bool>,
    /// Hide the mouse cursor
    #[arg(long)]
    pub hide_cursor: Option<bool>,

    /// Sets the background type
    #[arg(long)]
    pub bg_type: Option<BackgroundType>,
    /// Path to a background image
    #[arg(long)]
    pub image_path: Option<PathBuf>,
    /// Sets the image scaling mode
    #[arg(long)]
    pub image_scale: Option<BackgroundImageScale>,
}

#[derive(Subcommand, Debug)]
pub enum NLockSubcommands {
    /// Generate shell completions for nlock
    Completions {
        /// Shell to generate completions for
        shell: Option<String>,
    },
}

fn styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::BrightGreen.on_default().effects(Effects::BOLD))
        .usage(AnsiColor::BrightGreen.on_default().effects(Effects::BOLD))
        .literal(AnsiColor::BrightCyan.on_default().effects(Effects::BOLD))
        .placeholder(AnsiColor::BrightYellow.on_default())
        .valid(AnsiColor::BrightGreen.on_default())
        .invalid(AnsiColor::BrightRed.on_default())
}

fn build_cli() -> Command {
    NLockArgs::command().styles(styles())
}

pub fn run_cli() -> NLockArgs {
    let mut matches = build_cli().get_matches();
    let args = NLockArgs::from_arg_matches_mut(&mut matches).unwrap();

    // Shell completion subcommand
    if let Some(NLockSubcommands::Completions { shell }) = args.subcommand {
        let shell = if let Some(shell) = shell {
            Shell::from_shell_path(Path::new(&shell))
        } else {
            Shell::from_env()
        };

        if let Some(shell) = shell {
            let mut cli = build_cli();
            generate_completions(shell, &mut cli, "nlock", &mut std::io::stdout());
        } else {
            println!("Failed to identify shell");
            std::process::exit(1);
        }

        // Don't continue running after generation
        std::process::exit(0);
    }

    args
}
