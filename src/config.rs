// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026, Nathan Gill

use std::path::PathBuf;

use anyhow::{Result, anyhow};
use config::{Config, File, FileFormat};
use dirs::config_dir;
use serde::Deserialize;
use tracing::debug;

use crate::{
    args::NLockArgs,
    util::{BackgroundImageScale, BackgroundType, FontSlant, FontWeight, InputVisibility, Rgba},
};

const CONFIG_FILE_NAME: &str = "nlock.toml";
const CONFIG_DIR_NAME: &str = "nlock";
const SYSTEM_CONFIG_DIR: &str = "/etc";

macro_rules! set_if_some {
    ($target:expr, $opt:expr) => {
        if let Some(val) = $opt {
            $target = val;
        }
    };
}

macro_rules! set_if_some_string {
    ($target:expr, $opt:expr) => {
        if let Some(val) = $opt {
            $target = val.to_string();
        }
    };
}

macro_rules! set_if_some_path {
    ($target:expr, $opt:expr) => {
        if let Some(val) = $opt {
            $target = val.clone();
        }
    };
}

pub trait LoadArgOverrides {
    fn load_arg_overrides(&mut self, args: &NLockArgs);
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NLockConfig {
    #[serde(default)]
    pub colors: NLockConfigColors,

    #[serde(default)]
    pub font: NLockConfigFont,

    #[serde(default)]
    pub input: NLockConfigInput,

    #[serde(default)]
    pub frame: NLockConfigFrame,

    #[serde(default)]
    pub general: NLockConfigGeneral,

    #[serde(default)]
    pub image: NLockConfigImage,
}

impl LoadArgOverrides for NLockConfig {
    fn load_arg_overrides(&mut self, args: &NLockArgs) {
        self.colors.load_arg_overrides(args);
        self.font.load_arg_overrides(args);
        self.input.load_arg_overrides(args);
        self.frame.load_arg_overrides(args);
        self.general.load_arg_overrides(args);
        self.image.load_arg_overrides(args);
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NLockConfigColors {
    #[serde(default = "default_bg_color", rename = "background")]
    pub bg: Rgba,

    #[serde(default = "default_text_color", rename = "text")]
    pub text: Rgba,

    #[serde(default = "default_input_bg_color", rename = "inputBackground")]
    pub input_bg: Rgba,

    #[serde(default = "default_input_border_color", rename = "inputBorder")]
    pub input_border: Rgba,

    #[serde(
        default = "default_frame_border_idle_color",
        rename = "frameBorderIdle"
    )]
    pub frame_border_idle: Rgba,

    #[serde(
        default = "default_frame_border_success_color",
        rename = "frameBorderSuccess"
    )]
    pub frame_border_success: Rgba,

    #[serde(
        default = "default_frame_border_fail_color",
        rename = "frameBorderFail"
    )]
    pub frame_border_fail: Rgba,
}

impl Default for NLockConfigColors {
    fn default() -> Self {
        Self {
            bg: default_bg_color(),
            text: default_text_color(),
            input_bg: default_input_bg_color(),
            input_border: default_input_border_color(),
            frame_border_idle: default_frame_border_idle_color(),
            frame_border_success: default_frame_border_success_color(),
            frame_border_fail: default_frame_border_fail_color(),
        }
    }
}

impl LoadArgOverrides for NLockConfigColors {
    fn load_arg_overrides(&mut self, args: &NLockArgs) {
        set_if_some!(self.bg, args.bg_color);
        set_if_some!(self.text, args.text_color);
        set_if_some!(self.input_bg, args.input_bg_color);
        set_if_some!(self.input_border, args.input_border_color);
        set_if_some!(self.frame_border_idle, args.frame_border_idle_color);
        set_if_some!(self.frame_border_success, args.frame_border_success_color);
        set_if_some!(self.frame_border_fail, args.frame_border_fail_color);
    }
}

fn default_bg_color() -> Rgba {
    Rgba::new(0.0, 0.0, 0.0, 1.0)
}

fn default_text_color() -> Rgba {
    Rgba::new(1.0, 1.0, 1.0, 1.0)
}

fn default_input_bg_color() -> Rgba {
    Rgba::new(0.0, 0.0, 0.0, 1.0)
}

fn default_input_border_color() -> Rgba {
    Rgba::new(0.0, 0.0, 0.0, 1.0)
}

fn default_frame_border_idle_color() -> Rgba {
    Rgba::new(0.0, 0.0, 0.0, 0.0)
}

fn default_frame_border_success_color() -> Rgba {
    Rgba::new(0.0, 0.0, 0.0, 0.0)
}

fn default_frame_border_fail_color() -> Rgba {
    Rgba::new(1.0, 0.0, 0.0, 1.0)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NLockConfigFont {
    #[serde(default = "default_font_size")]
    pub size: f64,

    #[serde(default = "default_font_family")]
    pub family: String,

    #[serde(default = "default_font_slant")]
    pub slant: FontSlant,

    #[serde(default = "default_font_weight")]
    pub weight: FontWeight,

    #[serde(default = "default_font_use_dpi_scaling", rename = "useDpiScaling")]
    pub use_dpi_scaling: bool,
}

impl Default for NLockConfigFont {
    fn default() -> Self {
        Self {
            size: default_font_size(),
            family: default_font_family(),
            slant: default_font_slant(),
            weight: default_font_weight(),
            use_dpi_scaling: default_font_use_dpi_scaling(),
        }
    }
}

impl LoadArgOverrides for NLockConfigFont {
    fn load_arg_overrides(&mut self, args: &NLockArgs) {
        set_if_some!(self.size, args.font_size);
        set_if_some_string!(self.family, &args.font_family);
        set_if_some!(self.slant, args.font_slant);
        set_if_some!(self.weight, args.font_weight);
        set_if_some!(self.use_dpi_scaling, args.use_dpi_scaling);
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

fn default_font_use_dpi_scaling() -> bool {
    false
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NLockConfigInput {
    #[serde(default = "default_mask_char", rename = "maskChar")]
    pub mask_char: String,

    #[serde(default = "default_input_width")]
    pub width: f64,

    #[serde(default = "default_input_padding", rename = "paddingX")]
    pub padding_x: f64,

    #[serde(default = "default_input_padding", rename = "paddingY")]
    pub padding_y: f64,

    #[serde(default = "default_input_radius")]
    pub radius: f64,

    #[serde(default = "default_input_border")]
    pub border: f64,

    #[serde(default = "default_input_visible")]
    pub visible: InputVisibility,

    #[serde(default = "default_input_fit_to_content", rename = "fitToContent")]
    pub fit_to_content: bool,
}

impl Default for NLockConfigInput {
    fn default() -> Self {
        Self {
            mask_char: default_mask_char(),
            width: default_input_width(),
            padding_x: default_input_padding(),
            padding_y: default_input_padding(),
            radius: default_input_radius(),
            border: default_input_border(),
            visible: default_input_visible(),
            fit_to_content: default_input_fit_to_content(),
        }
    }
}

impl LoadArgOverrides for NLockConfigInput {
    fn load_arg_overrides(&mut self, args: &NLockArgs) {
        set_if_some_string!(self.mask_char, &args.mask_char);
        set_if_some!(self.width, args.input_width);
        set_if_some!(self.padding_x, args.input_padding_x);
        set_if_some!(self.padding_y, args.input_padding_y);
        set_if_some!(self.radius, args.input_radius);
        set_if_some!(self.border, args.input_border);
        set_if_some!(self.visible, args.input_visible);
        set_if_some!(self.fit_to_content, args.fit_to_content);
    }
}

fn default_mask_char() -> String {
    "*".to_string()
}

fn default_input_width() -> f64 {
    0.5f64
}

fn default_input_padding() -> f64 {
    0.05f64
}

fn default_input_radius() -> f64 {
    0.0f64
}

fn default_input_border() -> f64 {
    0.0f64
}

fn default_input_visible() -> InputVisibility {
    InputVisibility::Always
}

fn default_input_fit_to_content() -> bool {
    false
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NLockConfigFrame {
    #[serde(default = "default_frame_border")]
    pub border: f64,

    #[serde(default = "default_frame_radius")]
    pub radius: f64,
}

impl Default for NLockConfigFrame {
    fn default() -> Self {
        Self {
            border: default_frame_border(),
            radius: default_frame_radius(),
        }
    }
}

impl LoadArgOverrides for NLockConfigFrame {
    fn load_arg_overrides(&mut self, args: &NLockArgs) {
        set_if_some!(self.border, args.frame_border);
        set_if_some!(self.radius, args.frame_radius);
    }
}

fn default_frame_border() -> f64 {
    25.0f64
}

fn default_frame_radius() -> f64 {
    0.0f64
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NLockConfigGeneral {
    #[serde(default = "default_pwd_allow_empty", rename = "allowEmptyPassword")]
    pub pwd_allow_empty: bool,

    #[serde(default = "default_hide_cursor", rename = "hideCursor")]
    pub hide_cursor: bool,

    #[serde(default = "default_bg_type", rename = "backgroundType")]
    pub bg_type: BackgroundType,
}

impl Default for NLockConfigGeneral {
    fn default() -> Self {
        Self {
            pwd_allow_empty: default_pwd_allow_empty(),
            hide_cursor: default_hide_cursor(),
            bg_type: default_bg_type(),
        }
    }
}

impl LoadArgOverrides for NLockConfigGeneral {
    fn load_arg_overrides(&mut self, args: &NLockArgs) {
        set_if_some!(self.pwd_allow_empty, args.pwd_allow_empty);
        set_if_some!(self.hide_cursor, args.hide_cursor);
        set_if_some!(self.bg_type, args.bg_type);
    }
}

fn default_pwd_allow_empty() -> bool {
    false
}

fn default_hide_cursor() -> bool {
    true
}

fn default_bg_type() -> BackgroundType {
    BackgroundType::Color
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NLockConfigImage {
    #[serde(default = "default_image_path")]
    pub path: PathBuf,

    #[serde(default = "default_image_scale")]
    pub scale: BackgroundImageScale,
}

impl Default for NLockConfigImage {
    fn default() -> Self {
        Self {
            path: default_image_path(),
            scale: default_image_scale(),
        }
    }
}

impl LoadArgOverrides for NLockConfigImage {
    fn load_arg_overrides(&mut self, args: &NLockArgs) {
        set_if_some_path!(self.path, &args.image_path);
        set_if_some!(self.scale, args.image_scale);
    }
}

fn default_image_path() -> PathBuf {
    PathBuf::from("")
}

fn default_image_scale() -> BackgroundImageScale {
    BackgroundImageScale::Fill
}

impl NLockConfig {
    pub fn load(args: &NLockArgs) -> Result<Self> {
        let mut builder = Config::builder();

        if let Some(config_file) = &args.config_file {
            let custom_config = PathBuf::from(config_file);

            if custom_config.is_file() {
                builder = builder.add_source(File::new(config_file, FileFormat::Toml));
                debug!("Including config file {:#?}", custom_config);
            }
        } else {
            // TODO: I really need wrap this loading in something

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

            let mut user_config =
                config_dir().ok_or(anyhow!("Failed to get user config directory"))?;
            user_config.push(CONFIG_DIR_NAME);
            user_config.push(CONFIG_FILE_NAME);

            if user_config.is_file() {
                let user_config_str = user_config
                    .to_str()
                    .ok_or(anyhow!("Failed to get user config string from path"))?;
                builder = builder.add_source(File::new(user_config_str, FileFormat::Toml));
                debug!("Including config file {:#?}", user_config);
            }
        }

        let config = builder.build()?;
        let mut parsed_config = config.try_deserialize::<Self>()?;

        parsed_config.load_arg_overrides(args);

        Ok(parsed_config)
    }
}
