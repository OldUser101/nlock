// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026, Nathan Gill

use std::{
    os::{
        fd::{AsFd, BorrowedFd, OwnedFd},
        raw::c_void,
    },
    ptr::NonNull,
    sync::atomic::{AtomicU64, Ordering},
};

use anyhow::{Result, anyhow};
use nix::{
    fcntl::OFlag,
    sys::{
        mman::{MapFlags, ProtFlags, mmap, munmap, shm_open, shm_unlink},
        stat::Mode,
    },
    unistd::{ftruncate, getpid},
};
use tracing::{debug, warn};

static SHM_NUM: AtomicU64 = AtomicU64::new(0);

pub struct NLockShm {
    fd: OwnedFd,
    name: String,
    size: usize,
    data: Option<NonNull<c_void>>,
}

impl NLockShm {
    pub fn new(size: i64) -> Option<Self> {
        if size <= 0 {
            return None;
        }

        let name = format!(
            "/nlock-{}-{}",
            getpid(),
            SHM_NUM.fetch_add(1, Ordering::Relaxed),
        );
        debug!("Trying shm name '{}'", name);

        let fd = match shm_open(
            name.as_str(),
            OFlag::O_RDWR | OFlag::O_CREAT | OFlag::O_EXCL,
            Mode::S_IRUSR | Mode::S_IWUSR,
        ) {
            Ok(f) => f,
            Err(e) => {
                warn!("Failed to open shm '{}': {:?}", name, e);
                return None;
            }
        };

        ftruncate(&fd, size).ok()?;

        Some(Self {
            fd,
            name,
            size: size as usize,
            data: None,
        })
    }

    pub fn map(&mut self) -> Result<NonNull<c_void>> {
        if let Some(d) = self.data {
            return Err(anyhow!("shm '{}' already mapped at {:p}", self.name, d));
        }

        let data = unsafe {
            mmap(
                None,
                std::num::NonZeroUsize::new(self.size).ok_or(anyhow!("shm size was zero"))?,
                ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
                MapFlags::MAP_SHARED,
                &self.fd,
                0,
            )
        }?;

        self.data = Some(data);

        Ok(data)
    }

    pub fn data(&self) -> Option<NonNull<c_void>> {
        self.data
    }

    pub fn fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

impl Drop for NLockShm {
    fn drop(&mut self) {
        if let Some(data) = self.data
            && let Err(e) = unsafe { munmap(data, self.size) }
        {
            warn!("Failed to unmap shm '{}': {:?}", self.name, e);
        }

        if let Err(e) = shm_unlink(self.name.as_str()) {
            warn!("Failed to unlink shm '{}': {:?}", self.name, e);
        }
    }
}
