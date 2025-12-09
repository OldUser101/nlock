// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2025, Nathan Gill

use clap::{
    Arg, Command, ValueEnum,
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

pub struct NLockArgs {
    pub log_level: LogLevel,
    pub config_file: Option<String>,
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
}

pub fn run_cli() -> NLockArgs {
    let cli = build_cli();
    let args = cli.get_matches();

    let log_level = args.get_one("log_level").cloned().unwrap_or(LogLevel::Info);
    let config_file: Option<String> = args.get_one::<String>("config_file").cloned();

    NLockArgs {
        log_level,
        config_file,
    }
}
