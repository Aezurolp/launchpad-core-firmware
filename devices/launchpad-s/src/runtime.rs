use crate::surface::Surface;
use crate::usb;
use firmware_core::driver::Driver;
use firmware_core::sys::midi::MidiPort;

pub struct RuntimeDriver {
    surface: *mut Surface,
}

impl RuntimeDriver {
    pub const fn new(surface: *mut Surface) -> Self {
        Self { surface }
    }

    pub fn set_surface(&mut self, surface: *mut Surface) {
        self.surface = surface;
    }

    fn surface(&mut self) -> &mut Surface {
        unsafe { &mut *self.surface }
    }
}

impl Driver for RuntimeDriver {
    fn set_rgb_led(&mut self, index: u8, r: u8, g: u8, b: u8) {
        self.surface().set_rgb_led(index, r, g, b);
    }

    fn fill(&mut self, color: u32) {
        self.surface().fill(color);
    }

    fn brightness(&mut self) -> u8 {
        self.surface().brightness()
    }

    fn set_brightness(&mut self, brightness: u8) {
        self.surface().set_brightness(brightness);
    }

    fn send_midi(&mut self, port: MidiPort, data: &[u8]) {
        match port {
            MidiPort::Din => {}
            MidiPort::Daw | MidiPort::Midi => {
                let _ = usb::enqueue_tx_message(port as u8, data);
            }
        }
    }

    fn flash_size(&mut self) -> u32 {
        0
    }

    fn read_flash(&mut self, _offset: u32, data: &mut [u8]) {
        data.fill(0xff);
    }

    fn write_flash(&mut self, _offset: u32, _data: &[u8]) {}

    fn device_id(&self) -> u8 {
        32
    }
}
