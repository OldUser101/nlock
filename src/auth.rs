// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2025, Nathan Gill

use anyhow::{Result, anyhow};
use atomic_enum::atomic_enum;
use pam_rs::{Client, PamFlag};
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, warn};
use zeroize::Zeroizing;

use crate::config::NLockConfig;

#[derive(Debug)]
pub enum AuthRequest {
    Password(Zeroizing<String>, oneshot::Sender<Result<()>>),
    Exit,
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

fn authenticate(config: &AuthConfig, username: String, password: Zeroizing<String>) -> Result<()> {
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

pub async fn run_auth_loop(config: AuthConfig, auth_rx: mpsc::Receiver<AuthRequest>) -> Result<()> {
    let mut rx = auth_rx;

    let username = uzers::get_current_username().ok_or(anyhow!("Current user does not exist"))?;
    let username = username.to_string_lossy().to_string();

    debug!("Running authenticator for '{username}'");

    let mut success = false;

    while let Some(req) = rx.recv().await {
        match req {
            AuthRequest::Password(pwd, responder) => {
                if success {
                    continue;
                }

                match authenticate(&config, username.clone(), pwd) {
                    Ok(()) => {
                        success = true;
                        let _ = responder.send(Ok(()));
                    }
                    Err(e) => {
                        debug!("Auth error: {e}");
                        let _ = responder.send(Err(e));
                    }
                }
            }
            AuthRequest::Exit => break,
        }
    }

    Ok(())
}
