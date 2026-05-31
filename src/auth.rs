// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026, Nathan Gill

use std::{os::fd::AsFd, sync::Arc, thread::JoinHandle};

use anyhow::{Result, anyhow};
use atomic_enum::atomic_enum;
use nix::{
    errno::Errno,
    poll::{PollFd, PollFlags, PollTimeout},
};
use tracing::{debug, warn};
use zeroize::Zeroizing;

use crate::{auth_sys::AuthClient, comm::PipeCommChannel};

pub struct AuthChannel {
    pub request: PipeCommChannel<String>,
    pub response: PipeCommChannel<bool>,
    pub stop: PipeCommChannel<bool>,
}

impl AuthChannel {
    pub fn new() -> Result<Self> {
        Ok(Self {
            request: PipeCommChannel::new()?,
            response: PipeCommChannel::new()?,
            stop: PipeCommChannel::new()?,
        })
    }
}

#[atomic_enum]
pub enum AuthState {
    Idle,
    Success,
    Fail,
}

#[cfg_attr(not(target_os = "linux"), allow(unused_variables))]
fn authenticate(client: &mut AuthClient, password: Zeroizing<String>) -> Result<()> {
    client.set_password(password);
    client.authenticate()?;
    Ok(())
}

/// Handle an authentication request, returning a value to indicate success
fn handle_auth_request(client: &mut AuthClient, auth_comm: Arc<AuthChannel>) -> bool {
    let pwd = match auth_comm.request.read().map(Zeroizing::new) {
        Ok(p) => p,
        Err(e) => {
            warn!("Auth comm error: {e}");
            return false;
        }
    };

    match authenticate(client, pwd) {
        Ok(()) => true,
        Err(e) => {
            warn!("Auth failed: {e}");
            false
        }
    }
}

fn auth_loop(mut client: AuthClient, auth_comm: Arc<AuthChannel>) -> Result<()> {
    let mut success = false;

    loop {
        let req_fd = PollFd::new(auth_comm.request.rx().as_fd(), PollFlags::POLLIN);
        let stop_fd = PollFd::new(auth_comm.stop.rx().as_fd(), PollFlags::POLLIN);

        let mut events = [req_fd, stop_fd];

        match nix::poll::poll(&mut events, PollTimeout::NONE) {
            Ok(_) => {
                // stop events take priority over auth
                if events[1].any().unwrap_or_default() {
                    debug!("Received stop, exiting");
                    break;
                }

                // auth was requested for a password
                if events[0].any().unwrap_or_default() && !success {
                    success = handle_auth_request(&mut client, auth_comm.clone());

                    // dump auth result in response pipe
                    if let Err(e) = auth_comm.response.write(success) {
                        warn!("Failed to write auth response: {e}");
                    }
                }
            }
            Err(Errno::EINTR) => continue,
            Err(e) => return Err(anyhow!("poll failed: {e}")),
        }
    }

    Ok(())
}

pub fn setup_auth(auth_comm: Arc<AuthChannel>) -> Result<JoinHandle<()>> {
    let client = AuthClient::new("nlock")?;

    let handle = std::thread::spawn({
        move || {
            if let Err(e) = auth_loop(client, auth_comm) {
                warn!("Error in auth thread: {e}");
            }
            debug!("Auth thread exited");
        }
    });

    Ok(handle)
}
