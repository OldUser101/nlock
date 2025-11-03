// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2025, Nathan Gill

use anyhow::{Result, anyhow};
use std::os::fd::OwnedFd;
use tracing::{debug, warn};
use wayland_client::{
    Connection, Dispatch, QueueHandle, WEnum,
    protocol::{wl_keyboard, wl_pointer, wl_seat},
};
use xkbcommon::xkb;

use crate::state::NLockState;

pub struct NLockXkb {
    pub context: xkb::Context,
    pub keymap: Option<xkb::Keymap>,
    pub state: Option<xkb::State>,
}

impl NLockXkb {
    pub fn new() -> Self {
        Self {
            context: xkb::Context::new(0),
            keymap: None,
            state: None,
        }
    }
}

impl Default for NLockXkb {
    fn default() -> Self {
        Self::new()
    }
}

pub struct NLockSeat {
    pub pointer: Option<wl_pointer::WlPointer>,
    pub keyboard: Option<wl_keyboard::WlKeyboard>,
    pub repeat_rate: Option<i32>,
    pub repeat_delay: Option<i32>,
}

impl NLockSeat {
    pub fn new() -> Self {
        Self {
            pointer: None,
            keyboard: None,
            repeat_rate: None,
            repeat_delay: None,
        }
    }
}

impl Default for NLockSeat {
    fn default() -> Self {
        Self::new()
    }
}

impl NLockState {
    pub fn handle_keymap(&mut self, fd: OwnedFd, size: u32) -> Result<()> {
        let keymap =
            unsafe { xkb::Keymap::new_from_fd(&self.xkb.context, fd, size as usize, 1, 0) }?
                .ok_or(anyhow!("Failed to get keymap"))?;
        let state = xkb::State::new(&keymap);

        self.xkb.state = Some(state);
        self.xkb.keymap = Some(keymap);

        debug!("Created keymap and state");

        Ok(())
    }

    pub fn handle_key(
        &mut self,
        key: u32,
        _key_state: WEnum<wl_keyboard::KeyState>,
    ) -> Result<()> {
        if key == 1 {
            self.running = false;
        }

        Ok(())
    }

    pub fn handle_modifiers(
        &mut self,
        _depressed: u32,
        _latched: u32,
        _locked: u32,
        _group: u32,
    ) -> Result<()> {
        Ok(())
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for NLockState {
    fn event(
        state: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: <wl_keyboard::WlKeyboard as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Keymap { format, fd, size } => {
                if let WEnum::Value(wl_keyboard::KeymapFormat::XkbV1) = format
                    && let Err(e) = state.handle_keymap(fd, size)
                {
                    warn!("Error while handling keymap event: {e}");
                }
            }
            wl_keyboard::Event::Key {
                serial: _,
                time: _,
                key,
                state: key_state,
            } => {
                if let Err(e) = state.handle_key(key, key_state) {
                    warn!("Error while handling key event: {e}");
                }
            }
            wl_keyboard::Event::RepeatInfo { rate, delay } => {
                state.seat.repeat_rate = Some(rate);
                state.seat.repeat_delay = Some(delay);
            }
            wl_keyboard::Event::Modifiers {
                serial: _,
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
            } => {
                if let Err(e) =
                    state.handle_modifiers(mods_depressed, mods_latched, mods_locked, group)
                {
                    warn!("Error while handling modifiers event: {e}");
                }
            }
            _ => {}
        }
    }
}

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

impl Dispatch<wl_seat::WlSeat, ()> for NLockState {
    fn event(
        state: &mut Self,
        seat: &wl_seat::WlSeat,
        event: <wl_seat::WlSeat as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(capabilities),
        } = event
        {
            if capabilities.contains(wl_seat::Capability::Keyboard) {
                let keyboard = seat.get_keyboard(qh, ());
                state.seat.keyboard = Some(keyboard);

                debug!("Found keyboard");
            }
            if capabilities.contains(wl_seat::Capability::Pointer) {
                let pointer = seat.get_pointer(qh, ());
                state.seat.pointer = Some(pointer);

                debug!("Found pointer");
            }
        }
    }
}
