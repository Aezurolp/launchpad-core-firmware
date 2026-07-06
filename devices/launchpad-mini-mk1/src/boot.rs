use firmware_core::app::{AftertouchEvent, App, AppId, MidiEvent, SurfaceEvent};
use firmware_core::driver;

pub struct BootApp {
    tick: u16,
    next_app: Option<AppId>,
}

impl BootApp {
    pub const fn new() -> Self {
        Self {
            tick: 0,
            next_app: None,
        }
    }
}

impl App for BootApp {
    fn on_enter(&mut self) {
        self.tick = 0;
        self.next_app = None;
    }

    fn on_exit(&mut self) {}

    fn on_surface(&mut self, event: SurfaceEvent) {
        if event.pressed {
            driver::set_rgb_led(event.index, 0, 63, 0);
        } else {
            driver::set_rgb_led(event.index, 0, 0, 0);
        }
    }

    fn on_midi(&mut self, _event: MidiEvent) {}

    fn on_aftertouch(&mut self, _event: AftertouchEvent) {}

    fn on_tick(&mut self) {}

    fn take_requested_app_switch(&mut self) -> Option<AppId> {
        self.next_app.take()
    }
}
