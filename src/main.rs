// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026, Nathan Gill

pub mod args;
pub mod auth;
pub mod buffer;
pub mod cairo_ext;
pub mod comm;
pub mod config;
pub mod event;
pub mod render;
pub mod seat;
pub mod state;
pub mod surface;
pub mod util;

use std::sync::{Arc, atomic::Ordering};

use anyhow::{Result, bail};

#[cfg_attr(debug_assertions, allow(unused_imports))]
use nix::sys::prctl;

use tracing::{debug, error, warn};
use wayland_client::Connection;

use crate::{
    args::run_cli,
    auth::{AuthChannel, AuthConfig, run_auth_loop},
    config::NLockConfig,
    state::NLockState,
};

fn start(config: NLockConfig) -> Result<()> {
    // Prevent ptrace from attaching to nlock
    // Only do this in release config
    #[cfg(not(debug_assertions))]
    prctl::set_dumpable(false)?;

    let conn = Connection::connect_to_env()?;
    let display = conn.display();

    let auth_comm = Arc::new(AuthChannel::new()?);
    let auth_config = AuthConfig::new(&config);

    let mut state = NLockState::new(config, display, auth_comm.clone())?;

    let mut event_queue = conn.new_event_queue();
    let qh = event_queue.handle();

    state.get_registry(&qh);
    event_queue.roundtrip(&mut state)?;

    if state.compositor.is_none() {
        bail!("Missing WlCompositor");
    }

    if state.subcompositor.is_none() {
        bail!("Missing WlSubcompositor");
    }

    if state.shm.is_none() {
        bail!("Missing WlShm");
    }

    if state.r_seat.is_none() {
        bail!("Missing WlSeat");
    }

    if state.session_lock_manager.is_none() {
        bail!("Missing ExtSessionLockManagerV1");
    }

    // spawn authenticator loop in another thread
    std::thread::spawn({
        let auth_comm = auth_comm.clone();
        move || {
            if let Err(e) = run_auth_loop(auth_config, auth_comm) {
                warn!("Error in auth thread: {e}");
            }
            debug!("Auth thread exited");
        }
    });

    state.lock(&qh);

    while state.running.load(Ordering::Relaxed) {
        if let Err(e) = state.event_loop_cycle(&mut event_queue) {
            warn!("Error while running event loop: {e}");
        }
    }

    state.unlock(&qh);
    event_queue.roundtrip(&mut state)?;

    if let Err(e) = auth_comm.stop_ev.write(1) {
        warn!("Failed to stop auth loop: {e}");
    }

    Ok(())
}

fn main() {
    let args = run_cli();

    tracing_subscriber::fmt()
        .with_timer(tracing_subscriber::fmt::time::uptime())
        .with_max_level(args.log_level)
        .init();

    let now = chrono::Local::now();
    debug!("nlock started at {}", now.to_rfc3339());

    match NLockConfig::load(&args) {
        Ok(cfg) => {
            if let Err(e) = start(cfg) {
                error!("{:#?}", e);
            }
        }
        Err(e) => {
            error!("Error loading configuration: {:#?}", e);
            return;
        }
    }

    let now = chrono::Local::now();
    debug!("nlock exited at {}", now.to_rfc3339());
}
