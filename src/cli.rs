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

use crate::surface::{FontSlant, FontWeight, Rgba};

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
    fn load_arg_matches(matches: &ArgMatches) -> Self;
}

macro_rules! args_get_value {
    ($matches:expr, $obj:ty, $name:expr) => {
        $matches.get_one::<$obj>($name).cloned()
    };
}

pub struct NLockArgs {
    pub log_level: LogLevel,
    pub config_file: Option<String>,
    pub colors: NLockArgsColors,
    pub font: NLockArgsFont,
}

impl LoadArgMatches for NLockArgs {
    fn load_arg_matches(matches: &ArgMatches) -> Self {
        let log_level = matches
            .get_one::<LogLevel>("log_level")
            .cloned()
            .unwrap_or(LogLevel::Info);
        let config_file = matches.get_one::<String>("config_file").cloned();

        Self {
            log_level,
            config_file,
            colors: NLockArgsColors::load_arg_matches(matches),
            font: NLockArgsFont::load_arg_matches(matches),
        }
    }
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

macro_rules! enum_arg {
    ($id:expr, $long:expr, $help:expr, $val:expr, $t:ty) => {
        Arg::new($id)
            .help($help)
            .long($long)
            .value_name($val)
            .value_parser(EnumValueParser::<$t>::new())
    };
}

macro_rules! generic_arg {
    ($id:expr, $long:expr, $help:expr, $val:expr) => {
        Arg::new($id).help($help).long($long).value_name($val)
    };
}

macro_rules! f64_arg {
    ($id:expr, $long:expr, $help:expr, $val:expr) => {
        Arg::new($id)
            .help($help)
            .long($long)
            .value_name($val)
            .value_parser(f64::from_str)
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
    fn load_arg_matches(matches: &ArgMatches) -> Self {
        let bg = args_get_value!(matches, Rgba, "bg_color");
        let text = args_get_value!(matches, Rgba, "text_color");
        let input_bg = args_get_value!(matches, Rgba, "input_bg_color");
        let input_border = args_get_value!(matches, Rgba, "input_border_color");
        let frame_border_idle = args_get_value!(matches, Rgba, "frame_border_idle_color");
        let frame_border_success = args_get_value!(matches, Rgba, "frame_border_success_color");
        let frame_border_fail = args_get_value!(matches, Rgba, "frame_border_fail_color");

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

pub struct NLockArgsFont {
    pub size: Option<f64>,
    pub family: Option<String>,
    pub slant: Option<FontSlant>,
    pub weight: Option<FontWeight>,
}

impl LoadArgMatches for NLockArgsFont {
    fn load_arg_matches(matches: &ArgMatches) -> Self {
        let size = args_get_value!(matches, f64, "font_size");
        let family = args_get_value!(matches, String, "font_family");
        let slant = args_get_value!(matches, FontSlant, "font_slant");
        let weight = args_get_value!(matches, FontWeight, "font_weight");

        Self {
            size,
            family,
            slant,
            weight,
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
        .arg(f64_arg!(
            "font_size",
            "font-size",
            "Sets the font size, in points",
            "SIZE"
        ))
        .arg(generic_arg!(
            "font_family",
            "font-family",
            "Sets the font family",
            "FAMILY"
        ))
        .arg(enum_arg!(
            "font_slant",
            "font-slant",
            "Sets the font slant",
            "SLANT",
            FontSlant
        ))
        .arg(enum_arg!(
            "font_weight",
            "font-weight",
            "Sets the font weight",
            "WEIGHT",
            FontWeight
        ))
}

pub fn run_cli() -> NLockArgs {
    let cli = build_cli();
    let args = cli.get_matches();

    NLockArgs::load_arg_matches(&args)
}
