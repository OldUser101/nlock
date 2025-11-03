// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2025, Nathan Gill

pub mod buffer;
pub mod seat;
pub mod state;
pub mod surface;
pub mod util;

use crate::state::NLockState;

use anyhow::{Result, bail};
use tracing::{debug, error};
use wayland_client::Connection;

fn start() -> Result<()> {
    let conn = Connection::connect_to_env()?;
    let display = conn.display();

    let mut state = NLockState::new(display);

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

    if state.seat.is_none() {
        bail!("Missing WlSeat");
    }

    if state.session_lock_manager.is_none() {
        bail!("Missing ExtSessionLockManagerV1");
    }

    state.lock(&qh);

    while state.running {
        event_queue.blocking_dispatch(&mut state)?;
    }

    state.unlock(&qh);
    event_queue.roundtrip(&mut state)?;

    Ok(())
}

fn main() {
    tracing_subscriber::fmt()
        .with_timer(tracing_subscriber::fmt::time::uptime())
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let now = chrono::Local::now();
    debug!("nlock started at {}", now.to_rfc3339());

    if let Err(e) = start() {
        error!("{:#?}", e);
    }

    let now = chrono::Local::now();
    debug!("nlock exited at {}", now.to_rfc3339());
}
