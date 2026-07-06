use firmware_core::app::{AftertouchEvent, App, MidiEvent, MidiPort, SurfaceEvent};
use firmware_core::driver;

pub struct LiveApp;

impl LiveApp {
    pub const fn new() -> Self {
        Self
    }
}

impl App for LiveApp {
    fn on_enter(&mut self) {
        driver::fill(0);
    }

    fn on_exit(&mut self) {}

    fn on_surface(&mut self, event: SurfaceEvent) {
        if event.pressed {
            let value = event.value.max(10) / 2;
            driver::set_rgb_led(event.index, value, value / 2, 0);
            driver::send_midi(MidiPort::Daw, &[0x90, event.index, event.value]);
        } else {
            driver::set_rgb_led(event.index, 0, 0, 0);
            driver::send_midi(MidiPort::Daw, &[0x80, event.index, 0]);
        }
    }

    fn on_midi(&mut self, _event: MidiEvent) {}

    fn on_aftertouch(&mut self, event: AftertouchEvent) {
        driver::send_midi(MidiPort::Daw, &[0xa0, event.index, event.value]);
    }

    fn on_tick(&mut self) {}
}
