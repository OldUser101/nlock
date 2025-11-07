// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2025, Nathan Gill

use anyhow::{Result, anyhow};
use nix::{
    errno::Errno,
    poll::PollTimeout,
    sys::{
        epoll::{Epoll, EpollCreateFlags, EpollEvent, EpollFlags},
        timerfd::{ClockId, Expiration, TimerFd, TimerFlags, TimerSetTimeFlags},
    },
    unistd::read,
};
use std::os::fd::BorrowedFd;
use wayland_client::EventQueue;

use crate::state::NLockState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u64)]
pub enum EventType {
    Wayland = 0,
    KeyboardRepeat = 1,
}

impl EventType {
    fn from_u64(value: u64) -> Result<Self> {
        match value {
            0 => Ok(Self::Wayland),
            1 => Ok(Self::KeyboardRepeat),

            _ => Err(anyhow!("Invalid EventType value")),
        }
    }
}

/// This guard structure is used to ensure the Wayland file descriptor (given by a `ReadEventsGuard` object).
///
/// This structure contains a reference to an `Epoll` object (probably from an `EventLoop`).
/// The Wayland file descriptor is automatically removed from `Epoll` when dropped.
struct WaylandFdCleanup<'a> {
    epoll: &'a Epoll,
    fd: BorrowedFd<'a>,
}

impl Drop for WaylandFdCleanup<'_> {
    fn drop(&mut self) {
        let _ = self.epoll.delete(self.fd);
    }
}

impl NLockState {
    pub fn set_timer(&mut self, id: u64, expiration: Expiration) -> Result<()> {
        let repeat_timer = TimerFd::new(ClockId::CLOCK_MONOTONIC, TimerFlags::empty())?;
        repeat_timer.set(expiration, TimerSetTimeFlags::empty())?;

        let repeat_timer_ev = EpollEvent::new(EpollFlags::EPOLLIN, id);
        let epoll = self
            .epoll
            .as_ref()
            .ok_or(anyhow!("Epoll has not been created yet"))?;
        epoll.add(&repeat_timer, repeat_timer_ev)?;

        self.timers.push((repeat_timer, id));

        Ok(())
    }

    pub fn unset_timer(&mut self, id: u64) -> Result<()> {
        let epoll = self
            .epoll
            .as_ref()
            .ok_or(anyhow!("Epoll has not been created yet"))?;

        let mut i = 0;
        while i < self.timers.len() {
            if self.timers[i].1 == id {
                epoll.delete(&self.timers[i].0)?;
                self.timers.swap_remove(i);
            } else {
                i += 1;
            }
        }

        Ok(())
    }

    pub fn event_loop_cycle(&mut self, event_queue: &mut EventQueue<NLockState>) -> Result<()> {
        if self.epoll.is_none() {
            self.epoll = Some(Epoll::new(EpollCreateFlags::empty())?);
        }

        let mut events = [EpollEvent::empty(); 64];

        event_queue.flush()?;
        event_queue.dispatch_pending(self)?;

        let read_guard = event_queue
            .prepare_read()
            .ok_or(anyhow!("Failed to obtain Wayland event read guard"))?;
        let wayland_sock_fd = read_guard.connection_fd();
        let wayland_sock_ev = EpollEvent::new(EpollFlags::EPOLLIN, EventType::Wayland as u64);

        let epoll = self
            .epoll
            .as_ref()
            .ok_or(anyhow!("Epoll has not been created yet"))?;
        epoll.add(wayland_sock_fd, wayland_sock_ev)?;

        let n_events = {
            let _cleanup_guard = WaylandFdCleanup {
                fd: wayland_sock_fd,
                epoll,
            };

            match epoll.wait(&mut events, PollTimeout::NONE) {
                Ok(n) => n,
                Err(Errno::EINTR) => 0,
                Err(e) => return Err(anyhow!("Error during epoll: {e}")),
            }
        };

        let qh = event_queue.handle();

        let mut wayland_sock_ready = false;
        for event in &events[..n_events] {
            match EventType::from_u64(event.data())? {
                EventType::Wayland => {
                    wayland_sock_ready = true;
                }
                EventType::KeyboardRepeat => {
                    if let Some(idx) = self
                        .timers
                        .iter()
                        .position(|timer| timer.1 == EventType::KeyboardRepeat as u64)
                    {
                        let timer = &self.timers[idx];
                        let mut buf = [0u8; std::mem::size_of::<u64>()];
                        let res = read(&timer.0, &mut buf)?;
                        if res == std::mem::size_of::<u64>() {
                            let intervals = u64::from_ne_bytes(buf);
                            for _ in 0..intervals {
                                self.handle_repeat_event(&qh);
                            }
                        }
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
}
