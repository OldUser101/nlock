// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2025, Nathan Gill

use std::str::FromStr;

use clap::{
    Arg, ArgMatches, Command, ValueEnum,
    builder::{
        EnumValueParser, Styles,
        styling::{AnsiColor, Effects},
    },
};

use crate::surface::Rgba;

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn to_level(self) -> tracing::Level {
        match self {
            Self::Trace => tracing::Level::TRACE,
            Self::Debug => tracing::Level::DEBUG,
            Self::Info => tracing::Level::INFO,
            Self::Warn => tracing::Level::WARN,
            Self::Error => tracing::Level::ERROR,
        }
    }
}

pub trait LoadArgMatches {
    fn load_arg_matches(matches: ArgMatches) -> Self;
}

pub struct NLockArgs {
    pub log_level: LogLevel,
    pub config_file: Option<String>,
    pub colors: NLockArgsColors,
}

impl LoadArgMatches for NLockArgs {
    fn load_arg_matches(matches: ArgMatches) -> Self {
        let log_level = matches
            .get_one::<LogLevel>("log_level")
            .cloned()
            .unwrap_or(LogLevel::Info);
        let config_file = matches.get_one::<String>("config_file").cloned();

        Self {
            log_level,
            config_file,
            colors: NLockArgsColors::load_arg_matches(matches),
        }
    }
}

macro_rules! args_get_color {
    ($matches:expr, $name:expr) => {
        $matches.get_one::<Rgba>($name).cloned()
    };
}

macro_rules! color_arg {
    ($id:expr, $long:expr, $help:expr) => {
        Arg::new($id)
            .help($help)
            .long($long)
            .value_name("COLOR")
            .value_parser(Rgba::from_str)
    };
}

pub struct NLockArgsColors {
    pub bg: Option<Rgba>,
    pub text: Option<Rgba>,
    pub input_bg: Option<Rgba>,
    pub input_border: Option<Rgba>,
    pub frame_border_idle: Option<Rgba>,
    pub frame_border_success: Option<Rgba>,
    pub frame_border_fail: Option<Rgba>,
}

impl LoadArgMatches for NLockArgsColors {
    fn load_arg_matches(matches: ArgMatches) -> Self {
        let bg = args_get_color!(matches, "bg_color");
        let text = args_get_color!(matches, "text_color");
        let input_bg = args_get_color!(matches, "input_bg_color");
        let input_border = args_get_color!(matches, "input_border_color");
        let frame_border_idle = args_get_color!(matches, "frame_border_idle_color");
        let frame_border_success = args_get_color!(matches, "frame_border_success_color");
        let frame_border_fail = args_get_color!(matches, "frame_border_fail_color");

        Self {
            bg,
            text,
            input_bg,
            input_border,
            frame_border_idle,
            frame_border_success,
            frame_border_fail,
        }
    }
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
    Command::new("nlock")
        .about("Customisable, minimalist screen locker for Wayland")
        .version(env!("CARGO_PKG_VERSION"))
        .styles(styles())
        .arg(
            Arg::new("log_level")
                .help("Log level verbosity")
                .short('l')
                .long("log-level")
                .value_name("LOG LEVEL")
                .value_parser(EnumValueParser::<LogLevel>::new())
                .default_value("info"),
        )
        .arg(
            Arg::new("config_file")
                .help("Path to the configuration file")
                .short('c')
                .long("config-file")
                .value_name("CONFIG FILE"),
        )
        .arg(color_arg!(
            "bg_color",
            "bg-color",
            "Sets the background color"
        ))
        .arg(color_arg!(
            "text_color",
            "text-color",
            "Sets the text color"
        ))
        .arg(color_arg!(
            "input_bg_color",
            "input-bg-color",
            "Sets the input background color"
        ))
        .arg(color_arg!(
            "input_border_color",
            "input-border-color",
            "Sets the input border color"
        ))
        .arg(color_arg!(
            "frame_border_idle_color",
            "frame-border-idle-color",
            "Sets the idle frame border color"
        ))
        .arg(color_arg!(
            "frame_border_success_color",
            "frame-border-success-color",
            "Sets the success frame border color"
        ))
        .arg(color_arg!(
            "frame_border_fail_color",
            "frame-border-fail-color",
            "Sets the fail frame border color"
        ))
}

pub fn run_cli() -> NLockArgs {
    let cli = build_cli();
    let args = cli.get_matches();

    NLockArgs::load_arg_matches(args)
}
