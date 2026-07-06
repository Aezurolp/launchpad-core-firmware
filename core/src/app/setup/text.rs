use crate::sys::led;

pub(crate) struct Text {
    line_mask: [u8; 4],
    color_mask: u8,
    primary_color: u32,
    secondary_color: u32,
}

impl Text {
    pub const fn new(
        line_mask: [u8; 4],
        color_mask: u8,
        primary_color: u32,
        secondary_color: u32,
    ) -> Self {
        Self {
            line_mask,
            color_mask,
            primary_color,
            secondary_color,
        }
    }

    pub fn draw(&mut self) {
        for y in 0..4 {
            for x in 0..8 {
                let pos: u8 = 88 - (y * 10);
                let shape_active = self.line_mask[y as usize] & (1 << x) != 0;
                let use_primary = self.color_mask & (1 << x) != 0;

                if !shape_active {
                    continue;
                }

                if use_primary {
                    led::set(pos - x, self.primary_color)
                } else {
                    led::set(pos - x, self.secondary_color)
                }
            }
        }
    }
}
