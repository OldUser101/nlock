// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2025, Nathan Gill

use anyhow::{Result, anyhow};
use tracing::warn;
use wayland_client::{
    Dispatch, QueueHandle, WEnum,
    protocol::{wl_compositor, wl_output, wl_shm, wl_surface},
};
use wayland_protocols::ext::session_lock::v1::client::{
    ext_session_lock_surface_v1, ext_session_lock_v1,
};

use crate::{buffer::NLockBuffer, state::NLockState};

const DEFAULT_DPI: f64 = 96.0;
const DEFAULT_FONT_SIZE: f64 = 72.0;
const DEFAULT_LINE_WIDTH: f64 = 25.0;

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

    fn clear_surface(&self, context: &cairo::Context) -> Result<()> {
        context.save()?;
        context.set_source_rgb(0.0, 0.0, 0.0);
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

    fn configure_cairo_font(&self, context: &cairo::Context) -> Result<()> {
        let mut fo = cairo::FontOptions::new()?;
        fo.set_hint_style(cairo::HintStyle::Full);
        fo.set_antialias(cairo::Antialias::Subpixel);
        fo.set_subpixel_order(self.get_cairo_subpixel_order());

        context.set_font_options(&fo);
        context.select_font_face(
            "monospace",
            cairo::FontSlant::Normal,
            cairo::FontWeight::Bold,
        );

        let dpi = self.dpi.unwrap_or(DEFAULT_DPI);
        context.set_font_size((DEFAULT_FONT_SIZE / 72.0) * dpi);

        Ok(())
    }

    fn get_buffer_idx(
        &mut self,
        shm: &wl_shm::WlShm,
        qh: &QueueHandle<NLockState>,
    ) -> Option<usize> {
        if self.width.is_none() || self.height.is_none() {
            return None;
        }

        let index = self
            .buffers
            .iter()
            .position(|buf| buf.state.lock().map(|state| !state.in_use).unwrap_or(false));

        let idx = match index {
            Some(i) => i,
            None => {
                let width = self.width.unwrap() as i32;
                let height = self.height.unwrap() as i32;

                let mut buf =
                    NLockBuffer::new(shm, width, height, wl_shm::Format::Argb8888, true, qh)?;
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

    pub fn render(&mut self, password_len: usize, shm: &wl_shm::WlShm, qh: &QueueHandle<NLockState>) {
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
        if let Err(e) = self.render_frame(password_len, context) {
            warn!("Error while rendering: {e}");
        }

        surface.set_buffer_scale(self.output_scale);
        surface.attach(Some(wl_buffer), 0, 0);
        surface.damage(0, 0, i32::MAX, i32::MAX);
        surface.commit();
    }

    fn render_frame(&self, password_len: usize, context: &cairo::Context) -> Result<()> {
        self.configure_cairo_font(context)?;
        self.clear_surface(context)?;

        context.save()?;
        context.set_source_rgb(0.0, 0.0, 0.0);
        context.set_line_width(DEFAULT_LINE_WIDTH);
        context.rectangle(
            0.0,
            0.0,
            self.width.unwrap() as f64,
            self.height.unwrap() as f64,
        );
        context.stroke()?;
        context.restore()?;

        let fe = context.font_extents()?;

        let box_h = fe.height() * 1.5;
        let box_w = self.width.unwrap() as f64 * 0.5;
        let box_x = (self.width.unwrap() as f64 - box_w) / 2.0;
        let box_y = (self.height.unwrap() as f64 - box_h) / 2.0;

        context.save()?;
        context.rectangle(box_x, box_y, box_w, box_h);
        context.clip();

        let text = "*".repeat(password_len);
        let ext = context.text_extents(text.as_str())?;
        let text_x = box_x + (box_w - ext.width()) / 2.0 - ext.x_bearing();
        let text_y = box_y + (box_h - fe.descent()) / 2.0 + fe.ascent() / 2.0;

        context.set_source_rgb(1.0, 1.0, 1.0);
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
        {
            if let Some(shm) = &state.shm {
                let surface = &mut state.surfaces[*data];
                surface.width = Some(width);
                surface.height = Some(height);

                if let Err(e) = surface.calculate_dpi() {
                    warn!("Failed to set surface DPI: {e}, using default {DEFAULT_DPI}");
                    surface.dpi = Some(DEFAULT_DPI);
                }

                lock_surface.ack_configure(serial);
                surface.render(state.password.len(), shm, qh);
            }
        }
    }
}
