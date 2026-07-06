use firmware_core::app::{AftertouchEvent, App, MidiEvent, MidiPort, SurfaceEvent};
use firmware_core::driver;
use firmware_core::sys::led;

pub struct LiveApp;

impl LiveApp {
    pub const fn new() -> Self {
        Self
    }
}

impl App for LiveApp {
    fn on_enter(&mut self) {
        for i in 0..4 {
            led::set(95 + i, 0x101010);
        }

        // temporary: session mode led green
        led::set(95, 0xccffcc);
    }

    fn on_exit(&mut self) {}

    fn on_surface(&mut self, event: SurfaceEvent) {
        if event.index > 90 && event.index < 99 {
            driver::send_midi(
                MidiPort::Daw,
                &[176, event.index, if event.pressed { 127 } else { 0 }],
            )
        }
    }

    fn on_midi(&mut self, event: MidiEvent) {
        match event.status {
            0x90 => {
                led::novation(event.data1, event.data2);
            }

            0x80 => {
                led::novation(event.data1, 0);
            }

            0xb0 => {
                if (90..=99).contains(&event.data1) {
                    led::set_palette(event.data1, event.data2);
                }
            }

            _ => {}
        }
    }

    fn on_aftertouch(&mut self, _event: AftertouchEvent) {}

    fn on_tick(&mut self) {}
}

pub const fn new() -> LiveApp {
    LiveApp::new()
}
