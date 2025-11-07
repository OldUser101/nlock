// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2025, Nathan Gill

pub mod auth;
pub mod buffer;
pub mod event;
pub mod seat;
pub mod state;
pub mod surface;
pub mod util;

use std::sync::atomic::Ordering;

use crate::{auth::{run_auth_loop, AuthRequest}, state::NLockState};

use anyhow::{Result, bail};
use tracing::{debug, error, warn};
use wayland_client::Connection;
use tokio::sync::mpsc;

async fn start() -> Result<()> {
    let conn = Connection::connect_to_env()?;
    let display = conn.display();

    let (auth_tx, auth_rx) = mpsc::channel::<AuthRequest>(32);

    let mut state = NLockState::new(display, auth_tx.clone());

    let mut event_queue = conn.new_event_queue();
    let qh = event_queue.handle();

    state.get_registry(&qh);
    event_queue.roundtrip(&mut state)?;

    if state.compositor.is_none() {
        bail!("Missing WlCompositor");
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
        if let Err(e) = run_auth_loop(auth_rx).await {
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
    tracing_subscriber::fmt()
        .with_timer(tracing_subscriber::fmt::time::uptime())
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let now = chrono::Local::now();
    debug!("nlock started at {}", now.to_rfc3339());

    if let Err(e) = start().await {
        error!("{:#?}", e);
    }

    let now = chrono::Local::now();
    debug!("nlock exited at {}", now.to_rfc3339());
}
