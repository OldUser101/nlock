// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026, Nathan Gill

use std::{
    os::fd::{AsRawFd, BorrowedFd},
    sync::atomic::Ordering,
};

use anyhow::{Result, anyhow};
use nix::errno::Errno;
use tracing::warn;
use wayland_client::{EventQueue, QueueHandle, backend::ReadEventsGuard};

use crate::{
    auth::AuthState,
    event_loop::{Event, EventSource, EventTag},
    state::NLockState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum EventType {
    Wayland = 0,
    KeyboardRepeat = 1,
    AuthStateChanged = 2,
}

impl EventType {
    fn from_usize(value: usize) -> Result<Self> {
        match value {
            0 => Ok(Self::Wayland),
            1 => Ok(Self::KeyboardRepeat),
            2 => Ok(Self::AuthStateChanged),

            _ => Err(anyhow!("Invalid EventType value")),
        }
    }
}

impl From<EventType> for EventTag {
    fn from(value: EventType) -> Self {
        value as usize
    }
}

impl NLockState {
    fn poll_events(&mut self, wayland_sock_fd: BorrowedFd<'_>) -> Result<Vec<Event>> {
        self.event_loop.add(
            EventSource::Fd(wayland_sock_fd.as_raw_fd()),
            EventType::Wayland.into(),
        )?;

        let events = match self.event_loop.poll() {
            Ok(evs) => evs,
            Err(Errno::EINTR) => Vec::new(),
            Err(e) => return Err(anyhow!("Error during poll: {e}")),
        };

        self.event_loop.remove(EventType::Wayland.into());

        Ok(events)
    }

    fn process_events(
        &mut self,
        events: Vec<Event>,
        read_guard: ReadEventsGuard,
        event_queue: &mut EventQueue<NLockState>,
    ) -> Result<()> {
        let mut wayland_sock_ready = false;
        for event in events {
            match event {
                Event::Readable { fd: _, tag } => match EventType::from_usize(tag)? {
                    EventType::Wayland => {
                        wayland_sock_ready = true;
                    }
                    EventType::AuthStateChanged => match self.auth_comm.response.read() {
                        Ok(AuthState::Idle) => {
                            // request was ignored by backend, do nothing
                        }
                        Ok(AuthState::Success) => {
                            // auth was successful, set flags for exit
                            self.auth_state.store(AuthState::Success, Ordering::Relaxed);
                            self.running.store(false, Ordering::Relaxed);
                            self.state_changed.store(true, Ordering::Relaxed);
                        }
                        Ok(AuthState::Fail) => {
                            // auth failed, set fail state
                            self.auth_state.store(AuthState::Fail, Ordering::Relaxed);
                            self.state_changed.store(true, Ordering::Relaxed);
                        }
                        Err(e) => {
                            warn!("Failed to receive auth response: {e}");
                        }
                    },
                    _ => {}
                },
                Event::Timeout { tag } => {
                    if EventType::from_usize(tag)? == EventType::KeyboardRepeat {
                        self.handle_repeat_event();
                    }
                }
            }
        }

        if wayland_sock_ready {
            read_guard.read()?;
            event_queue.dispatch_pending(self)?;
        } else {
            std::mem::drop(read_guard);
        }

        Ok(())
    }

    fn re_render(&mut self, qh: &QueueHandle<NLockState>) {
        // Re-render only if state was updated
        if self.state_changed.load(Ordering::Relaxed)
            && let Some(shm) = &self.shm
        {
            let auth_state = self.auth_state.clone().load(Ordering::Relaxed);

            for i in 0..self.surfaces.len() {
                self.surfaces[i].render(
                    &self.config,
                    auth_state,
                    self.password.chars().count(),
                    self.background_image.as_ref(),
                    shm,
                    qh,
                );
            }

            self.state_changed.store(false, Ordering::Relaxed);
        }
    }

    pub fn event_loop_cycle(&mut self, event_queue: &mut EventQueue<NLockState>) -> Result<()> {
        event_queue.flush()?;
        event_queue.dispatch_pending(self)?;

        let read_guard = event_queue
            .prepare_read()
            .ok_or(anyhow!("Failed to obtain Wayland event read guard"))?;
        let wayland_sock_fd = read_guard.connection_fd();

        let events = self.poll_events(wayland_sock_fd)?;
        self.process_events(events, read_guard, event_queue)?;
        self.re_render(&event_queue.handle());

        Ok(())
    }
}
