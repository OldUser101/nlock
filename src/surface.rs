// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026, Nathan Gill

use std::sync::atomic::Ordering;

use anyhow::{Result, anyhow, bail};
use tracing::{debug, trace, warn};
use wayland_client::{
    Dispatch, QueueHandle,
    protocol::{wl_compositor, wl_output, wl_shm, wl_subcompositor, wl_subsurface, wl_surface},
};
use wayland_protocols::ext::session_lock::v1::client::{
    ext_session_lock_surface_v1, ext_session_lock_v1,
};

use crate::{
    auth::AuthState,
    buffer::NLockBuffer,
    config::NLockConfig,
    render::{DEFAULT_DPI, NLockRenderBackgroundArgs, NLockRenderOverlayArgs, NLockRenderer},
    state::NLockState,
};

pub struct NLockSurface {
    pub created: bool,
    // Background rendering is expensive, only do it once.
    pub bg_rendered: bool,
    pub index: usize,
    pub output_name: Option<String>,
    pub output_scale: i32,

    width: Option<u32>,
    height: Option<u32>,
    last_width: Option<u32>,
    last_height: Option<u32>,
    physical_width: Option<i32>,
    physical_height: Option<i32>,

    dpi: Option<f64>,
    subpixel: Option<cairo::SubpixelOrder>,

    renderer: NLockRenderer,

    pub ov_surface: Option<wl_surface::WlSurface>,
    pub bg_surface: Option<wl_surface::WlSurface>,
    pub subsurface: Option<wl_subsurface::WlSubsurface>,
    pub output: wl_output::WlOutput,
    pub lock_surface: Option<ext_session_lock_surface_v1::ExtSessionLockSurfaceV1>,
    pub buffers: Vec<NLockBuffer>,
}

impl NLockSurface {
    pub fn new(output: wl_output::WlOutput, index: usize) -> Self {
        Self {
            created: false,
            bg_rendered: false,
            index,
            output_name: None,
            output_scale: 1,
            width: None,
            height: None,
            last_width: None,
            last_height: None,
            physical_width: None,
            physical_height: None,
            dpi: None,
            renderer: NLockRenderer::default(),
            subpixel: None,
            ov_surface: None,
            bg_surface: None,
            subsurface: None,
            output,
            lock_surface: None,
            buffers: Vec::new(),
        }
    }

    /// Get the **scaled** dimensions of the current output
    pub fn get_dimensions<T>(&self) -> Result<(T, T)>
    where
        u32: Into<T>,
    {
        let width = self.width.ok_or(anyhow!("Surface width not set"))?;
        let height = self.height.ok_or(anyhow!("Surface height not set"))?;

        if width == 0 || height == 0 {
            bail!("Surface dimensions invalid: {}x{}", width, height);
        }

        if self.output_scale <= 0 {
            bail!("Output scale {} is invalid", self.output_scale);
        }

        let width = width * self.output_scale as u32;
        let height = height * self.output_scale as u32;

        Ok((width.into(), height.into()))
    }

    pub fn set_raw_dimensions(&mut self, width: u32, height: u32) -> Result<()> {
        if width == 0 || height == 0 {
            bail!("Surface dimensions invalid: {}x{}", width, height);
        }

        self.width = Some(width);
        self.height = Some(height);

        Ok(())
    }

    pub fn get_raw_dimensions<T>(&self) -> Result<(T, T)>
    where
        u32: Into<T>,
    {
        let width = self.width.ok_or(anyhow!("Surface width not set"))?;
        let height = self.height.ok_or(anyhow!("Surface height not set"))?;

        if width == 0 || height == 0 {
            bail!("Surface dimensions invalid: {}x{}", width, height);
        }

        Ok((width.into(), height.into()))
    }

    pub fn get_physical_dimensions<T>(&self) -> Result<(T, T)>
    where
        i32: Into<T>,
    {
        let width = self
            .physical_width
            .ok_or(anyhow!("Output physical width not set"))?;
        let height = self
            .physical_height
            .ok_or(anyhow!("Output physical height not set"))?;

        if width <= 0 || height <= 0 {
            bail!("Output physical dimensions invalid: {}x{}", width, height);
        }

        Ok((width.into(), height.into()))
    }

    pub fn set_physical_dimensions(&mut self, width: i32, height: i32) -> Result<()> {
        if width <= 0 || height <= 0 {
            bail!("Output physical dimensions invalid: {}x{}", width, height);
        }

        self.physical_width = Some(width);
        self.physical_height = Some(height);

        Ok(())
    }

    pub fn set_subpixel_order(&mut self, order: cairo::SubpixelOrder) {
        self.subpixel = Some(order);
        self.renderer.set_subpixel_order(order);
    }

    fn update_last_dimensions(&mut self) -> Result<()> {
        let (width, height) = self.get_dimensions::<u32>()?;

        self.last_width = Some(width);
        self.last_height = Some(height);

        Ok(())
    }

    fn new_buffer(
        &mut self,
        width: u32,
        height: u32,
        shm: &wl_shm::WlShm,
        qh: &QueueHandle<NLockState>,
    ) -> Option<usize> {
        let buf = NLockBuffer::new(
            shm,
            width as i32,
            height as i32,
            wl_shm::Format::Argb8888,
            qh,
        )?;

        self.buffers.push(buf);

        debug!(
            "Allocated buffer {} dim. {}x{}",
            self.buffers.len() - 1,
            width,
            height
        );

        Some(self.buffers.len() - 1)
    }

    fn get_buffer_idx(
        &mut self,
        shm: &wl_shm::WlShm,
        qh: &QueueHandle<NLockState>,
    ) -> Option<usize> {
        let (width, height) = self.get_dimensions::<u32>().ok()?;

        // The surface size changed, new buffers needed
        if let Some(last_width) = self.last_width
            && let Some(last_height) = self.last_height
            && (last_width != width || last_height != height)
        {
            return self.new_buffer(width, height, shm, qh);
        }

        let index = self
            .buffers
            .iter()
            .position(|buf| !buf.state.in_use.load(Ordering::Acquire));

        let idx = match index {
            Some(i) => i,
            None => self.new_buffer(width, height, shm, qh)?,
        };

        Some(idx)
    }

    pub fn calculate_dpi(&mut self) {
        let dpi = (|| {
            let (width, height) = self.get_raw_dimensions::<f64>().ok()?;
            let (phys_width, phys_height) = self.get_physical_dimensions::<f64>().ok()?;

            let dpi_x = width / (phys_width / 25.4);
            let dpi_y = height / (phys_height / 25.4);
            let dpi = (dpi_x + dpi_y) / 2.0;

            debug!(
                "Got DPI {}: W H PW PH: {} {} {} {}",
                dpi, width, height, phys_width, phys_height
            );

            if dpi.is_finite() { Some(dpi) } else { None }
        })()
        .unwrap_or(DEFAULT_DPI);

        self.dpi = Some(dpi);
        self.renderer.set_dpi(dpi);
    }

    pub fn create_surface(
        &mut self,
        compositor: &wl_compositor::WlCompositor,
        subcompositor: &wl_subcompositor::WlSubcompositor,
        session_lock: &ext_session_lock_v1::ExtSessionLockV1,
        qh: &QueueHandle<NLockState>,
    ) {
        if !self.created {
            let bg_surface = compositor.create_surface(qh, ());
            let ov_surface = compositor.create_surface(qh, ());
            let subsurface = subcompositor.get_subsurface(&ov_surface, &bg_surface, qh, ());

            // Pass all input to the main surface, this feels a bit hacky
            let region = compositor.create_region(qh, ());
            region.add(0, 0, 0, 0);
            ov_surface.set_input_region(Some(&region));

            self.bg_surface = Some(bg_surface);
            self.ov_surface = Some(ov_surface);
            self.subsurface = Some(subsurface);

            if let Some(surface) = &self.bg_surface
                && self.ov_surface.is_some()
                && self.subsurface.is_some()
            {
                let lock_surface =
                    session_lock.get_lock_surface(surface, &self.output, qh, self.index);
                self.lock_surface = Some(lock_surface);
            } else {
                warn!("Failed to create background, overlay, or sub surface");
            }

            self.created = true;
        }
    }

    pub fn render(
        &mut self,
        config: &NLockConfig,
        auth_state: AuthState,
        pwd_len: usize,
        bg_image: Option<&cairo::ImageSurface>,
        shm: &wl_shm::WlShm,
        qh: &QueueHandle<NLockState>,
    ) {
        // DPI used in font scaling, uses default if not set, effectively
        // disabling scaling.
        if self.dpi.is_none() && config.font.use_dpi_scaling {
            self.calculate_dpi();
        }

        if let Err(e) = self.render_overlay(config, auth_state, pwd_len, shm, qh) {
            warn!("Error while rendering overlay: {e}");
        }

        if let Err(e) = self.render_background(config, bg_image, shm, qh) {
            warn!("Error while rendering background: {e}");
        }

        // Update last width and height to allow for resizing
        if let Err(e) = self.update_last_dimensions() {
            warn!("Failed to update previous dimensions: {e}");
        }
    }

    fn render_background(
        &mut self,
        config: &NLockConfig,
        bg_image: Option<&cairo::ImageSurface>,
        shm: &wl_shm::WlShm,
        qh: &QueueHandle<NLockState>,
    ) -> Result<()> {
        // Background rendered, but we need to commit again
        // to allow overlay update (synchronised surfaces)
        if self.bg_rendered
            && let Some(surface) = &self.bg_surface
        {
            surface.commit();
            return Ok(());
        }

        let (buf_width, buf_height) = self.get_dimensions::<f64>()?;

        let idx = match self.get_buffer_idx(shm, qh) {
            Some(i) => i,
            None => {
                bail!("Failed to obtain buffer for rendering background");
            }
        };

        trace!("got buffer index {} for background", idx);

        let surface = match &self.bg_surface {
            Some(s) => s,
            None => {
                bail!("wl_surface not set when attempting background render");
            }
        };

        let buffer = &self.buffers[idx];
        let context = &buffer.context;

        context.save()?;
        self.renderer.render_background(
            config,
            NLockRenderBackgroundArgs {
                buf_height,
                buf_width,
                context,
                image: bg_image,
            },
        )?;
        context.restore()?;

        let mut buf_guard = buffer
            .lock_buffer()
            .ok_or(anyhow!("Failed to lock buffer {}", idx))?;
        buf_guard.commit_to(surface, self.output_scale);

        // Avoid rendering the background again
        self.bg_rendered = true;

        Ok(())
    }

    fn render_overlay(
        &mut self,
        config: &NLockConfig,
        auth_state: AuthState,
        pwd_len: usize,
        shm: &wl_shm::WlShm,
        qh: &QueueHandle<NLockState>,
    ) -> Result<()> {
        let (buf_width, buf_height) = self.get_dimensions::<f64>()?;

        let idx = match self.get_buffer_idx(shm, qh) {
            Some(i) => i,
            None => {
                bail!("Failed to obtain buffer for rendering overlay");
            }
        };

        trace!("got buffer index {} for overlay", idx);

        let surface = match &self.ov_surface {
            Some(s) => s,
            None => {
                bail!("wl_surface not set when attempting overlay render");
            }
        };

        let subsurface = match &self.subsurface {
            Some(s) => s,
            None => {
                bail!("wl_subsurface not set when attempting overlay render");
            }
        };

        let buffer = &self.buffers[idx];
        let context = &buffer.context;

        // Save context to ensure transformations don't leak
        context.save()?;
        self.renderer.render_overlay(
            config,
            NLockRenderOverlayArgs {
                auth_state,
                buf_height,
                buf_width,
                context,
                pwd_len,
            },
        )?;
        context.restore()?;

        // Ensure subsurface position is always set to 0,0
        subsurface.set_position(0, 0);

        let mut buf_guard = buffer
            .lock_buffer()
            .ok_or(anyhow!("Failed to lock buffer {}", idx))?;
        buf_guard.commit_to(surface, self.output_scale);

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

            if let Err(e) = surface.set_raw_dimensions(width, height) {
                warn!("Failed to set surface dimensions: {e}");
                return;
            }

            lock_surface.ack_configure(serial);

            let auth_state = state.auth_state.clone().load(Ordering::Relaxed);
            surface.render(
                &state.config,
                auth_state,
                state.password.len(),
                state.background_image.as_ref(),
                shm,
                qh,
            );
        }
    }
}
