use core::ptr;
use core::sync::atomic::{AtomicPtr, AtomicU8, Ordering};

use embassy_stm32::interrupt::{self, InterruptExt};

use crate::grid::Grid;

const LP_LED_COUNT: usize = 100;
const LED_STATUS_BYTES: usize = 320;
const GROUP_COUNT: usize = 4;
const BRIGHT_BIT_COUNT: usize = 6;
const GROUP_STRIDE: usize = 0x50;
const SHIFT_BYTES_PER_GROUP: usize = 10;

const GREEN_MAP: [u16; LP_LED_COUNT] = [
    0, 23, 103, 183, 263, 20, 100, 180, 260, 0, 250, 29, 109, 189, 269, 26, 106, 186, 266, 242,
    170, 35, 115, 195, 275, 32, 112, 192, 272, 162, 90, 42, 122, 202, 282, 36, 116, 196, 276, 82,
    10, 47, 127, 207, 287, 44, 124, 204, 284, 2, 253, 55, 135, 215, 295, 51, 131, 211, 291, 245,
    173, 56, 136, 216, 296, 57, 137, 217, 297, 165, 93, 66, 146, 226, 306, 60, 140, 220, 300, 85,
    13, 71, 151, 231, 311, 68, 148, 228, 308, 5, 0, 77, 157, 237, 317, 74, 154, 234, 314, 17,
];
const RED_MAP: [u16; LP_LED_COUNT] = [
    0, 24, 104, 184, 264, 21, 101, 181, 261, 0, 251, 30, 110, 190, 270, 27, 107, 187, 267, 243,
    171, 38, 118, 198, 278, 33, 113, 193, 273, 163, 91, 41, 121, 201, 281, 37, 117, 197, 277, 83,
    11, 48, 128, 208, 288, 45, 125, 205, 285, 3, 254, 54, 134, 214, 294, 50, 130, 210, 290, 246,
    174, 62, 142, 222, 302, 58, 138, 218, 298, 166, 94, 65, 145, 225, 305, 61, 141, 221, 301, 86,
    14, 72, 152, 232, 312, 69, 149, 229, 309, 6, 0, 78, 158, 238, 318, 75, 155, 235, 315, 18,
];
const BLUE_MAP: [u16; LP_LED_COUNT] = [
    0, 25, 105, 185, 265, 22, 102, 182, 262, 0, 252, 31, 111, 191, 271, 28, 108, 188, 268, 244,
    172, 39, 119, 199, 279, 34, 114, 194, 274, 164, 92, 40, 120, 200, 280, 43, 123, 203, 283, 84,
    12, 49, 129, 209, 289, 46, 126, 206, 286, 4, 255, 53, 133, 213, 293, 52, 132, 212, 292, 247,
    175, 63, 143, 223, 303, 59, 139, 219, 299, 167, 95, 64, 144, 224, 304, 67, 147, 227, 307, 87,
    15, 73, 153, 233, 313, 70, 150, 230, 310, 7, 0, 79, 159, 239, 319, 76, 156, 236, 316, 19,
];

const RAW_RED_OFFSET: usize = 0;
const RAW_GREEN_OFFSET: usize = LP_LED_COUNT;
const RAW_BLUE_OFFSET: usize = LP_LED_COUNT * 2;
const RAW_RGB_SIZE: usize = LP_LED_COUNT * 3;

fn apply_led_brightness(value: u8, brightness: u8) -> u8 {
    let value = value.min(0x3f);
    if value == 0 {
        return 0;
    }

    let x = brightness.min(7) as u32;
    ((((value as u32 - 1) * (63 * (x + 5) * (x + 5) - 144)) / (62 * 144)) + 1) as u8
}

pub struct Leds {
    payload: [[[u8; SHIFT_BYTES_PER_GROUP]; BRIGHT_BIT_COUNT]; GROUP_COUNT],
    raw_rgb: [u8; RAW_RGB_SIZE],
    brightness: u8,
}

impl Leds {
    pub fn new() -> Self {
        Self {
            payload: [[[0xff; SHIFT_BYTES_PER_GROUP]; BRIGHT_BIT_COUNT]; GROUP_COUNT],
            raw_rgb: [0; RAW_RGB_SIZE],
            brightness: 7,
        }
    }

    pub fn clear(&mut self) {
        self.fill(0);
    }

    pub fn fill(&mut self, rgb: u32) {
        for i in 0..LP_LED_COUNT {
            self.set_led(i as u8, rgb);
        }
    }

    pub fn set_led(&mut self, led: u8, rgb: u32) {
        if led as usize >= LP_LED_COUNT {
            return;
        }

        let rgb = rgb & 0x00ff_ffff;
        let r = ((rgb >> 18) & 0x3f) as u8;
        let g = ((rgb >> 10) & 0x3f) as u8;
        let b = ((rgb >> 2) & 0x3f) as u8;
        self.set_led_rgb(led, r, g, b);
    }

    pub fn set_led_rgb(&mut self, led: u8, r: u8, g: u8, b: u8) {
        if led as usize >= LP_LED_COUNT {
            return;
        }

        let led = led as usize;
        self.raw_rgb[RAW_RED_OFFSET + led] = r.min(0x3f);
        self.raw_rgb[RAW_GREEN_OFFSET + led] = g.min(0x3f);
        self.raw_rgb[RAW_BLUE_OFFSET + led] = b.min(0x3f);
        self.write_scaled_led(led);
    }

    pub fn build_group_payload(&self, group: usize, bright_bit: usize, out: &mut [u8; 10]) {
        if group >= GROUP_COUNT || bright_bit >= BRIGHT_BIT_COUNT {
            out.fill(0xff);
            return;
        }

        out.copy_from_slice(&self.payload[group][bright_bit]);
    }

    pub fn brightness(&self) -> u8 {
        self.brightness
    }

    pub fn set_brightness(&mut self, brightness: u8) {
        self.brightness = brightness.min(7);
        self.rebuild_scaled_payload();
    }

    fn write_scaled_led(&mut self, led: usize) {
        let r = self.scale_intensity(self.raw_rgb[RAW_RED_OFFSET + led]);
        let g = self.scale_intensity(self.raw_rgb[RAW_GREEN_OFFSET + led]);
        let b = self.scale_intensity(self.raw_rgb[RAW_BLUE_OFFSET + led]);

        self.write_status(RED_MAP[led], r);
        self.write_status(GREEN_MAP[led], g);
        self.write_status(BLUE_MAP[led], b);
    }

    fn rebuild_scaled_payload(&mut self) {
        self.payload = [[[0xff; SHIFT_BYTES_PER_GROUP]; BRIGHT_BIT_COUNT]; GROUP_COUNT];
        for led in 0..LP_LED_COUNT {
            self.write_scaled_led(led);
        }
    }

    fn scale_intensity(&self, value: u8) -> u8 {
        apply_led_brightness(value, self.brightness)
    }

    fn write_status(&mut self, bit_index: u16, value: u8) {
        let offset = bit_index as usize;
        if offset >= LED_STATUS_BYTES {
            return;
        }

        let group = offset / GROUP_STRIDE;
        if group >= GROUP_COUNT {
            return;
        }

        let group_offset = offset - group * GROUP_STRIDE;
        let byte_index = group_offset >> 3;
        if byte_index >= SHIFT_BYTES_PER_GROUP {
            return;
        }

        let bit_mask = 1u8 << (group_offset & 7);
        for bright_bit in 0..BRIGHT_BIT_COUNT {
            let dst = &mut self.payload[group][bright_bit][byte_index];
            if value & (1 << bright_bit) != 0 {
                *dst &= !bit_mask;
            } else {
                *dst |= bit_mask;
            }
        }
    }
}

const RCC_APB1ENR: *mut u32 = 0x4002_101c as *mut u32;
const TIM3_CR1: *mut u32 = 0x4000_0400 as *mut u32;
const TIM3_DIER: *mut u32 = 0x4000_040c as *mut u32;
const TIM3_SR: *mut u32 = 0x4000_0410 as *mut u32;
const TIM3_EGR: *mut u32 = 0x4000_0414 as *mut u32;
const TIM3_CNT: *mut u32 = 0x4000_0424 as *mut u32;
const TIM3_PSC: *mut u32 = 0x4000_0428 as *mut u32;
const TIM3_ARR: *mut u32 = 0x4000_042c as *mut u32;

const RCC_APB1ENR_TIM3EN: u32 = 1 << 1;
const TIM_CR1_CEN: u32 = 1 << 0;
const TIM_CR1_URS: u32 = 1 << 2;
const TIM_CR1_DIR: u32 = 1 << 4;
const TIM_DIER_UIE: u32 = 1 << 0;
const TIM_SR_UIF: u32 = 1 << 0;
const TIM_EGR_UG: u32 = 1 << 0;

const TIMER_VALUE: [u16; 3] = [10, 10, 50];
const BRIGHT_TIMES_VBUS_POWER: [u16; 6] = [5, 935, 35, 455, 95, 215];
const TIM3_PERIOD_TICKS: u16 = 59;
const TIM3_PRESCALER: u16 = 10;

static GRID: AtomicPtr<Grid> = AtomicPtr::new(ptr::null_mut());
static SURFACE_MODE: AtomicU8 = AtomicU8::new(0);

pub fn start_scan(grid: *mut Grid) {
    GRID.store(grid, Ordering::Release);
    SURFACE_MODE.store(0, Ordering::Relaxed);

    unsafe {
        modify_reg(RCC_APB1ENR, |value| value | RCC_APB1ENR_TIM3EN);
        modify_reg(TIM3_CR1, |value| value & !TIM_CR1_CEN);
        modify_reg(TIM3_CR1, |value| {
            (value & !TIM_CR1_CEN) | TIM_CR1_URS | TIM_CR1_DIR
        });
        write_reg(TIM3_PSC, TIM3_PRESCALER as u32);
        write_reg(TIM3_ARR, TIM3_PERIOD_TICKS as u32);
        write_reg(TIM3_CNT, TIMER_VALUE[0] as u32);
        write_reg(TIM3_EGR, TIM_EGR_UG);
        write_reg(TIM3_SR, 0);
        modify_reg(TIM3_DIER, |value| value | TIM_DIER_UIE);
    }

    interrupt::TIM3.unpend();
    unsafe {
        interrupt::TIM3.enable();
    }

    unsafe {
        modify_reg(TIM3_CR1, |value| value | TIM_CR1_CEN);
    }
}

#[cortex_m_rt::interrupt]
fn TIM3() {
    if unsafe { read_reg(TIM3_SR) } & TIM_SR_UIF == 0 {
        return;
    }
    unsafe {
        write_reg(TIM3_SR, 0);
    }

    let grid = GRID.load(Ordering::Acquire);
    if grid.is_null() {
        return;
    }
    let grid = unsafe { &mut *grid };

    let mode = SURFACE_MODE.load(Ordering::Relaxed);
    let next_delay = match mode {
        0 => {
            grid.blank_phase();
            TIMER_VALUE[0]
        }
        1 => {
            grid.null_surface_phase();
            TIMER_VALUE[1]
        }
        2 => {
            grid.ledshift_phase();
            TIMER_VALUE[2]
        }
        _ => {
            let step = grid.bright_phase() as usize;
            BRIGHT_TIMES_VBUS_POWER[step]
        }
    };

    SURFACE_MODE.store((mode + 1) & 3, Ordering::Relaxed);
    unsafe {
        write_reg(TIM3_CNT, next_delay as u32);
    }
}

unsafe fn read_reg(reg: *mut u32) -> u32 {
    unsafe { ptr::read_volatile(reg) }
}

unsafe fn write_reg(reg: *mut u32, value: u32) {
    unsafe {
        ptr::write_volatile(reg, value);
    }
}

unsafe fn modify_reg(reg: *mut u32, f: impl FnOnce(u32) -> u32) {
    unsafe {
        let value = ptr::read_volatile(reg);
        ptr::write_volatile(reg, f(value));
    }
}
