// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026, Nathan Gill

use std::{
    os::raw::c_int,
    sync::{Arc, OnceLock},
};

use anyhow::{Result, anyhow};
use nix::sys::signal::{SigHandler, Signal};
use tracing::debug;

use crate::comm::PipeCommChannel;

/// PipeCommChannel is async-signal-safe, only using read/write syscalls
/// Unit type only, since only a notification is needed, not actual data
static DEBUG_COMM: OnceLock<Arc<PipeCommChannel<()>>> = OnceLock::new();

/// Handle a debug signal, probably SIGUSR1
/// This function is async-signal-safe, but not reentrant, thus should
/// only be registered for one signal at a time, for which POSIX mandates
/// it won't be interrupted by itself.
extern "C" fn handle_debug(_: c_int) {
    if let Some(comm) = DEBUG_COMM.get() {
        let _ = comm.write(());
    }
}

/// Register a pipe comm channel to use as a signal callback, typically SIGUSR1
pub fn install_debug_handler(comm: Arc<PipeCommChannel<()>>) -> Result<()> {
    if DEBUG_COMM.set(comm).is_err() {
        return Err(anyhow!(
            "Failed to set debug comm channel, already initialised"
        ));
    }

    let handler = SigHandler::Handler(handle_debug);
    unsafe { nix::sys::signal::signal(Signal::SIGUSR1, handler) }?;

    debug!(
        "Debug handler set up for SIGUSR1 ({})",
        Signal::SIGUSR1 as c_int
    );

    Ok(())
}
