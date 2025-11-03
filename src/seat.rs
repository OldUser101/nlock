// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2025, Nathan Gill

use wayland_client::{Dispatch, protocol::wl_pointer};

use crate::state::NLockState;

impl Dispatch<wl_pointer::WlPointer, ()> for NLockState {
    fn event(
        _: &mut Self,
        pointer: &wl_pointer::WlPointer,
        event: <wl_pointer::WlPointer as wayland_client::Proxy>::Event,
        _: &(),
        _: &wayland_client::Connection,
        _: &wayland_client::QueueHandle<Self>,
    ) {
        if let wl_pointer::Event::Enter {
            serial,
            surface: _,
            surface_x: _,
            surface_y: _,
        } = event
        {
            pointer.set_cursor(serial, None, 0, 0);
        }
    }
}
