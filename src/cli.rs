// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2025, Nathan Gill

use clap::{
    Arg, ArgMatches, Command, ValueEnum, arg,
    builder::{
        EnumValueParser, Styles,
        styling::{AnsiColor, Effects},
    },
};

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
}

impl LoadArgMatches for NLockArgs {
    fn load_arg_matches(matches: ArgMatches) -> Self {
        let log_level = matches
            .get_one("log-level")
            .cloned()
            .unwrap_or(LogLevel::Info);
        let config_file: Option<String> = matches.get_one::<String>("config-file").cloned();

        Self {
            log_level,
            config_file,
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
        .arg(arg!(--bg - color))
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
}

pub fn run_cli() -> NLockArgs {
    let cli = build_cli();
    let args = cli.get_matches();

    NLockArgs::load_arg_matches(args)
}
