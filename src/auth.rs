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

use crate::{auth_sys::AuthClient, comm::PipeCommChannel, config::NLockConfig};

pub struct AuthChannel {
    pub request: PipeCommChannel<String>,
    pub response: PipeCommChannel<AuthState>,
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

#[derive(Debug)]
pub struct AuthConfig {
    pwd_allow_empty: bool,
}

impl From<&NLockConfig> for AuthConfig {
    fn from(value: &NLockConfig) -> Self {
        Self {
            pwd_allow_empty: value.general.pwd_allow_empty,
        }
    }
}

#[atomic_enum]
#[derive(PartialEq)]
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
fn handle_auth_request(
    config: &AuthConfig,
    client: &mut AuthClient,
    auth_comm: Arc<AuthChannel>,
) -> AuthState {
    let pwd = match auth_comm.request.read().map(Zeroizing::new) {
        Ok(p) => p,
        Err(e) => {
            warn!("Auth comm error: {e}");
            return AuthState::Fail;
        }
    };

    if !config.pwd_allow_empty && pwd.is_empty() {
        debug!("Auth request ignored, password is empty");
        return AuthState::Idle;
    }

    match authenticate(client, pwd) {
        Ok(()) => AuthState::Success,
        Err(e) => {
            warn!("Auth failed: {e}");
            AuthState::Fail
        }
    }
}

fn auth_loop(
    config: AuthConfig,
    mut client: AuthClient,
    auth_comm: Arc<AuthChannel>,
) -> Result<()> {
    let mut state = AuthState::Idle;

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
                if events[0].any().unwrap_or_default() && state != AuthState::Success {
                    state = handle_auth_request(&config, &mut client, auth_comm.clone());

                    // dump auth result in response pipe
                    if let Err(e) = auth_comm.response.write(state) {
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

pub fn setup_auth(config: AuthConfig, auth_comm: Arc<AuthChannel>) -> Result<JoinHandle<()>> {
    let client = AuthClient::new("nlock")?;

    let handle = std::thread::spawn({
        move || {
            if let Err(e) = auth_loop(config, client, auth_comm) {
                warn!("Error in auth thread: {e}");
            }
            debug!("Auth thread exited");
        }
    });

    Ok(handle)
}
