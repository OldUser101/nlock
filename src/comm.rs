use std::{
    marker::PhantomData,
    os::fd::{AsFd, OwnedFd},
};

use anyhow::{Result, anyhow};
use nix::errno::Errno;

/// A one-way pipe based communication channel
pub struct PipeCommChannel<T> {
    tx: OwnedFd,
    rx: OwnedFd,
    _marker: PhantomData<T>,
}

impl<T> PipeCommChannel<T>
where
    T: AsBytes + FromBytes,
{
    pub fn new() -> Result<Self> {
        let p = nix::unistd::pipe()?;
        Ok(Self {
            tx: p.1,
            rx: p.0,
            _marker: PhantomData,
        })
    }

    pub fn write(&self, msg: T) -> Result<()> {
        let msg_buf = msg.as_bytes();
        let size_buf = msg_buf.len().to_ne_bytes();

        write(&self.tx, &size_buf)?;
        write(&self.tx, msg_buf)?;

        Ok(())
    }

    pub fn read(&self) -> Result<T> {
        let mut size_buf = [0u8; std::mem::size_of::<usize>()];
        read(&self.rx, &mut size_buf)?;

        let size = usize::from_ne_bytes(size_buf);
        let mut msg_buf = vec![0u8; size];
        read(&self.rx, &mut msg_buf)?;

        let msg = T::from_bytes(&msg_buf).ok_or(anyhow!("Failed to convert message"))?;

        Ok(msg)
    }

    pub fn tx(&self) -> &OwnedFd {
        &self.tx
    }

    pub fn rx(&self) -> &OwnedFd {
        &self.rx
    }
}

/// Wrapper of `nix::unistd::read` which re-tries on interrupt
fn read<Fd>(fd: Fd, buf: &mut [u8]) -> Result<usize>
where
    Fd: AsFd,
{
    loop {
        match nix::unistd::read(&fd, buf) {
            Ok(sz) => return Ok(sz),
            Err(Errno::EINTR) => continue,
            Err(e) => return Err(anyhow!("read failed: {e}")),
        }
    }
}

/// Wrapper of `nix::unistd::write` which re-tries on interrupt
fn write<Fd>(fd: Fd, buf: &[u8]) -> Result<usize>
where
    Fd: AsFd,
{
    loop {
        match nix::unistd::write(&fd, buf) {
            Ok(sz) => return Ok(sz),
            Err(Errno::EINTR) => continue,
            Err(e) => return Err(anyhow!("write failed: {e}")),
        }
    }
}

pub trait AsBytes {
    /// Convert to a bytes-like representation of the object
    fn as_bytes(&self) -> &[u8];
}

impl AsBytes for String {
    fn as_bytes(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl AsBytes for bool {
    fn as_bytes(&self) -> &[u8] {
        match self {
            false => &[0u8],
            true => &[1u8],
        }
    }
}

pub trait FromBytes {
    /// Convert from a bytes-like representation of the object
    fn from_bytes(bytes: &[u8]) -> Option<Self>
    where
        Self: Sized;
}

impl FromBytes for String {
    fn from_bytes(bytes: &[u8]) -> Option<Self>
    where
        Self: Sized,
    {
        Self::from_utf8(bytes.to_vec()).ok()
    }
}

impl FromBytes for bool {
    fn from_bytes(bytes: &[u8]) -> Option<Self>
    where
        Self: Sized,
    {
        if bytes.len() != 1 {
            return None;
        }
        match bytes[0] {
            0u8 => Some(false),
            1u8 => Some(true),
            _ => None,
        }
    }
}
