// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2025, Nathan Gill

use anyhow::{Result, anyhow};
use atomic_enum::atomic_enum;
use pam_client::{Context, Flag, conv_mock::Conversation};
use tokio::sync::{mpsc, oneshot};
use tracing::{info, warn};
use zeroize::Zeroizing;

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

fn authenticate(username: String, password: Zeroizing<String>) -> Result<()> {
    let mut context = Context::new(
        "nlock",
        None,
        Conversation::with_credentials(username, password.as_str()),
    )?;
    context.authenticate(Flag::DISALLOW_NULL_AUTHTOK)?;
    Ok(())
}

pub async fn run_auth_loop(auth_rx: mpsc::Receiver<AuthRequest>) -> Result<()> {
    let mut rx = auth_rx;

    let username = uzers::get_current_username().ok_or(anyhow!("Current user does not exist"))?;
    let username = username.to_string_lossy().to_string();

    info!("Running authenticator for '{username}'");

    let mut success = false;

    while let Some(req) = rx.recv().await {
        match req {
            AuthRequest::Password(pwd, responder) => {
                if success {
                    continue;
                }

                match authenticate(username.clone(), pwd) {
                    Ok(()) => {
                        success = true;
                        let _ = responder.send(Ok(()));
                    }
                    Err(e) => {
                        warn!("Auth error: {e}");
                        let _ = responder.send(Err(e));
                    }
                }
            }
            AuthRequest::Exit => break,
        }
    }

    Ok(())
}
