use std::{
    os::fd::RawFd,
    time::{Duration, Instant},
};

use anyhow::{Result, bail};
use nix::{errno::Errno, libc, poll::PollTimeout};
use tracing::trace;

pub type EventTag = usize;

pub type PollResult<T> = std::result::Result<T, Errno>;

#[derive(Debug)]
struct TaggedFd {
    fd: RawFd,
    tag: EventTag,
}

#[derive(Debug)]
struct Timer {
    interval: Duration,
    last: Instant,
    tag: EventTag,
}

pub enum Event {
    Readable { fd: RawFd, tag: EventTag },
    Timeout { tag: EventTag },
}

pub enum Expiration {
    Interval(Duration),
    IntervalDelayed(Duration, Duration),
}

pub enum EventSource {
    Fd(RawFd),
    Timer(Expiration),
}

pub struct NLockEventLoop {
    source_fds: Vec<TaggedFd>,
    timers: Vec<Timer>,
}

impl NLockEventLoop {
    pub fn new() -> Self {
        Self {
            source_fds: Vec::new(),
            timers: Vec::new(),
        }
    }

    fn next_expiry(&self) -> Option<Duration> {
        if self.timers.is_empty() {
            return None;
        }

        let mut dur = Duration::MAX;
        for t in &self.timers {
            let now = Instant::now();
            let elapsed = now.saturating_duration_since(t.last);

            if elapsed >= t.interval {
                // already expired
                return Some(Duration::ZERO);
            }

            // this is weird but required to handle delayed timers
            let left = if t.last > now {
                t.last.duration_since(now)
            } else {
                t.interval - elapsed
            };

            if left < dur {
                dur = left;
            }
        }

        Some(dur)
    }

    pub fn poll(&mut self) -> PollResult<Vec<Event>> {
        let mut events = Vec::new();

        let mut poll_fds: Vec<libc::pollfd> = self
            .source_fds
            .iter()
            .map(|t| libc::pollfd {
                fd: t.fd,
                events: libc::POLLIN,
                revents: 0,
            })
            .collect();

        let timeout = self
            .next_expiry()
            .map(|d| libc::c_int::try_from(d.as_millis()).unwrap_or(libc::c_int::MAX))
            .unwrap_or(PollTimeout::NONE.into());

        let res = unsafe {
            libc::poll(
                poll_fds.as_mut_ptr(),
                poll_fds.len() as libc::nfds_t,
                timeout,
            )
        };
        if res == -1 {
            return Err(Errno::last());
        }

        for pf in &poll_fds {
            if pf.revents & libc::POLLIN != 0
                && let Some(tfd) = self.source_fds.iter().find(|t| t.fd == pf.fd)
            {
                let ev = Event::Readable {
                    fd: tfd.fd,
                    tag: tfd.tag,
                };
                events.push(ev);
            }
        }

        for t in &mut self.timers {
            let now = Instant::now();
            let elapsed = now - t.last;
            if elapsed >= t.interval {
                let ev = Event::Timeout { tag: t.tag };
                t.last = now;
                events.push(ev);
            }
        }

        Ok(events)
    }

    pub fn add(&mut self, source: EventSource, tag: EventTag) -> Result<()> {
        match source {
            EventSource::Fd(fd) => {
                let tfd = TaggedFd { fd, tag };
                trace!("added event source: {:?}", tfd);
                self.source_fds.push(tfd);
            }
            EventSource::Timer(expiration) => {
                let (delay, interval) = match expiration {
                    Expiration::Interval(interval) => (None, interval),
                    Expiration::IntervalDelayed(delay, interval) => (Some(delay), interval),
                };

                if interval.is_zero() {
                    bail!("timer interval cannot be set to zero!");
                }

                let timer = Timer {
                    interval,
                    last: if let Some(delay) = delay {
                        // offset next interval by delay
                        Instant::now() + delay - interval
                    } else {
                        Instant::now()
                    },
                    tag,
                };
                trace!("added timer: {:?}", timer);
                self.timers.push(timer);
            }
        }

        Ok(())
    }

    pub fn remove(&mut self, tag: EventTag) {
        self.source_fds.retain(|t| t.tag != tag);
        self.timers.retain(|t| t.tag != tag);
        trace!("removed tag: {:?}", tag);
    }
}

impl Default for NLockEventLoop {
    fn default() -> Self {
        Self::new()
    }
}
