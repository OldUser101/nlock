// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2025, Nathan Gill

use nix::{
    sys::mman::{MapFlags, ProtFlags, mmap, munmap},
    unistd::ftruncate,
};
use std::{
    os::{fd::AsFd, raw::c_void},
    ptr::NonNull,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use wayland_client::{
    Dispatch, QueueHandle,
    protocol::{wl_buffer, wl_shm, wl_surface},
};

use crate::{state::NLockState, util::open_shm};

pub struct NLockBuffer {
    buffer: wl_buffer::WlBuffer,
    data: NonNull<c_void>,

    pub width: i32,
    pub height: i32,
    pub size: usize,
    pub state: Arc<NLockBufferState>,
    pub surface: cairo::ImageSurface,
    pub context: cairo::Context,
}

pub struct NLockBufferState {
    pub in_use: AtomicBool,
}

pub struct NLockBufferGuard<'a> {
    wl_buffer: &'a wl_buffer::WlBuffer,
    state: &'a Arc<NLockBufferState>,
    committed: bool,
}

impl<'a> NLockBufferGuard<'a> {
    /// Attaches, damages, and commits the current buffer onto the specified
    /// surface.
    pub fn commit_to(&mut self, surface: &wl_surface::WlSurface) {
        surface.attach(Some(self.wl_buffer), 0, 0);
        surface.damage(0, 0, i32::MAX, i32::MAX);
        surface.commit();

        self.committed = true;
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
        shm: &wl_shm::WlShm,
        width: i32,
        height: i32,
        format: wl_shm::Format,
        qh: &QueueHandle<NLockState>,
    ) -> Option<Self> {
        let stride = width * 4;
        let size = stride * height;

        let fd = open_shm()?;
        ftruncate(&fd, size as i64).ok()?;

        let data = unsafe {
            mmap(
                None,
                std::num::NonZeroUsize::new_unchecked(size as usize),
                ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
                MapFlags::MAP_SHARED,
                &fd,
                0,
            )
            .ok()?
        };

        let state = Arc::new(NLockBufferState {
            in_use: AtomicBool::new(false),
        });

        let pool = shm.create_pool(fd.as_fd(), size, qh, ());
        let buffer = pool.create_buffer(0, width, height, stride, format, qh, state.clone());

        pool.destroy();

        let surface = unsafe {
            cairo::ImageSurface::create_for_data_unsafe(
                data.as_ptr() as *mut u8,
                cairo::Format::ARgb32,
                width,
                height,
                width * 4,
            )
        }
        .ok()?;

        let context = cairo::Context::new(&surface).ok()?;

        Some(Self {
            buffer,
            data,
            width,
            height,
            size: size as usize,
            state,
            surface,
            context,
        })
    }

    pub fn lock_buffer(&self) -> Option<NLockBufferGuard<'_>> {
        if self.state.in_use.swap(true, Ordering::AcqRel) {
            None
        } else {
            // Buffer is now "in_use", explicit manage state
            Some(NLockBufferGuard {
                wl_buffer: &self.buffer,
                state: &self.state,
                committed: false,
            })
        }
    }

    pub fn destroy(&mut self) {
        self.buffer.destroy();
        let _ = unsafe { munmap(self.data, self.size) };
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
            data.in_use.store(false, Ordering::Release);
        }
    }
}
