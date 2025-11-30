// SPDX-License-Idenifier: GPL-3.0-or-later
// Copyright (C) 2025, Nathan Gill

use std::path::PathBuf;

use anyhow::{Result, anyhow};
use config::{Config, File, FileFormat};
use dirs::config_dir;
use serde::Deserialize;
use tracing::debug;

use crate::surface::{FontSlant, FontWeight, Rgba};

const CONFIG_FILE_NAME: &str = "nlock.toml";
const CONFIG_DIR_NAME: &str = "nlock";
const SYSTEM_CONFIG_DIR: &str = "/usr/share";

#[derive(Default, Deserialize)]
pub struct NLockConfig {
    #[serde(default)]
    pub colors: NLockConfigColors,

    #[serde(default)]
    pub font: NLockConfigFont,

    #[serde(default)]
    pub input: NLockConfigInput,
}

#[derive(Deserialize)]
pub struct NLockConfigColors {
    #[serde(default = "default_bg_color", rename = "background")]
    pub bg: Rgba,

    #[serde(default = "default_text_color", rename = "text")]
    pub text: Rgba,
}

impl Default for NLockConfigColors {
    fn default() -> Self {
        Self {
            bg: default_bg_color(),
            text: default_text_color(),
        }
    }
}

fn default_bg_color() -> Rgba {
    Rgba::new(0.0, 0.0, 0.0, 1.0)
}

fn default_text_color() -> Rgba {
    Rgba::new(1.0, 1.0, 1.0, 1.0)
}

#[derive(Deserialize)]
pub struct NLockConfigFont {
    #[serde(default = "default_font_size")]
    pub size: f64,

    #[serde(default = "default_font_family")]
    pub family: String,

    #[serde(default = "default_font_slant")]
    pub slant: FontSlant,

    #[serde(default = "default_font_weight")]
    pub weight: FontWeight,
}

impl Default for NLockConfigFont {
    fn default() -> Self {
        Self {
            size: default_font_size(),
            family: default_font_family(),
            slant: default_font_slant(),
            weight: default_font_weight(),
        }
    }
}

fn default_font_size() -> f64 {
    72.0f64
}

fn default_font_family() -> String {
    "".to_string()
}

fn default_font_slant() -> FontSlant {
    FontSlant::Normal
}

fn default_font_weight() -> FontWeight {
    FontWeight::Normal
}

#[derive(Deserialize)]
pub struct NLockConfigInput {
    #[serde(default = "default_mask_char", rename = "maskChar")]
    pub mask_char: String,
}

impl Default for NLockConfigInput {
    fn default() -> Self {
        Self {
            mask_char: default_mask_char(),
        }
    }
}

fn default_mask_char() -> String {
    "*".to_string()
}

impl NLockConfig {
    pub fn load() -> Result<Self> {
        let mut builder = Config::builder();

        let mut system_config = PathBuf::from(SYSTEM_CONFIG_DIR);
        system_config.push(CONFIG_DIR_NAME);
        system_config.push(CONFIG_FILE_NAME);

        if system_config.is_file() {
            let system_config_str = system_config
                .to_str()
                .ok_or(anyhow!("Failed to get system config string from path"))?;
            builder = builder.add_source(File::new(system_config_str, FileFormat::Toml));
            debug!("Including config file {:#?}", system_config);
        }

        let mut user_config = config_dir().ok_or(anyhow!("Failed to get user config directory"))?;
        user_config.push(CONFIG_DIR_NAME);
        user_config.push(CONFIG_FILE_NAME);

        if user_config.is_file() {
            let user_config_str = user_config
                .to_str()
                .ok_or(anyhow!("Failed to get user config string from path"))?;
            builder = builder.add_source(File::new(user_config_str, FileFormat::Toml));
            debug!("Including config file {:#?}", user_config);
        }

        let config = builder.build()?;

        Ok(config.try_deserialize::<Self>()?)
    }
}
