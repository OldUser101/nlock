// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026, Nathan Gill

use std::{os::fd::AsFd, sync::Arc};

use anyhow::{Result, anyhow};
use atomic_enum::atomic_enum;
use nix::{
    errno::Errno,
    poll::{PollFd, PollFlags, PollTimeout},
    sys::eventfd::EventFd,
};
use pam_rs::{Client, PamFlag};
use tracing::{debug, warn};
use zeroize::Zeroizing;

use crate::{comm::PipeCommChannel, config::NLockConfig};

pub struct AuthChannel {
    pub request: PipeCommChannel,
    pub response: PipeCommChannel,
    pub stop_ev: EventFd,
}

impl AuthChannel {
    pub fn new() -> Result<Self> {
        Ok(Self {
            request: PipeCommChannel::new()?,
            response: PipeCommChannel::new()?,
            stop_ev: EventFd::new()?,
        })
    }
}

#[atomic_enum]
pub enum AuthState {
    Idle,
    Success,
    Fail,
}

pub struct AuthConfig {
    pub allow_empty: bool,
}

impl AuthConfig {
    pub fn new(config: &NLockConfig) -> Self {
        Self {
            allow_empty: config.general.pwd_allow_empty,
        }
    }
}

fn authenticate(config: &AuthConfig, username: &str, password: Zeroizing<String>) -> Result<()> {
    let mut client = Client::with_password("nlock")?;
    client
        .conversation_mut()
        .set_credentials(username, password.as_str());

    let mut flags = PamFlag::None;
    if !config.allow_empty {
        flags = PamFlag::Disallow_Null_AuthTok;
    }

    client.authenticate(flags)?;

    Ok(())
}

/// Handle an authentication request, returning a value to indicate success
fn handle_auth_request(config: &AuthConfig, auth_comm: Arc<AuthChannel>, username: &str) -> bool {
    let pwd = match auth_comm.request.read_str().map(Zeroizing::new) {
        Ok(p) => p,
        Err(e) => {
            warn!("Auth comm error: {e}");
            return false;
        }
    };

    match authenticate(config, username, pwd) {
        Ok(()) => true,
        Err(e) => {
            warn!("Auth failed: {e}");
            false
        }
    }
}

pub async fn run_auth_loop(config: AuthConfig, auth_comm: Arc<AuthChannel>) -> Result<()> {
    let username = uzers::get_current_username().ok_or(anyhow!("Current user does not exist"))?;
    let username = username.to_string_lossy().to_string();

    debug!("Running authenticator for '{username}'");

    let mut success = false;

    loop {
        let req_fd = PollFd::new(auth_comm.request.rx().as_fd(), PollFlags::POLLIN);
        let stop_fd = PollFd::new(auth_comm.stop_ev.as_fd(), PollFlags::POLLIN);

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
                    success = handle_auth_request(&config, auth_comm.clone(), &username);

                    // dump auth result in response pipe
                    if let Err(e) = auth_comm.response.write_bool(success) {
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
