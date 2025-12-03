// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2025, Nathan Gill

use std::sync::atomic::Ordering;

use anyhow::{Result, anyhow};
use serde::{Deserialize, de};
use tracing::warn;
use wayland_client::{
    Dispatch, QueueHandle, WEnum,
    protocol::{wl_compositor, wl_output, wl_shm, wl_surface},
};
use wayland_protocols::ext::session_lock::v1::client::{
    ext_session_lock_surface_v1, ext_session_lock_v1,
};

use crate::{auth::AuthState, buffer::NLockBuffer, config::NLockConfig, state::NLockState};

const DEFAULT_DPI: f64 = 96.0;

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

impl<'de> Deserialize<'de> for Rgba {
    fn deserialize<D>(d: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(d)?;
        let s = s.strip_prefix('#').ok_or(de::Error::custom("missing #"))?;
        let argb = match s.len() {
            6 => {
                let r =
                    u8::from_str_radix(&s[0..2], 16).map_err(de::Error::custom)? as f64 / 255.0f64;
                let g =
                    u8::from_str_radix(&s[2..4], 16).map_err(de::Error::custom)? as f64 / 255.0f64;
                let b =
                    u8::from_str_radix(&s[4..6], 16).map_err(de::Error::custom)? as f64 / 255.0f64;
                Rgba { r, g, b, a: 1.0 }
            }
            8 => {
                let r =
                    u8::from_str_radix(&s[0..2], 16).map_err(de::Error::custom)? as f64 / 255.0f64;
                let g =
                    u8::from_str_radix(&s[2..4], 16).map_err(de::Error::custom)? as f64 / 255.0f64;
                let b =
                    u8::from_str_radix(&s[4..6], 16).map_err(de::Error::custom)? as f64 / 255.0f64;
                let a =
                    u8::from_str_radix(&s[6..8], 16).map_err(de::Error::custom)? as f64 / 255.0f64;
                Rgba { r, g, b, a }
            }
            _ => return Err(de::Error::custom("expected #RRGGBBAA or #RRGGBB format")),
        };
        Ok(argb)
    }
}

#[derive(Debug, Deserialize, Copy, Clone)]
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

#[derive(Debug, Deserialize, Copy, Clone)]
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

pub struct NLockSurface {
    pub created: bool,
    pub index: usize,
    pub output_name: Option<String>,
    pub output_scale: i32,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub physical_width: Option<i32>,
    pub physical_height: Option<i32>,
    pub dpi: Option<f64>,
    pub subpixel: Option<WEnum<wl_output::Subpixel>>,
    pub surface: Option<wl_surface::WlSurface>,
    pub output: wl_output::WlOutput,
    pub lock_surface: Option<ext_session_lock_surface_v1::ExtSessionLockSurfaceV1>,
    pub buffers: Vec<NLockBuffer>,
}

impl NLockSurface {
    pub fn new(output: wl_output::WlOutput, index: usize) -> Self {
        Self {
            created: false,
            index,
            output_name: None,
            output_scale: 1,
            width: None,
            height: None,
            physical_width: None,
            physical_height: None,
            dpi: None,
            subpixel: None,
            surface: None,
            output,
            lock_surface: None,
            buffers: Vec::new(),
        }
    }

    fn get_cairo_subpixel_order(&self) -> cairo::SubpixelOrder {
        if let Some(subpixel) = self.subpixel {
            match subpixel {
                WEnum::Value(wl_output::Subpixel::HorizontalRgb) => {
                    return cairo::SubpixelOrder::Rgb;
                }
                WEnum::Value(wl_output::Subpixel::HorizontalBgr) => {
                    return cairo::SubpixelOrder::Bgr;
                }
                WEnum::Value(wl_output::Subpixel::VerticalRgb) => {
                    return cairo::SubpixelOrder::Vrgb;
                }
                WEnum::Value(wl_output::Subpixel::VerticalBgr) => {
                    return cairo::SubpixelOrder::Vbgr;
                }
                _ => {
                    return cairo::SubpixelOrder::Default;
                }
            }
        }

        cairo::SubpixelOrder::Default
    }

    fn clear_surface(&self, config: &NLockConfig, context: &cairo::Context) -> Result<()> {
        context.save()?;
        context.set_source_rgba(
            config.colors.bg.r,
            config.colors.bg.g,
            config.colors.bg.b,
            config.colors.bg.a,
        );
        context.set_operator(cairo::Operator::Source);
        context.paint()?;
        context.restore()?;

        Ok(())
    }

    fn configure_cairo_init(&self, context: &mut cairo::Context) -> Result<()> {
        context.set_antialias(cairo::Antialias::Best);

        context.save()?;
        context.set_operator(cairo::Operator::Source);
        context.set_source_rgb(0.0, 0.0, 0.0);
        context.paint()?;
        context.restore()?;
        context.identity_matrix();

        Ok(())
    }

    fn configure_cairo_font(&self, config: &NLockConfig, context: &cairo::Context) -> Result<()> {
        let mut fo = cairo::FontOptions::new()?;
        fo.set_hint_style(cairo::HintStyle::Full);
        fo.set_antialias(cairo::Antialias::Subpixel);
        fo.set_subpixel_order(self.get_cairo_subpixel_order());

        context.set_font_options(&fo);
        context.select_font_face(
            &config.font.family,
            cairo::FontSlant::from(config.font.slant),
            cairo::FontWeight::from(config.font.weight),
        );

        let dpi = self.dpi.unwrap_or(DEFAULT_DPI);
        context.set_font_size((config.font.size / 72.0) * dpi);

        Ok(())
    }

    fn get_buffer_idx(
        &mut self,
        shm: &wl_shm::WlShm,
        qh: &QueueHandle<NLockState>,
    ) -> Option<usize> {
        let width = self.width?;
        let height = self.height?;

        let index = self
            .buffers
            .iter()
            .position(|buf| buf.state.lock().map(|state| !state.in_use).unwrap_or(false));

        let idx = match index {
            Some(i) => i,
            None => {
                let mut buf = NLockBuffer::new(
                    shm,
                    width as i32,
                    height as i32,
                    wl_shm::Format::Argb8888,
                    true,
                    qh,
                )?;
                self.configure_cairo_init(&mut buf.context).ok()?;

                self.buffers.push(buf);

                self.buffers.len() - 1
            }
        };

        Some(idx)
    }

    pub fn calculate_dpi(&mut self) -> Result<()> {
        let width = self.width.ok_or(anyhow!("Surface width not set"))? as f64;
        let height = self.height.ok_or(anyhow!("Surface height not set"))? as f64;
        let physical_width = self
            .physical_width
            .ok_or(anyhow!("Output physical width not set"))? as f64;
        let physical_height = self
            .physical_height
            .ok_or(anyhow!("Output physical height not set"))? as f64;

        if physical_width == 0.0 || physical_height == 0.0 {
            self.dpi = Some(DEFAULT_DPI);
        }

        let dpi_x = width / (physical_width / 25.4);
        let dpi_y = height / (physical_height / 25.4);
        let dpi = (dpi_x + dpi_y) / 2.0;

        self.dpi = Some(dpi);

        Ok(())
    }

    pub fn create_surface(
        &mut self,
        compositor: &wl_compositor::WlCompositor,
        session_lock: &ext_session_lock_v1::ExtSessionLockV1,
        qh: &QueueHandle<NLockState>,
    ) {
        if !self.created {
            let surface = compositor.create_surface(qh, ());
            self.surface = Some(surface);

            if let Some(surface) = &self.surface {
                let lock_surface =
                    session_lock.get_lock_surface(surface, &self.output, qh, self.index);
                self.lock_surface = Some(lock_surface);
            }

            self.created = true;
        }
    }

    fn draw_rounded_rect(context: &cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
        context.new_sub_path();
        context.arc(x + w - r, y + r, r, -90f64.to_radians(), 0f64.to_radians());
        context.arc(
            x + w - r,
            y + h - r,
            r,
            0f64.to_radians(),
            90f64.to_radians(),
        );
        context.arc(x + r, y + h - r, r, 90f64.to_radians(), 180f64.to_radians());
        context.arc(x + r, y + r, r, 180f64.to_radians(), 270f64.to_radians());
        context.close_path();
    }

    fn set_frame_border_color(
        &self,
        config: &NLockConfig,
        context: &cairo::Context,
        auth_state: AuthState,
    ) {
        match auth_state {
            AuthState::Idle => context.set_source_rgba(
                config.colors.frame_border_idle.r,
                config.colors.frame_border_idle.g,
                config.colors.frame_border_idle.b,
                config.colors.frame_border_idle.a,
            ),
            AuthState::Success => context.set_source_rgba(
                config.colors.frame_border_success.r,
                config.colors.frame_border_success.g,
                config.colors.frame_border_success.b,
                config.colors.frame_border_success.a,
            ),
            AuthState::Fail => context.set_source_rgba(
                config.colors.frame_border_fail.r,
                config.colors.frame_border_fail.g,
                config.colors.frame_border_fail.b,
                config.colors.frame_border_fail.a,
            ),
        }
    }

    pub fn render(
        &mut self,
        config: &NLockConfig,
        auth_state: AuthState,
        password_len: usize,
        shm: &wl_shm::WlShm,
        qh: &QueueHandle<NLockState>,
    ) {
        let idx = match self.get_buffer_idx(shm, qh) {
            Some(i) => i,
            None => {
                warn!("Failed to obtain buffer for rendering");
                return;
            }
        };

        let surface = match &self.surface {
            Some(s) => s,
            None => {
                warn!("wl_surface not set when attempting render");
                return;
            }
        };

        let buffer = &self.buffers[idx];
        let wl_buffer = &buffer.buffer;

        let context = &buffer.context;
        if let Err(e) = self.render_frame(config, auth_state, password_len, context) {
            warn!("Error while rendering: {e}");
        }

        surface.set_buffer_scale(self.output_scale);
        surface.attach(Some(wl_buffer), 0, 0);
        surface.damage(0, 0, i32::MAX, i32::MAX);
        surface.commit();
    }

    fn render_frame(
        &self,
        config: &NLockConfig,
        auth_state: AuthState,
        password_len: usize,
        context: &cairo::Context,
    ) -> Result<()> {
        let width = self.width.ok_or(anyhow!("Surface width not set"))? as f64;
        let height = self.height.ok_or(anyhow!("Surface height not set"))? as f64;

        self.configure_cairo_font(config, context)?;
        self.clear_surface(config, context)?;

        // Draw border colour
        context.save()?;
        self.set_frame_border_color(config, context, auth_state);
        context.set_line_width(config.frame.border);

        let frame_offset = config.frame.border / 2.0;
        let frame_w = width - (frame_offset * 2.0);
        let frame_h = height - (frame_offset * 2.0);

        Self::draw_rounded_rect(
            context,
            frame_offset,
            frame_offset,
            frame_w,
            frame_h,
            config.frame.radius,
        );
        context.stroke()?;
        context.restore()?;

        let fe = context.font_extents()?;

        let padding_x = config.input.padding_x * width;
        let padding_y = config.input.padding_y * height;

        let inner_h = fe.height();
        let inner_w = width * config.input.width;
        let inner_x = (width - inner_w) / 2.0;
        let inner_y = (height - inner_h) / 2.0;

        let outer_h = inner_h + (padding_y * 2.0) + config.input.border;
        let outer_w = inner_w + (padding_x * 2.0) + config.input.border;
        let outer_x = (width - outer_w) / 2.0;
        let outer_y = (height - outer_h) / 2.0;

        context.save()?;

        // Draw the outer rectangle, including padding
        // Outer rectangle should have rounded corners
        Self::draw_rounded_rect(
            context,
            outer_x,
            outer_y,
            outer_w,
            outer_h,
            config.input.radius * outer_h, // radius is relative, Cairo requires absolute
        );
        context.set_source_rgba(
            config.colors.input_bg.r,
            config.colors.input_bg.g,
            config.colors.input_bg.b,
            config.colors.input_bg.a,
        );
        context.fill_preserve()?;
        context.set_source_rgba(
            config.colors.input_border.r,
            config.colors.input_border.g,
            config.colors.input_border.b,
            config.colors.input_border.a,
        );
        context.set_line_width(config.input.border);
        context.stroke_preserve()?;
        context.clip();

        // Clip text to the inner rectangle
        context.rectangle(inner_x, inner_y, inner_w, inner_h);
        context.clip();

        let text = config.input.mask_char.repeat(password_len);
        let ext = context.text_extents(text.as_str())?;
        let text_x = inner_x + (inner_w - ext.width()) / 2.0 - ext.x_bearing();
        let text_y = inner_y + (inner_h - fe.descent()) / 2.0 + fe.ascent() / 2.0;

        // Actually draw the text
        context.set_source_rgba(
            config.colors.text.r,
            config.colors.text.g,
            config.colors.text.b,
            config.colors.text.a,
        );
        context.move_to(text_x, text_y);
        context.show_text(text.as_str())?;

        context.restore()?;

        Ok(())
    }

    pub fn destroy(&mut self) {
        if let Some(lock_surface) = &self.lock_surface {
            lock_surface.destroy();
        }

        self.buffers.iter_mut().for_each(|buf| buf.destroy());
        self.output.release();
    }
}

impl Dispatch<ext_session_lock_surface_v1::ExtSessionLockSurfaceV1, usize> for NLockState {
    fn event(
        state: &mut Self,
        lock_surface: &ext_session_lock_surface_v1::ExtSessionLockSurfaceV1,
        event: <ext_session_lock_surface_v1::ExtSessionLockSurfaceV1 as wayland_client::Proxy>::Event,
        data: &usize,
        _: &wayland_client::Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let ext_session_lock_surface_v1::Event::Configure {
            serial,
            width,
            height,
        } = event
            && let Some(shm) = &state.shm
        {
            let surface = &mut state.surfaces[*data];
            surface.width = Some(width);
            surface.height = Some(height);

            if let Err(e) = surface.calculate_dpi() {
                warn!("Failed to set surface DPI: {e}, using default {DEFAULT_DPI}");
                surface.dpi = Some(DEFAULT_DPI);
            }

            lock_surface.ack_configure(serial);

            let auth_state = state.auth_state.clone().load(Ordering::Relaxed);
            surface.render(&state.config, auth_state, state.password.len(), shm, qh);
        }
    }
}
