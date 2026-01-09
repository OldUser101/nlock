// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026, Nathan Gill

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use anyhow::{Result, anyhow, bail};
use cairo::ImageSurface;
use gdk_pixbuf::Pixbuf;
use mio::Poll;
use nix::sys::eventfd::EventFd;
use nix::sys::timerfd::TimerFd;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, warn};
use wayland_client::protocol::{wl_region, wl_subcompositor, wl_subsurface};
use wayland_client::{
    Connection, Dispatch, QueueHandle, delegate_noop,
    protocol::{
        wl_callback, wl_compositor, wl_display, wl_output, wl_registry, wl_seat, wl_shm,
        wl_shm_pool, wl_surface,
    },
};
use wayland_protocols::ext::session_lock::v1::client::{
    ext_session_lock_manager_v1, ext_session_lock_v1,
};
use zeroize::Zeroizing;

use crate::auth::{AtomicAuthState, AuthState};
use crate::cairo_ext::{ImageSurfaceExt, SubpixelOrderExt};
use crate::config::NLockConfig;
use crate::util::BackgroundType;
use crate::{
    auth::AuthRequest,
    seat::{NLockSeat, NLockXkb},
    surface::NLockSurface,
};

pub struct NLockState {
    pub config: NLockConfig,
    pub running: Arc<AtomicBool>,
    pub locked: bool,
    pub unlocked: bool,
    pub state_changed: Arc<AtomicBool>,
    pub display: wl_display::WlDisplay,
    pub registry: Option<wl_registry::WlRegistry>,
    pub compositor: Option<wl_compositor::WlCompositor>,
    pub subcompositor: Option<wl_subcompositor::WlSubcompositor>,
    pub shm: Option<wl_shm::WlShm>,
    pub r_seat: Option<wl_seat::WlSeat>,
    pub session_lock_manager: Option<ext_session_lock_manager_v1::ExtSessionLockManagerV1>,
    pub session_lock: Option<ext_session_lock_v1::ExtSessionLockV1>,
    pub surfaces: Vec<NLockSurface>,
    pub seat: NLockSeat,
    pub xkb: NLockXkb,
    pub password: Zeroizing<String>,
    pub poll: Option<Poll>,
    pub timers: Vec<(TimerFd, usize)>,
    pub auth_tx: mpsc::Sender<AuthRequest>,
    pub auth_state: Arc<AtomicAuthState>,
    pub state_ev: Arc<EventFd>,
    pub background_image: Option<cairo::ImageSurface>,
}

impl NLockState {
    pub fn new(
        config: NLockConfig,
        display: wl_display::WlDisplay,
        auth_tx: mpsc::Sender<AuthRequest>,
    ) -> Result<Self> {
        let mut s = Self {
            config,
            running: Arc::new(AtomicBool::new(true)),
            locked: false,
            unlocked: false,
            state_changed: Arc::new(AtomicBool::new(false)),
            display,
            registry: None,
            compositor: None,
            subcompositor: None,
            shm: None,
            r_seat: None,
            session_lock_manager: None,
            session_lock: None,
            surfaces: Vec::new(),
            seat: NLockSeat::default(),
            xkb: NLockXkb::default(),
            password: Zeroizing::new("".to_string()),
            poll: None,
            timers: Vec::new(),
            auth_tx,
            auth_state: Arc::new(AtomicAuthState::new(AuthState::Idle)),
            state_ev: Arc::new(EventFd::new()?),
            background_image: None,
        };

        if let Err(e) = s.try_load_background_image() {
            bail!(
                "Failed to load background image: {}: {}",
                s.config.image.path.display(),
                e
            );
        }

        Ok(s)
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

            self.clear_password();

            debug!("Session is unlocked");
        }
    }

    pub fn clear_password(&mut self) {
        self.password.clear();
    }

    pub fn submit_password(&mut self) {
        let tx_clone = self.auth_tx.clone();
        let password = self.password.clone();
        let running = self.running.clone();
        let state_changed = self.state_changed.clone();
        let state_ev = self.state_ev.clone();
        let auth_state = self.auth_state.clone();

        auth_state.store(AuthState::Idle, Ordering::Relaxed);

        tokio::spawn(async move {
            let (resp_tx, resp_rx) = oneshot::channel();
            if let Err(e) = tx_clone
                .send(AuthRequest::Password(password, resp_tx))
                .await
            {
                warn!("Failed to submit password: {e}");
                return;
            }

            match resp_rx.await {
                Ok(Ok(())) => {
                    debug!("Authentication completed sucecssfully");

                    auth_state.store(AuthState::Success, Ordering::Relaxed);
                    running.store(false, Ordering::Relaxed);
                    let _ = state_ev.write(1);
                }
                Ok(Err(e)) => {
                    debug!("PAM authentication error: {e}");

                    auth_state.store(AuthState::Fail, Ordering::Relaxed);
                    state_changed.store(true, Ordering::Relaxed);
                    let _ = state_ev.write(1);
                }
                Err(e) => warn!("Error receiving from auth thread: {e}"),
            }
        });

        self.clear_password();
    }

    fn try_load_background_image(&mut self) -> Result<()> {
        if self.config.general.bg_type == BackgroundType::Color {
            self.config.general.bg_type = BackgroundType::Color;
            return Ok(());
        }

        let pixbuf = Pixbuf::from_file(self.config.image.path.clone())?;
        let pixbuf = pixbuf
            .apply_embedded_orientation()
            .ok_or(anyhow!("Failed to apply embedded image orientation"))?;
        self.background_image = Some(ImageSurface::create_from_pixbuf(&pixbuf)?);
        self.config.general.bg_type = BackgroundType::Image;

        Ok(())
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
                "wl_subcompositor" => {
                    let subcompositor = registry.bind::<wl_subcompositor::WlSubcompositor, _, _>(
                        name,
                        version,
                        qh,
                        (),
                    );
                    state.subcompositor = Some(subcompositor);
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
delegate_noop!(NLockState: ignore wl_subcompositor::WlSubcompositor);
delegate_noop!(NLockState: ignore wl_shm::WlShm);
delegate_noop!(NLockState: ignore wl_surface::WlSurface);
delegate_noop!(NLockState: ignore wl_subsurface::WlSubsurface);
delegate_noop!(NLockState: ignore ext_session_lock_manager_v1::ExtSessionLockManagerV1);
delegate_noop!(NLockState: ignore wl_callback::WlCallback);
delegate_noop!(NLockState: ignore wl_shm_pool::WlShmPool);
delegate_noop!(NLockState: ignore wl_region::WlRegion);

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
                state.surfaces[*data]
                    .set_subpixel_order(cairo::SubpixelOrder::from_wl_subpixel(subpixel));

                if let Err(e) =
                    state.surfaces[*data].set_physical_dimensions(physical_width, physical_height)
                {
                    warn!("Failed to set output physical dimensions: {e}");
                }
            }
            wl_output::Event::Name { name } => {
                debug!("Found output '{name}'");
                state.surfaces[*data].output_name = Some(name);
            }
            wl_output::Event::Scale { factor } => {
                debug!(
                    "Set output scale for '{}' to {factor}",
                    state.surfaces[*data]
                        .output_name
                        .as_ref()
                        .unwrap_or(&"".to_string())
                );

                state.surfaces[*data].output_scale = factor;
            }
            wl_output::Event::Done => {
                if let (Some(compositor), Some(subcompositor), Some(session_lock)) =
                    (&state.compositor, &state.subcompositor, &state.session_lock)
                {
                    state.surfaces[*data].create_surface(
                        compositor,
                        subcompositor,
                        session_lock,
                        qh,
                    );
                }
            }
            _ => {}
        }
    }
}
