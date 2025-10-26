// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2025, Nathan Gill

use wayland_client::{
    QueueHandle, WEnum,
    protocol::{wl_compositor::WlCompositor, wl_output, wl_surface},
};

use crate::state::NLockState;

pub struct NLockSurface {
    pub dirty: bool,
    pub created: bool,
    pub output_name: Option<String>,
    pub output_scale: i32,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub subpixel: Option<WEnum<wl_output::Subpixel>>,
    pub surface: Option<wl_surface::WlSurface>,
}

impl NLockSurface {
    pub fn new() -> Self {
        Self {
            dirty: false,
            created: false,
            output_name: None,
            output_scale: 1,
            width: None,
            height: None,
            subpixel: None,
            surface: None,
        }
    }

    pub fn create_surface(&mut self, compositor: &WlCompositor, qh: &QueueHandle<NLockState>) {
        if !self.created {
            let surface = compositor.create_surface(qh, ());
            self.surface = Some(surface);
            self.created = true;
        }
    }
}

impl Default for NLockSurface {
    fn default() -> Self {
        Self::new()
    }
}
