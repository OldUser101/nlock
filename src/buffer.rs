// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026, Nathan Gill

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use tracing::{trace, warn};
use wayland_client::{
    Dispatch, QueueHandle,
    protocol::{wl_buffer, wl_shm, wl_surface},
};

use crate::{shm::NLockShm, state::NLockState, surface::NLockSurfaceTracking};

pub struct NLockBuffer {
    buffer: wl_buffer::WlBuffer,

    // required to keep shm mapping alive
    _shm: NLockShm,

    pub width: i32,
    pub height: i32,
    pub state: Arc<NLockBufferState>,
    pub surface: cairo::ImageSurface,
    pub context: cairo::Context,
}

pub struct NLockBufferState {
    pub in_use: AtomicBool,
}

pub struct NLockCommitArgs<'a> {
    pub surface: &'a wl_surface::WlSurface,
    pub scale: i32,
    pub output: u32,
    pub tracking: &'a mut NLockSurfaceTracking,
}

pub struct NLockBufferGuard<'a> {
    wl_buffer: &'a wl_buffer::WlBuffer,
    state: &'a Arc<NLockBufferState>,
    committed: bool,
}

impl<'a> NLockBufferGuard<'a> {
    /// Attaches, damages, and commits the current buffer onto the specified
    /// surface.
    pub fn commit_to(&mut self, args: NLockCommitArgs, qh: &QueueHandle<NLockState>) {
        args.surface.set_buffer_scale(args.scale);
        args.surface.attach(Some(self.wl_buffer), 0, 0);
        args.surface.damage(0, 0, i32::MAX, i32::MAX);
        args.surface.frame(qh, args.output);
        args.surface.commit();

        self.committed = true;
        args.tracking.ready = false;
        args.tracking.dirty = false;
    }
}

impl<'a> Drop for NLockBufferGuard<'a> {
    fn drop(&mut self) {
        if !self.committed {
            self.state.in_use.store(false, Ordering::Release);
        }
    }
}

impl NLockBuffer {
    pub fn new(
        wl_shm: &wl_shm::WlShm,
        width: i32,
        height: i32,
        format: wl_shm::Format,
        qh: &QueueHandle<NLockState>,
    ) -> Option<Self> {
        if width <= 0 || height <= 0 {
            warn!(
                "cannot create a buffer with dimensions: {}x{}",
                width, height
            );
            return None;
        }

        let stride = width * 4;
        let size = stride * height;

        let mut shm = NLockShm::new(size as i64)?;
        let data = shm.map().ok()?;

        let state = Arc::new(NLockBufferState {
            in_use: AtomicBool::new(false),
        });

        let pool = wl_shm.create_pool(shm.fd(), size, qh, ());
        let buffer = pool.create_buffer(0, width, height, stride, format, qh, state.clone());

        pool.destroy();

        let surface = unsafe {
            cairo::ImageSurface::create_for_data_unsafe(
                data.as_ptr() as *mut u8,
                cairo::Format::ARgb32,
                width,
                height,
                stride,
            )
        }
        .ok()?;

        let context = cairo::Context::new(&surface).ok()?;

        Some(Self {
            buffer,
            _shm: shm,
            width,
            height,
            state,
            surface,
            context,
        })
    }

    pub fn lock_buffer(&self) -> Option<NLockBufferGuard<'_>> {
        if self.state.in_use.swap(true, Ordering::AcqRel) {
            None
        } else {
            // Buffer is now "in_use", explicitly manage state
            Some(NLockBufferGuard {
                wl_buffer: &self.buffer,
                state: &self.state,
                committed: false,
            })
        }
    }
}

impl Drop for NLockBuffer {
    fn drop(&mut self) {
        self.buffer.destroy();
    }
}

impl Dispatch<wl_buffer::WlBuffer, Arc<NLockBufferState>> for NLockState {
    fn event(
        _: &mut Self,
        _: &wl_buffer::WlBuffer,
        event: <wl_buffer::WlBuffer as wayland_client::Proxy>::Event,
        data: &Arc<NLockBufferState>,
        _: &wayland_client::Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_buffer::Event::Release = event {
            trace!("release {:p}", Arc::as_ptr(data),);
            data.in_use.store(false, Ordering::Release);
        }
    }
}
