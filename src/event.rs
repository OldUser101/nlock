// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026, Nathan Gill

use std::{
    os::fd::{AsFd, AsRawFd, BorrowedFd},
    sync::atomic::Ordering,
};

use anyhow::{Result, anyhow};
use mio::{Events, Interest, Poll, Token, unix::SourceFd};
use nix::{
    sys::timerfd::{ClockId, Expiration, TimerFd, TimerFlags, TimerSetTimeFlags},
    unistd::read,
};
use wayland_client::{EventQueue, QueueHandle, backend::ReadEventsGuard};

use crate::{state::NLockState, util::is_eintr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum EventType {
    Wayland = 0,
    KeyboardRepeat = 1,
    StateChanged = 2,
}

impl EventType {
    fn from_usize(value: usize) -> Result<Self> {
        match value {
            0 => Ok(Self::Wayland),
            1 => Ok(Self::KeyboardRepeat),
            2 => Ok(Self::StateChanged),

            _ => Err(anyhow!("Invalid EventType value")),
        }
    }
}

impl NLockState {
    pub fn set_timer(&mut self, id: usize, expiration: Expiration) -> Result<()> {
        let repeat_timer = TimerFd::new(ClockId::CLOCK_MONOTONIC, TimerFlags::empty())?;
        repeat_timer.set(expiration, TimerSetTimeFlags::empty())?;

        let mut repeat_timer_src = SourceFd(&repeat_timer.as_fd().as_raw_fd());

        let poll = self
            .poll
            .as_mut()
            .ok_or(anyhow!("Poll has not been created yet"))?;

        poll.registry()
            .register(&mut repeat_timer_src, Token(id), Interest::READABLE)?;

        self.timers.push((repeat_timer, id));

        Ok(())
    }

    pub fn unset_timer(&mut self, id: usize) -> Result<()> {
        let poll = self
            .poll
            .as_mut()
            .ok_or(anyhow!("Poll has not been created yet"))?;

        let mut i = 0;
        while i < self.timers.len() {
            if self.timers[i].1 == id {
                poll.registry()
                    .deregister(&mut SourceFd(&self.timers[i].0.as_fd().as_raw_fd()))?;
                self.timers.swap_remove(i);
            } else {
                i += 1;
            }
        }

        Ok(())
    }

    fn setup_poll(&mut self) -> Result<()> {
        let poll = Poll::new()?;

        // Register the state event file descriptor
        poll.registry().register(
            &mut SourceFd(&self.state_ev.as_raw_fd()),
            Token(EventType::StateChanged as usize),
            Interest::READABLE,
        )?;

        self.poll = Some(poll);
        Ok(())
    }

    fn poll_events(&mut self, events: &mut Events, wayland_sock_fd: BorrowedFd<'_>) -> Result<()> {
        let mut wayland_sock_src = SourceFd(&wayland_sock_fd.as_raw_fd());

        let poll = self
            .poll
            .as_mut()
            .ok_or(anyhow!("Poll has not been created yet"))?;

        {
            // Register the Wayland file descriptor with the poll
            poll.registry().register(
                &mut wayland_sock_src,
                Token(EventType::Wayland as usize),
                Interest::READABLE,
            )?;

            match poll.poll(events, None) {
                Ok(_) => {}
                Err(e) if is_eintr(&e) => {}
                Err(e) => return Err(anyhow!("Error during epoll: {e}")),
            }

            poll.registry().deregister(&mut wayland_sock_src)?;
        }

        Ok(())
    }

    fn process_events(
        &mut self,
        events: &Events,
        read_guard: ReadEventsGuard,
        event_queue: &mut EventQueue<NLockState>,
    ) -> Result<()> {
        let mut wayland_sock_ready = false;
        for event in events {
            match EventType::from_usize(event.token().0)? {
                EventType::Wayland => {
                    wayland_sock_ready = true;
                }
                EventType::KeyboardRepeat => {
                    if let Some(idx) = self
                        .timers
                        .iter()
                        .position(|timer| timer.1 == EventType::KeyboardRepeat as usize)
                    {
                        let timer = &self.timers[idx];
                        let mut buf = [0u8; std::mem::size_of::<u64>()];
                        let res = read(&timer.0, &mut buf)?;
                        if res == std::mem::size_of::<u64>() {
                            let intervals = u64::from_ne_bytes(buf);
                            for _ in 0..intervals {
                                self.handle_repeat_event();
                            }
                        }
                    }
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
        if self.poll.is_none() {
            self.setup_poll()?;
        }

        let mut events = Events::with_capacity(64);

        event_queue.flush()?;
        event_queue.dispatch_pending(self)?;

        let read_guard = event_queue
            .prepare_read()
            .ok_or(anyhow!("Failed to obtain Wayland event read guard"))?;
        let wayland_sock_fd = read_guard.connection_fd();

        self.poll_events(&mut events, wayland_sock_fd)?;
        self.process_events(&events, read_guard, event_queue)?;
        self.re_render(&event_queue.handle());

        Ok(())
    }
}
