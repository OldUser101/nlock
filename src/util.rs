// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2025, Nathan Gill

use nix::{
    fcntl::OFlag,
    sys::{
        mman::{shm_open, shm_unlink},
        stat::Mode,
    },
    unistd::getpid,
};
use std::os::fd::OwnedFd;
use tracing::debug;

pub fn open_shm() -> Option<OwnedFd> {
    let mut retries = 100;

    loop {
        let time = chrono::Local::now();
        let name = format!(
            "/nlock-{}-{}-{}",
            getpid(),
            time.timestamp_micros(),
            time.timestamp_subsec_nanos()
        );
        debug!("Trying temp file name '{}'", name);

        if let Ok(fd) = shm_open(
            name.as_str(),
            OFlag::O_RDWR | OFlag::O_CREAT | OFlag::O_EXCL,
            Mode::S_IRUSR | Mode::S_IWUSR,
        ) {
            let _ = shm_unlink(name.as_str());
            return Some(fd);
        }

        retries -= 1;
        if retries <= 0 {
            break;
        }
    }

    None
}
