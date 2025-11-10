// SPDX-License-Idenifier: GPL-3.0-or-later
// Copyright (C) 2025, Nathan Gill

use std::{path::PathBuf, str::FromStr};

use anyhow::{Result, anyhow};
use config::{Config, ConfigBuilder, File, FileFormat, builder::DefaultState};
use dirs::config_dir;
use palette::Srgb;
use serde::Deserialize;
use tracing::debug;

const CONFIG_FILE_NAME: &str = "nlock.toml";
const CONFIG_DIR_NAME: &str = "nlock";
const SYSTEM_CONFIG_DIR: &str = "/usr/share";

#[derive(Deserialize)]
pub struct NLockRawConfig {
    #[serde(rename = "backgroundColor")]
    pub background_color: Option<String>,
}

impl NLockRawConfig {
    pub fn set_overrides(
        &self,
        builder: ConfigBuilder<DefaultState>,
    ) -> Result<ConfigBuilder<DefaultState>> {
        let mut builder = builder;

        if self.background_color.is_some() {
            builder = builder.set_override("backgroundColor", self.background_color.clone())?;
        }

        Ok(builder)
    }

    pub fn load() -> Result<Self> {
        let mut builder = Config::builder().set_default("backgroundColor", "#000000")?;

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

    pub fn finalize(&self) -> Result<NLockConfig> {
        Ok(NLockConfig {
            background_color: Srgb::from_str(
                self.background_color
                    .as_ref()
                    .ok_or(anyhow!("backgroundColor required, but not provided"))?,
            )?
            .into(),
        })
    }
}

pub struct NLockConfig {
    pub background_color: Srgb,
}
