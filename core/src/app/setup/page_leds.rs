use crate::app::SurfaceEvent;
use crate::app::setup::page::Page;
use crate::app::setup::text::Text;
use crate::driver;
use crate::sys::settings;

const BRIGHTNESS_START: u8 = 31;
const BRIGHTNESS_END: u8 = 38;

pub struct LedsPage {
    text: Text,
}

impl LedsPage {
    pub const fn new() -> Self {
        Self {
            text: Text::new(
                [0b10111110, 0b10110101, 0b10100101, 0b11111110],
                0b11000111,
                0x2020ff,
                0xddddff,
            ),
        }
    }

    fn draw_brightness_selector(&self) {
        let brightness = driver::brightness().min(7);

        for level in 0..8 {
            driver::set_led(BRIGHTNESS_START + level, 0x101010);
        }

        driver::set_led(BRIGHTNESS_START + brightness, 0xccccff);
    }

    #[cfg(feature = "launchpad-pro-mk3")]
    fn draw_mirror_toggle(&self) {
        let mirror_enabled = settings::with(|s| s.mirror_enabled);
        driver::set_led(
            11,
            if mirror_enabled != 0 {
                0x10ff10
            } else {
                0xff1010
            },
        );
    }
}

impl Page for LedsPage {
    fn on_enter(&mut self) {
        driver::set_led(79, 0x0a0a55);

        self.text.draw();
        self.draw_brightness_selector();
        #[cfg(feature = "launchpad-pro-mk3")]
        self.draw_mirror_toggle();
    }

    fn on_surface(&mut self, event: SurfaceEvent) {
        #[cfg(feature = "launchpad-pro-mk3")]
        if event.pressed && event.index == 11 {
            settings::update(|s| {
                s.mirror_enabled = if s.mirror_enabled != 0 { 0 } else { 1 };
            });
            self.draw_mirror_toggle();
            return;
        }

        if !event.pressed || event.index < BRIGHTNESS_START || event.index > BRIGHTNESS_END {
            return;
        }

        let brightness = event.index - BRIGHTNESS_START;

        driver::set_brightness(brightness);
        settings::update(|settings| {
            settings.brightness = brightness;
        });

        self.draw_brightness_selector();
    }
}
