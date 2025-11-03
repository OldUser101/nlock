// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2025, Nathan Gill

use tracing::debug;
use wayland_client::{
    Connection, Dispatch, QueueHandle, delegate_noop,
    protocol::{
        wl_callback, wl_compositor, wl_display, wl_output, wl_registry, wl_seat,
        wl_shm, wl_shm_pool, wl_surface,
    },
};
use wayland_protocols::ext::session_lock::v1::client::{
    ext_session_lock_manager_v1, ext_session_lock_v1,
};

use crate::{seat::{NLockSeat, NLockXkb}, surface::NLockSurface};

pub struct NLockState {
    pub running: bool,
    pub locked: bool,
    pub unlocked: bool,
    pub display: wl_display::WlDisplay,
    pub registry: Option<wl_registry::WlRegistry>,
    pub compositor: Option<wl_compositor::WlCompositor>,
    pub shm: Option<wl_shm::WlShm>,
    pub r_seat: Option<wl_seat::WlSeat>,
    pub session_lock_manager: Option<ext_session_lock_manager_v1::ExtSessionLockManagerV1>,
    pub session_lock: Option<ext_session_lock_v1::ExtSessionLockV1>,
    pub surfaces: Vec<NLockSurface>,
    pub seat: NLockSeat,
    pub xkb: NLockXkb, 
}

impl NLockState {
    pub fn new(display: wl_display::WlDisplay) -> Self {
        Self {
            running: true,
            locked: false,
            unlocked: false,
            display,
            registry: None,
            compositor: None,
            shm: None,
            r_seat: None,
            session_lock_manager: None,
            session_lock: None,
            surfaces: Vec::new(),
            seat: NLockSeat::default(),
            xkb: NLockXkb::default(),
        }
    }

    pub fn get_registry(&mut self, qh: &QueueHandle<Self>) {
        let registry = self.display.get_registry(qh, ());
        self.registry = Some(registry);
    }

    pub fn lock(&mut self, qh: &QueueHandle<Self>) {
        if let Some(session_lock_manager) = &self.session_lock_manager {
            let session_lock = session_lock_manager.lock(qh, ());
            self.session_lock = Some(session_lock);
        }
    }

    pub fn unlock(&mut self, qh: &QueueHandle<Self>) {
        if let Some(session_lock) = &self.session_lock {
            if self.locked {
                session_lock.unlock_and_destroy();
            } else {
                session_lock.destroy();
            }

            self.surfaces.iter_mut().for_each(|s| s.destroy());

            self.display.sync(qh, ());
            self.session_lock = None;
            self.locked = false;
            self.unlocked = true;

            debug!("Session is unlocked");
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for NLockState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<NLockState>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match &interface[..] {
                "wl_compositor" => {
                    let compositor =
                        registry.bind::<wl_compositor::WlCompositor, _, _>(name, version, qh, ());
                    state.compositor = Some(compositor);
                }
                "wl_shm" => {
                    let shm = registry.bind::<wl_shm::WlShm, _, _>(name, version, qh, ());
                    state.shm = Some(shm);
                }
                "wl_seat" => {
                    let seat = registry.bind::<wl_seat::WlSeat, _, _>(name, version, qh, ());
                    state.r_seat = Some(seat);
                }
                "wl_output" => {
                    let index = state.surfaces.len();
                    let output =
                        registry.bind::<wl_output::WlOutput, _, _>(name, version, qh, index);

                    let surface = NLockSurface::new(output, index);
                    state.surfaces.push(surface);
                }
                "ext_session_lock_manager_v1" => {
                    let session_lock_manager = registry
                        .bind::<ext_session_lock_manager_v1::ExtSessionLockManagerV1, _, _>(
                        name,
                        version,
                        qh,
                        (),
                    );
                    state.session_lock_manager = Some(session_lock_manager);
                }
                _ => {}
            }
        }
    }
}

delegate_noop!(NLockState: ignore wl_compositor::WlCompositor);
delegate_noop!(NLockState: ignore wl_shm::WlShm);
delegate_noop!(NLockState: ignore wl_surface::WlSurface);
delegate_noop!(NLockState: ignore ext_session_lock_manager_v1::ExtSessionLockManagerV1);
delegate_noop!(NLockState: ignore wl_callback::WlCallback);
delegate_noop!(NLockState: ignore wl_shm_pool::WlShmPool);

impl Dispatch<ext_session_lock_v1::ExtSessionLockV1, ()> for NLockState {
    fn event(
        state: &mut Self,
        _: &ext_session_lock_v1::ExtSessionLockV1,
        event: <ext_session_lock_v1::ExtSessionLockV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            ext_session_lock_v1::Event::Locked => {
                state.locked = true;

                debug!("Session is locked");
            }
            ext_session_lock_v1::Event::Finished => {
                state.unlock(qh);
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_output::WlOutput, usize> for NLockState {
    fn event(
        state: &mut Self,
        _: &wl_output::WlOutput,
        event: <wl_output::WlOutput as wayland_client::Proxy>::Event,
        data: &usize,
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_output::Event::Geometry {
                x: _,
                y: _,
                physical_width,
                physical_height,
                subpixel,
                make: _,
                model: _,
                transform: _,
            } => {
                state.surfaces[*data].subpixel = Some(subpixel);
                state.surfaces[*data].physical_width = Some(physical_width);
                state.surfaces[*data].physical_height = Some(physical_height);
            }
            wl_output::Event::Name { name } => {
                debug!("Found output '{name}'");
                state.surfaces[*data].output_name = Some(name);
            }
            wl_output::Event::Scale { factor } => {
                state.surfaces[*data].output_scale = factor;
            }
            wl_output::Event::Done => {
                if let (Some(compositor), Some(session_lock)) =
                    (&state.compositor, &state.session_lock)
                {
                    state.surfaces[*data].create_surface(compositor, session_lock, qh);
                }
            }
            _ => {}
        }
    }
}

