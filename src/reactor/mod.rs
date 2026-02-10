use anyhow::Result;
use std::{
    os::fd::{AsFd, OwnedFd},
    time::Duration,
};

#[cfg(target_os = "linux")]
pub mod epoll;

pub type Id = u64;
pub type Token = u64;

fn dup_fd(fd: impl AsFd) -> Result<OwnedFd> {
    Ok(fd.as_fd().try_clone_to_owned()?)
}

#[derive(Debug)]
pub enum Interest {
    Readable {
        fd: OwnedFd,
        token: Token,
    },
    Writable {
        fd: OwnedFd,
        token: Token,
    },
    Timer {
        delay: Option<Duration>,
        timeout: Duration,
        token: Token,
    },
}

impl Interest {
    pub fn readable(fd: impl AsFd, token: Token) -> Result<Self> {
        Ok(Self::Readable {
            fd: dup_fd(fd)?,
            token,
        })
    }

    pub fn writable(fd: impl AsFd, token: Token) -> Result<Self> {
        Ok(Self::Writable {
            fd: dup_fd(fd)?,
            token,
        })
    }

    pub fn fd(&self) -> Option<&OwnedFd> {
        match self {
            Interest::Readable { fd, .. } => Some(fd),
            Interest::Writable { fd, .. } => Some(fd),
            _ => None,
        }
    }

    pub fn token(&self) -> Token {
        match self {
            Interest::Readable { token, .. } => *token,
            Interest::Writable { token, .. } => *token,
            Interest::Timer { token, .. } => *token,
        }
    }
}

pub trait Reactor {
    fn register(&mut self, interest: Interest) -> Result<Id>;
    fn deregister(&mut self, id: Id) -> Result<()>;

    fn wait(&mut self, timeout: Option<Duration>) -> Result<Vec<Id>>;

    fn event(&self, id: Id) -> Option<Token>;
    fn count(&self) -> usize;
}
