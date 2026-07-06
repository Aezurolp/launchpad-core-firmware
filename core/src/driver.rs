use core::cell::UnsafeCell;
use core::mem;

use crate::sys::midi::MidiPort;

pub trait Driver {
    fn set_rgb_led(&mut self, index: u8, r: u8, g: u8, b: u8);
    fn set_led(&mut self, index: u8, color: u32) {
        self.set_rgb_led(
            index,
            ((color >> 18) & 0x3f) as u8,
            ((color >> 10) & 0x3f) as u8,
            ((color >> 2) & 0x3f) as u8,
        );
    }
    fn fill(&mut self, color: u32);
    fn brightness(&mut self) -> u8;
    fn set_brightness(&mut self, brightness: u8);
    fn send_midi(&mut self, port: MidiPort, data: &[u8]);
    fn flash_size(&mut self) -> u32;
    fn read_flash(&mut self, offset: u32, data: &mut [u8]);
    fn write_flash(&mut self, offset: u32, data: &[u8]);
    fn device_id(&self) -> u8;
    fn highspeed_leds_enabled(&self) -> bool {
        false
    }
}

struct DriverSlot {
    ptr: UnsafeCell<Option<*mut dyn Driver>>,
}

unsafe impl Sync for DriverSlot {}

impl DriverSlot {
    const fn new() -> Self {
        Self {
            ptr: UnsafeCell::new(None),
        }
    }

    fn install(&self, driver: &mut dyn Driver) {
        unsafe {
            let erased: *mut dyn Driver =
                mem::transmute::<&mut dyn Driver, *mut dyn Driver>(driver);
            *self.ptr.get() = Some(erased);
        }
    }

    fn with<R>(&self, f: impl FnOnce(&mut dyn Driver) -> R) -> Option<R> {
        unsafe {
            let slot = &mut *self.ptr.get();
            slot.as_mut().map(|ptr| f(&mut **ptr))
        }
    }
}

static DRIVER: DriverSlot = DriverSlot::new();

pub fn install(driver: &mut dyn Driver) {
    DRIVER.install(driver);
}

pub fn with<R>(f: impl FnOnce(&mut dyn Driver) -> R) -> Option<R> {
    DRIVER.with(f)
}

pub fn set_rgb_led(index: u8, r: u8, g: u8, b: u8) {
    let _ = with(|driver| driver.set_rgb_led(index, r, g, b));
}

pub fn set_led_raw(index: u8, r: u8, g: u8, b: u8) {
    set_rgb_led(index, r, g, b);
}

pub fn set_led(index: u8, color: u32) {
    let _ = with(|driver| {
        driver.set_rgb_led(
            index,
            ((color >> 18) & 0x3f) as u8,
            ((color >> 10) & 0x3f) as u8,
            ((color >> 2) & 0x3f) as u8,
        )
    });
}

pub fn fill(color: u32) {
    let _ = with(|driver| driver.fill(color));
}

pub fn brightness() -> u8 {
    with(|driver| driver.brightness()).unwrap_or(7)
}

pub fn set_brightness(brightness: u8) {
    let _ = with(|driver| driver.set_brightness(brightness));
}

pub fn send_midi(port: MidiPort, data: &[u8]) {
    let _ = with(|driver| driver.send_midi(port, data));
}

pub fn flash_size() -> u32 {
    with(|driver| driver.flash_size()).unwrap_or(0)
}

pub fn read_flash(offset: u32, data: &mut [u8]) {
    let _ = with(|driver| driver.read_flash(offset, data));
}

pub fn write_flash(offset: u32, data: &[u8]) {
    let _ = with(|driver| driver.write_flash(offset, data));
}

pub fn device_id() -> u8 {
    with(|driver| driver.device_id()).unwrap_or(0)
}

pub fn highspeed_leds_enabled() -> bool {
    with(|driver| driver.highspeed_leds_enabled()).unwrap_or(false)
}
