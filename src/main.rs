// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2025, Nathan Gill

pub mod args;
pub mod auth;
pub mod buffer;
pub mod config;
pub mod event;
pub mod image;
pub mod seat;
pub mod state;
pub mod surface;
pub mod util;

use std::sync::atomic::Ordering;

use crate::{
    args::run_cli,
    auth::{AuthConfig, AuthRequest, run_auth_loop},
    config::NLockConfig,
    state::NLockState,
};

use anyhow::{Result, bail};

#[cfg_attr(debug_assertions, allow(unused_imports))]
use nix::sys::prctl;

use tokio::sync::mpsc;
use tracing::{debug, error, warn};
use wayland_client::Connection;

async fn start(config: NLockConfig) -> Result<()> {
    // Prevent ptrace from attaching to nlock
    // Only do this in release config
    #[cfg(not(debug_assertions))]
    prctl::set_dumpable(false)?;

    let conn = Connection::connect_to_env()?;
    let display = conn.display();

    let (auth_tx, auth_rx) = mpsc::channel::<AuthRequest>(32);
    let auth_config = AuthConfig::new(&config);

    let mut state = NLockState::new(config, display, auth_tx.clone())?;

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

    tokio::spawn(async move {
        if let Err(e) = run_auth_loop(auth_config, auth_rx).await {
            warn!("Error in auth thread: {e}");
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

    auth_tx.send(AuthRequest::Exit).await.unwrap();

    Ok(())
}

#[tokio::main()]
async fn main() {
    let args = run_cli();

    tracing_subscriber::fmt()
        .with_timer(tracing_subscriber::fmt::time::uptime())
        .with_max_level(args.log_level.to_level())
        .init();

    let now = chrono::Local::now();
    debug!("nlock started at {}", now.to_rfc3339());

    match NLockConfig::load(&args) {
        Ok(cfg) => {
            if let Err(e) = start(cfg).await {
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
