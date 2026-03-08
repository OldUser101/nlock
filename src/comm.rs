use std::os::fd::{AsFd, OwnedFd};

use anyhow::{Result, anyhow};
use nix::errno::Errno;

/// A one-way pipe based communication channel
pub struct PipeCommChannel {
    tx: OwnedFd,
    rx: OwnedFd,
}

impl PipeCommChannel {
    pub fn new() -> Result<Self> {
        let p = nix::unistd::pipe()?;
        Ok(Self { tx: p.1, rx: p.0 })
    }

    pub fn write_str(&self, msg: String) -> Result<()> {
        let size_buf = msg.len().to_ne_bytes();
        let msg_buf = msg.as_bytes();

        write(&self.tx, &size_buf)?;
        write(&self.tx, msg_buf)?;

        Ok(())
    }

    pub fn read_str(&self) -> Result<String> {
        let mut size_buf = [0u8; std::mem::size_of::<u64>()];
        read(&self.rx, &mut size_buf)?;

        let size = usize::from_ne_bytes(size_buf);
        let mut msg_buf = vec![0u8; size];
        read(&self.rx, &mut msg_buf)?;

        let msg = String::from_utf8(msg_buf)?;

        Ok(msg)
    }

    pub fn write_bool(&self, val: bool) -> Result<()> {
        let val_buf = if val { [1u8] } else { [0u8] };
        write(&self.tx, &val_buf)?;

        Ok(())
    }

    pub fn read_bool(&self) -> Result<bool> {
        let mut val_buf = [0u8; 1];
        read(&self.rx, &mut val_buf)?;

        let val = match val_buf[0] {
            0u8 => false,
            1u8 => true,
            _ => return Err(anyhow!("Invalid value for bool, expected 1 or 0")),
        };

        Ok(val)
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
