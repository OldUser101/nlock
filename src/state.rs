use wayland_client::{
    delegate_noop, protocol::{wl_compositor, wl_keyboard, wl_output, wl_registry, wl_seat, wl_shm, wl_surface}, Connection, Dispatch, QueueHandle, WEnum
};
use wayland_protocols::ext::session_lock::v1::client::ext_session_lock_manager_v1;
use tracing::debug;

use crate::surface::NLockSurface;

pub struct NLockState {
    pub running: bool,
    pub compositor: Option<wl_compositor::WlCompositor>,
    pub shm: Option<wl_shm::WlShm>,
    pub seat: Option<wl_seat::WlSeat>,
    pub session_lock_manager: Option<ext_session_lock_manager_v1::ExtSessionLockManagerV1>,
    pub surfaces: Vec<NLockSurface>,
}

impl NLockState {
    pub fn new() -> Self {
        Self {
            running: true,
            compositor: None,
            shm: None,
            seat: None,
            session_lock_manager: None,
            surfaces: Vec::new(),
        }
    }
}

impl Default for NLockState {
    fn default() -> Self {
        Self::new()
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for NLockState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<NLockState>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match &interface[..] {
                "wl_compositor" => {
                    let compositor =
                        registry.bind::<wl_compositor::WlCompositor, _, _>(name, version, qh, ());
                    state.compositor = Some(compositor);
                }
                "wl_shm" => {
                    let shm = registry.bind::<wl_shm::WlShm, _, _>(name, version, qh, ());
                    state.shm = Some(shm);
                }
                "wl_seat" => {
                    let seat = registry.bind::<wl_seat::WlSeat, _, _>(name, version, qh, ());
                    state.seat = Some(seat);
                }
                "wl_output" => {
                    let surface = NLockSurface::new();
                    state.surfaces.push(surface);
                    let _ = registry.bind::<wl_output::WlOutput, _, _>(
                        name,
                        version,
                        qh,
                        state.surfaces.len() - 1,
                    );
                }
                "ext_session_lock_manager_v1" => {
                    let session_lock_manager = registry
                        .bind::<ext_session_lock_manager_v1::ExtSessionLockManagerV1, _, _>(
                        name,
                        version,
                        qh,
                        (),
                    );
                    state.session_lock_manager = Some(session_lock_manager);
                }
                _ => {}
            }
        }
    }
}

delegate_noop!(NLockState: ignore wl_compositor::WlCompositor);
delegate_noop!(NLockState: ignore wl_shm::WlShm);
delegate_noop!(NLockState: ignore wl_surface::WlSurface);
delegate_noop!(NLockState: ignore ext_session_lock_manager_v1::ExtSessionLockManagerV1);

impl Dispatch<wl_output::WlOutput, usize> for NLockState {
    fn event(
        state: &mut Self,
        _: &wl_output::WlOutput,
        event: <wl_output::WlOutput as wayland_client::Proxy>::Event,
        data: &usize,
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_output::Event::Geometry {
                x: _,
                y: _,
                physical_width: _,
                physical_height: _,
                subpixel,
                make: _,
                model: _,
                transform: _,
            } => {
                state.surfaces[*data].subpixel = Some(subpixel);
            }
            wl_output::Event::Name { name } => {
                debug!("Found output '{name}'");
                state.surfaces[*data].output_name = Some(name);
            }
            wl_output::Event::Scale { factor } => {
                state.surfaces[*data].output_scale = factor;
            }
            wl_output::Event::Done => {
                if let Some(compositor) = &state.compositor {
                    state.surfaces[*data].create_surface(compositor, qh);
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for NLockState {
    fn event(
        _: &mut Self,
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
                seat.get_keyboard(qh, ());
            }
        }
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
        if let wl_keyboard::Event::Key { key, .. } = event {
            if key == 1 {
                state.running = false;
            }
        }
    }
}
