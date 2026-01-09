// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026, Nathan Gill

use std::{os::fd::OwnedFd, str::FromStr};

use clap::ValueEnum;
use nix::{
    fcntl::OFlag,
    libc,
    sys::{
        mman::{shm_open, shm_unlink},
        stat::Mode,
    },
    unistd::getpid,
};
use serde::{Deserialize, de};
use tracing::debug;

#[derive(Debug, Deserialize, Copy, Clone, PartialEq, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum BackgroundType {
    Color,
    Image,
}

#[derive(Deserialize, Copy, Clone, PartialEq, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum BackgroundImageScale {
    Stretch,
    Fill,
    Fit,
    Center,
    Tile,
}

#[derive(Debug, Copy, Clone)]
pub struct Rgba {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl Rgba {
    pub fn new(r: f64, g: f64, b: f64, a: f64) -> Self {
        Self { r, g, b, a }
    }
}

impl Default for Rgba {
    fn default() -> Self {
        Self::new(0.0, 0.0, 0.0, 1.0)
    }
}

impl FromStr for Rgba {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let s = s.strip_prefix('#').unwrap_or(s);
        let argb = match s.len() {
            6 => {
                let r =
                    u8::from_str_radix(&s[0..2], 16).map_err(|e| e.to_string())? as f64 / 255.0f64;
                let g =
                    u8::from_str_radix(&s[2..4], 16).map_err(|e| e.to_string())? as f64 / 255.0f64;
                let b =
                    u8::from_str_radix(&s[4..6], 16).map_err(|e| e.to_string())? as f64 / 255.0f64;
                Self { r, g, b, a: 1.0 }
            }
            8 => {
                let r =
                    u8::from_str_radix(&s[0..2], 16).map_err(|e| e.to_string())? as f64 / 255.0f64;
                let g =
                    u8::from_str_radix(&s[2..4], 16).map_err(|e| e.to_string())? as f64 / 255.0f64;
                let b =
                    u8::from_str_radix(&s[4..6], 16).map_err(|e| e.to_string())? as f64 / 255.0f64;
                let a =
                    u8::from_str_radix(&s[6..8], 16).map_err(|e| e.to_string())? as f64 / 255.0f64;
                Self { r, g, b, a }
            }
            _ => return Err("expected RRGGBBAA or RRGGBB format".to_string()),
        };
        Ok(argb)
    }
}

impl<'de> Deserialize<'de> for Rgba {
    fn deserialize<D>(d: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(d)?;
        let argb = Self::from_str(&s).map_err(de::Error::custom)?;
        Ok(argb)
    }
}

#[derive(Debug, Deserialize, Copy, Clone, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum FontSlant {
    Normal,
    Italic,
    Oblique,
}

impl From<FontSlant> for cairo::FontSlant {
    fn from(value: FontSlant) -> Self {
        match value {
            FontSlant::Normal => Self::Normal,
            FontSlant::Italic => Self::Italic,
            FontSlant::Oblique => Self::Oblique,
        }
    }
}

#[derive(Debug, Deserialize, Copy, Clone, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum FontWeight {
    Normal,
    Bold,
}

impl From<FontWeight> for cairo::FontWeight {
    fn from(value: FontWeight) -> Self {
        match value {
            FontWeight::Normal => Self::Normal,
            FontWeight::Bold => Self::Bold,
        }
    }
}

pub fn open_shm() -> Option<OwnedFd> {
    let mut retries = 100;

    loop {
        let time = chrono::Local::now();
        let name = format!(
            "/nlock-{}-{}-{}",
            getpid(),
            time.timestamp_micros(),
            time.timestamp_subsec_nanos()
        );
        debug!("Trying shm file name '{}'", name);

        if let Ok(fd) = shm_open(
            name.as_str(),
            OFlag::O_RDWR | OFlag::O_CREAT | OFlag::O_EXCL,
            Mode::S_IRUSR | Mode::S_IWUSR,
        ) {
            let _ = shm_unlink(name.as_str());
            return Some(fd);
        }

        retries -= 1;
        if retries <= 0 {
            break;
        }
    }

    None
}

// This helper function just checks if an `std::io::Error` was an EINTR
pub fn is_eintr(err: &std::io::Error) -> bool {
    match err.raw_os_error() {
        Some(code) => code == libc::EINTR,
        None => false,
    }
}
