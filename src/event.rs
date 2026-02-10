// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026, Nathan Gill

use std::{sync::atomic::Ordering, time::Duration};

use anyhow::{Result, anyhow};
use nix::unistd::read;
use tracing::warn;
use wayland_client::{EventQueue, QueueHandle, backend::ReadEventsGuard};

use crate::{
    reactor::{Id, Interest, Reactor},
    state::NLockState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u64)]
pub enum EventType {
    Wayland = 0,
    KeyboardRepeat = 1,
    StateChanged = 2,
}

impl EventType {
    fn from_u64(value: u64) -> Result<Self> {
        match value {
            0 => Ok(Self::Wayland),
            1 => Ok(Self::KeyboardRepeat),
            2 => Ok(Self::StateChanged),

            _ => Err(anyhow!("Invalid EventType value")),
        }
    }
}

impl NLockState {
    pub fn set_timer(&mut self, id: u64, timeout: Duration, delay: Option<Duration>) -> Result<()> {
        let event_id = self.reactor.register(Interest::Timer {
            delay,
            timeout,
            token: id,
        })?;

        self.timers.push((event_id, id));

        Ok(())
    }

    pub fn unset_timer(&mut self, id: u64) -> Result<()> {
        let mut i = 0;
        while i < self.timers.len() {
            if self.timers[i].1 == id {
                self.reactor.deregister(self.timers[i].0)?;
                self.timers.swap_remove(i);
            } else {
                i += 1;
            }
        }

        Ok(())
    }

    fn setup_reactor(&mut self) -> Result<()> {
        self.reactor.register(Interest::readable(
            self.state_ev.clone(),
            EventType::StateChanged as u64,
        )?)?;

        Ok(())
    }

    fn poll_events(&mut self) -> Result<Vec<Id>> {
        let events = match self.reactor.wait(None) {
            Ok(ev) => ev,
            Err(e) => return Err(anyhow!("Error while waiting: {e}")),
        };

        Ok(events)
    }

    fn process_events(
        &mut self,
        events: &Vec<Id>,
        read_guard: ReadEventsGuard,
        event_queue: &mut EventQueue<NLockState>,
    ) -> Result<()> {
        let mut wayland_sock_ready = false;
        for id in events {
            let token = if let Some(tok) = self.reactor.event(*id) {
                tok
            } else {
                warn!("Token for ID {} not found", id);
                continue;
            };

            match EventType::from_u64(token)? {
                EventType::Wayland => {
                    wayland_sock_ready = true;
                }
                EventType::KeyboardRepeat => {
                    self.handle_repeat_event();
                }
                EventType::StateChanged => {
                    // Read whatever is stored in there, we don't care what
                    let mut buf = [0u8; std::mem::size_of::<u64>()];
                    let _ = read(self.state_ev.clone(), &mut buf)?;
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
                    self.password.len(),
                    self.background_image.as_ref(),
                    shm,
                    qh,
                );
            }

            self.state_changed.store(false, Ordering::Relaxed);
        }
    }

    pub fn event_loop_cycle(&mut self, event_queue: &mut EventQueue<NLockState>) -> Result<()> {
        if self.reactor.count() == 0 {
            self.setup_reactor()?;
        }

        event_queue.flush()?;
        event_queue.dispatch_pending(self)?;

        let read_guard = event_queue
            .prepare_read()
            .ok_or(anyhow!("Failed to obtain Wayland event read guard"))?;
        let wayland_sock_fd = read_guard.connection_fd();

        // Register the Wayland file descriptor with the reactor
        let wayland_id = self.reactor.register(Interest::readable(
            wayland_sock_fd,
            EventType::Wayland as u64,
        )?)?;

        let events = self.poll_events()?;
        self.process_events(&events, read_guard, event_queue)?;
        self.re_render(&event_queue.handle());

        self.reactor.deregister(wayland_id)?;

        Ok(())
    }
}
