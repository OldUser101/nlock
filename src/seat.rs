// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2025, Nathan Gill

use anyhow::{Result, anyhow};
use nix::sys::{time::TimeSpec, timerfd::Expiration};
use std::{os::fd::OwnedFd, sync::atomic::Ordering, time::Duration};
use tracing::{debug, warn};
use wayland_client::{
    Connection, Dispatch, QueueHandle, WEnum,
    protocol::{wl_keyboard, wl_pointer, wl_seat},
};
use xkbcommon::xkb;

use crate::{event::EventType, state::NLockState};

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
    pub repeat_rate: i32,
    pub repeat_delay: i32,
    pub repeat_keysym: Option<xkb::Keysym>,
    pub repeat_codepoint: Option<u32>,
    pub repeat_timer_set: bool,
}

impl NLockSeat {
    pub fn new() -> Self {
        Self {
            pointer: None,
            keyboard: None,
            repeat_rate: 0,
            repeat_delay: 0,
            repeat_keysym: None,
            repeat_codepoint: None,
            repeat_timer_set: false,
        }
    }
}

impl Default for NLockSeat {
    fn default() -> Self {
        Self::new()
    }
}

impl NLockState {
    pub fn handle_keymap_event(&mut self, fd: OwnedFd, size: u32) -> Result<()> {
        let keymap =
            unsafe { xkb::Keymap::new_from_fd(&self.xkb.context, fd, size as usize, 1, 0) }?
                .ok_or(anyhow!("Failed to get keymap"))?;
        let state = xkb::State::new(&keymap);

        self.xkb.state = Some(state);
        self.xkb.keymap = Some(keymap);

        debug!("Created keymap and state");

        Ok(())
    }

    pub fn process_key(&mut self, keysym: xkb::Keysym, codepoint: u32) {
        match keysym {
            xkb::Keysym::KP_Enter | xkb::Keysym::Return => {
                self.submit_password();
            }
            xkb::Keysym::BackSpace | xkb::Keysym::Delete => {
                if !self.password.is_empty() {
                    self.password.pop();
                }
            }
            _ => match char::from_u32(codepoint) {
                Some(ch) if !ch.is_control() => {
                    self.password.push(ch);
                }
                _ => {}
            },
        }

        self.state_changed.store(true, Ordering::Relaxed);
    }

    pub fn handle_key_event(
        &mut self,
        key: u32,
        key_state: WEnum<wl_keyboard::KeyState>,
    ) -> Result<()> {
        if self.xkb.state.is_none() {
            return Err(anyhow!("Xkb state not set"));
        }

        let keycode = xkb::Keycode::new(key + 8);
        let keysym = self.xkb.state.as_ref().unwrap().key_get_one_sym(keycode);
        let codepoint = self.xkb.state.as_ref().unwrap().key_get_utf32(keycode);

        if let WEnum::Value(wl_keyboard::KeyState::Pressed) = key_state {
            self.process_key(keysym, codepoint);
        }

        if self.seat.repeat_timer_set
            && let Err(e) = self.unset_timer(EventType::KeyboardRepeat as u64)
        {
            return Err(e);
        } else {
            self.seat.repeat_timer_set = false;
        }

        if let WEnum::Value(wl_keyboard::KeyState::Pressed) = key_state
            && self.seat.repeat_rate > 0
        {
            self.seat.repeat_keysym = Some(keysym);
            self.seat.repeat_codepoint = Some(codepoint);

            let repeat_delay_duration = Duration::from_millis(self.seat.repeat_delay as u64);
            let repeat_rate_duration = Duration::from_millis(self.seat.repeat_rate as u64);

            self.set_timer(
                EventType::KeyboardRepeat as u64,
                Expiration::IntervalDelayed(
                    TimeSpec::from_duration(repeat_delay_duration),
                    TimeSpec::from_duration(repeat_rate_duration),
                ),
            )?;

            self.seat.repeat_timer_set = true;
        }

        Ok(())
    }

    pub fn handle_repeat_event(&mut self) {
        if let (Some(keysym), Some(codepoint)) =
            (self.seat.repeat_keysym, self.seat.repeat_codepoint)
        {
            self.process_key(keysym, codepoint);
        }
    }

    pub fn handle_modifiers_event(
        &mut self,
        depressed: u32,
        latched: u32,
        locked: u32,
        group: u32,
    ) -> Result<()> {
        if self.xkb.state.is_none() {
            return Err(anyhow!("Xkb state not set"));
        }

        self.xkb
            .state
            .as_mut()
            .unwrap()
            .update_mask(depressed, latched, locked, 0, 0, group);
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
                    && let Err(e) = state.handle_keymap_event(fd, size)
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
                if let Err(e) = state.handle_key_event(key, key_state) {
                    warn!("Error while handling key event: {e}");
                }
            }
            wl_keyboard::Event::RepeatInfo { rate, delay } => {
                state.seat.repeat_rate = rate;
                state.seat.repeat_delay = delay;
            }
            wl_keyboard::Event::Modifiers {
                serial: _,
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
            } => {
                if let Err(e) =
                    state.handle_modifiers_event(mods_depressed, mods_latched, mods_locked, group)
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
        state: &mut Self,
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
            && state.config.general.hide_cursor
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
                if let Some(keyboard) = &state.seat.keyboard {
                    keyboard.release();
                }

                let keyboard = seat.get_keyboard(qh, ());
                state.seat.keyboard = Some(keyboard);

                debug!("Found keyboard");
            }
            if capabilities.contains(wl_seat::Capability::Pointer) {
                if let Some(pointer) = &state.seat.pointer {
                    pointer.release();
                }

                let pointer = seat.get_pointer(qh, ());
                state.seat.pointer = Some(pointer);

                debug!("Found pointer");
            }
        }
    }
}
