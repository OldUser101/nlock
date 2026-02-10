use anyhow::{Result, anyhow};
use nix::{
    errno::Errno,
    poll::PollTimeout,
    sys::{
        epoll::{Epoll, EpollCreateFlags, EpollEvent, EpollFlags},
        time::TimeSpec,
        timerfd::{ClockId, Expiration, TimerFd, TimerFlags, TimerSetTimeFlags},
    },
    unistd::read,
};
use std::{collections::HashMap, time::Duration};
use tracing::trace;

use super::{Id, Interest, Reactor, Token};

/// Epoll-based implementation of `Reactor`
pub struct EpollReactor {
    next_id: Id,
    interests: HashMap<Id, Interest>,
    timers: HashMap<Id, TimerFd>,
    epoll: Epoll,
}

impl EpollReactor {
    pub fn new() -> Result<Self> {
        Ok(Self {
            next_id: 0,
            interests: HashMap::new(),
            timers: HashMap::new(),
            epoll: Epoll::new(EpollCreateFlags::empty())?,
        })
    }

    fn get_next_id(&mut self) -> Id {
        let next_id = self.next_id;
        self.next_id += 1;

        if self.interests.contains_key(&next_id) {
            // this isn't safe, but we should never exceed 2^64 interests
            return self.get_next_id();
        }

        next_id
    }
}

impl Reactor for EpollReactor {
    /// Register a new Interest with the reactor, returning a unique ID
    fn register(&mut self, interest: Interest) -> Result<Id> {
        let id = self.get_next_id();

        match interest {
            Interest::Readable { ref fd, .. } => {
                let event = EpollEvent::new(EpollFlags::EPOLLIN, id);
                self.epoll.add(fd, event)?;
                self.interests.insert(id, interest);
                trace!("added interest READABLE {}", id);
            }
            Interest::Writable { ref fd, .. } => {
                let event = EpollEvent::new(EpollFlags::EPOLLOUT, id);
                self.epoll.add(fd, event)?;
                self.interests.insert(id, interest);
                trace!("added interest WRITABLE {}", id);
            }
            Interest::Timer {
                ref delay,
                ref timeout,
                ..
            } => {
                let expr = if let Some(delay) = delay {
                    Expiration::IntervalDelayed(
                        TimeSpec::from_duration(*delay),
                        TimeSpec::from_duration(*timeout),
                    )
                } else {
                    Expiration::Interval(TimeSpec::from_duration(*timeout))
                };

                let timer = TimerFd::new(ClockId::CLOCK_MONOTONIC, TimerFlags::empty())?;
                timer.set(expr, TimerSetTimeFlags::empty())?;

                let event = EpollEvent::new(EpollFlags::EPOLLIN, id);
                self.epoll.add(&timer, event)?;

                self.timers.insert(id, timer);
                self.interests.insert(id, interest);
                trace!("added interest TIMER {}", id);
            }
        }

        Ok(id)
    }

    /// Remove an event from the interest list by ID
    fn deregister(&mut self, id: Id) -> Result<()> {
        // try to remove the interest here
        let interest = self
            .interests
            .remove(&id)
            .ok_or(anyhow!("interest {} not found", id))?;

        match interest {
            Interest::Readable { fd, .. } => {
                self.epoll.delete(fd)?;
            }
            Interest::Writable { fd, .. } => {
                self.epoll.delete(fd)?;
            }
            Interest::Timer { .. } => {
                let timer = self
                    .timers
                    .remove(&id)
                    .ok_or(anyhow!("timer {} not found", id))?;
                self.epoll.delete(timer)?;
            }
        }

        trace!("remove interest {}", id);

        Ok(())
    }

    /// Wait for event to trigger
    /// Optional timeout have a duration which, in milliseconds, fits in a u16
    fn wait(&mut self, timeout: Option<Duration>) -> Result<Vec<Id>> {
        let mut events = [EpollEvent::empty(); 64];

        let timeout = if let Some(timeout) = timeout {
            PollTimeout::from(timeout.as_millis() as u16)
        } else {
            PollTimeout::NONE
        };

        let n_events = match self.epoll.wait(&mut events, timeout) {
            Ok(n) => n,
            Err(Errno::EINTR) => 0,
            Err(e) => return Err(anyhow!("epoll failed: {e}")),
        };

        let mut v_events = Vec::new();
        for ev in &events[0..n_events] {
            let id = ev.data() as Id;

            if let Some(timer) = self.timers.get(&id) {
                // read timer event count and push multiple ids
                let mut buf = [0u8; std::mem::size_of::<u64>()];
                let res = read(timer, &mut buf)?;
                if res == std::mem::size_of::<u64>() {
                    let intervals = u64::from_ne_bytes(buf);
                    for _ in 0..intervals {
                        v_events.push(id);
                    }
                }
            } else {
                v_events.push(id);
            }
        }

        Ok(v_events)
    }

    /// Get an interest token by ID
    fn event(&self, id: Id) -> Option<Token> {
        self.interests.get(&id).map(|i| i.token())
    }

    /// Get the number of registered interests
    fn count(&self) -> usize {
        self.interests.len()
    }
}
