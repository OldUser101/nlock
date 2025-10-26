// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2025, Nathan Gill

use nix::{
    sys::mman::{MapFlags, ProtFlags, mmap, munmap},
    unistd::ftruncate,
};
use std::{
    os::{fd::AsFd, raw::c_void},
    ptr::NonNull,
    sync::{Arc, Mutex},
};
use wayland_client::{
    Dispatch, QueueHandle,
    protocol::{wl_buffer, wl_shm},
};

use crate::{state::NLockState, util::open_shm};

pub struct NLockBuffer {
    pub buffer: Option<wl_buffer::WlBuffer>,
    pub data: NonNull<c_void>,
    pub width: i32,
    pub height: i32,
    pub size: usize,
    pub state: Arc<Mutex<NLockBufferState>>,
}

#[derive(Default)]
pub struct NLockBufferState {
    pub in_use: bool,
}

impl NLockBuffer {
    pub fn new(
        shm: &wl_shm::WlShm,
        width: i32,
        height: i32,
        format: wl_shm::Format,
        in_use: bool,
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

        let state = Arc::new(Mutex::new(NLockBufferState { in_use }));

        let pool = shm.create_pool(fd.as_fd(), size, qh, ());
        let buffer = pool.create_buffer(0, width, height, stride, format, qh, state.clone());

        pool.destroy();

        Some(Self {
            buffer: Some(buffer),
            data,
            width,
            height,
            size: size as usize,
            state: state.clone(),
        })
    }

    pub fn destroy(&mut self) {
        if let Some(buffer) = &self.buffer {
            buffer.destroy();
        }

        let _ = unsafe { munmap(self.data, self.size) };
    }
}

impl Dispatch<wl_buffer::WlBuffer, Arc<Mutex<NLockBufferState>>> for NLockState {
    fn event(
        _: &mut Self,
        _: &wl_buffer::WlBuffer,
        event: <wl_buffer::WlBuffer as wayland_client::Proxy>::Event,
        data: &Arc<Mutex<NLockBufferState>>,
        _: &wayland_client::Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_buffer::Event::Release = event {
            if let Ok(mut state) = data.lock() {
                state.in_use = false;
            }
        }
    }
}
