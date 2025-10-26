// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2025, Nathan Gill

use tracing::warn;
use wayland_client::{
    Dispatch, QueueHandle, WEnum,
    protocol::{wl_compositor, wl_output, wl_shm, wl_surface},
};
use wayland_protocols::ext::session_lock::v1::client::{
    ext_session_lock_surface_v1, ext_session_lock_v1,
};

use crate::{buffer::NLockBuffer, state::NLockState};

pub struct NLockSurface {
    pub created: bool,
    pub index: usize,
    pub output_name: Option<String>,
    pub output_scale: i32,
    pub width: Option<u32>,
    pub height: Option<u32>,
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
            subpixel: None,
            surface: None,
            output,
            lock_surface: None,
            buffers: Vec::new(),
        }
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

                let buf = NLockBuffer::new(shm, width, height, wl_shm::Format::Argb8888, true, qh)?;
                self.buffers.push(buf);

                self.buffers.len() - 1
            }
        };

        Some(idx)
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

    pub fn render(&mut self, shm: &wl_shm::WlShm, qh: &QueueHandle<NLockState>) {
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
        let wl_buffer = match &buffer.buffer {
            Some(wl_buf) => wl_buf,
            None => {
                warn!("wl_buffer not set when attempting render");
                return;
            }
        };

        surface.set_buffer_scale(self.output_scale);
        surface.attach(Some(wl_buffer), 0, 0);
        surface.damage(0, 0, i32::MAX, i32::MAX);
        surface.commit();
    }

    pub fn destroy(&mut self) {
        if let Some(lock_surface) = &self.lock_surface {
            lock_surface.destroy();
        }

        self.buffers.iter_mut().for_each(|buf| buf.destroy());
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
                lock_surface.ack_configure(serial);
                surface.render(shm, qh);
            }
        }
    }
}
