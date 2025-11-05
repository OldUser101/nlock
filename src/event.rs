use std::os::fd::BorrowedFd;
use nix::{errno::Errno, poll::PollTimeout, sys::epoll::{Epoll, EpollCreateFlags, EpollEvent, EpollFlags}};
use wayland_client::EventQueue;
use anyhow::{anyhow, Result};

use crate::state::NLockState;

const WAYLAND_SOCK_EV_DATA: u64 = 0;

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
    pub fn event_loop_cycle(&mut self, event_queue: &mut EventQueue<NLockState>) -> Result<()> {
        if self.epoll.is_none() {
            self.epoll = Some(Epoll::new(EpollCreateFlags::empty())?);
        }

        let mut events = [EpollEvent::empty(); 64];
        
        event_queue.flush()?;
        event_queue.dispatch_pending(self)?;

        let read_guard = event_queue.prepare_read().ok_or(anyhow!("Failed to obtain Wayland event read guard"))?;
        let wayland_sock_fd = read_guard.connection_fd();
        let wayland_sock_ev = EpollEvent::new(EpollFlags::EPOLLIN, WAYLAND_SOCK_EV_DATA);
        
        let epoll = self.epoll.as_ref().ok_or(anyhow!("Epoll has not been created yet"))?;
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

        let mut wayland_sock_ready = false;
        for event in &events[..n_events] {
            match event.data() {
                WAYLAND_SOCK_EV_DATA => {
                    wayland_sock_ready = true;
                }
                _ => {}
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
