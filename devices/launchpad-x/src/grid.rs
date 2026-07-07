// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

use core::ptr;

use crate::inputs::{GridEvent, Inputs};
use crate::leds::Leds;

const SCAN_ROW_LUT: [u8; 96] = [
    0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 3, 0, 1, 2, 3, 0, 1, 2,
    3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1,
    2, 3, 0, 1, 2, 3, 0, 1, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0,
];
const SCAN_PHASE_LUT: [u8; 96] = [
    0, 3, 2, 4, 5, 0, 3, 2, 1, 5, 0, 3, 4, 1, 5, 0, 2, 4, 1, 5, 3, 2, 4, 1, 4, 0, 3, 2, 2, 5, 0, 3,
    3, 1, 5, 0, 0, 4, 1, 5, 5, 2, 4, 1, 1, 3, 2, 4, 2, 4, 0, 3, 3, 2, 5, 0, 0, 3, 1, 5, 5, 0, 4, 1,
    1, 5, 2, 4, 4, 1, 3, 2, 3, 2, 4, 0, 0, 3, 2, 5, 5, 0, 3, 1, 1, 5, 0, 4, 4, 1, 5, 2, 2, 4, 1, 3,
];
const BRIGHTNESS_BASE_LUT: [u64; 9] = [0x77, 0x6d, 0x62, 0x58, 0x4d, 0x43, 0x38, 0x2d, 0x22];
const BRIGHTNESS_PHASE_LUT: [[u64; 6]; 9] = [
    [0x0005, 0x0008, 0x000e, 0x001a, 0x0032, 0x0062],
    [0x0006, 0x000a, 0x0012, 0x0022, 0x0042, 0x0082],
    [0x0007, 0x000c, 0x0016, 0x002a, 0x0052, 0x00a2],
    [0x0008, 0x000e, 0x001a, 0x0032, 0x0062, 0x00c2],
    [0x0009, 0x0010, 0x001e, 0x003a, 0x0072, 0x00e2],
    [0x000a, 0x0012, 0x0022, 0x0042, 0x0082, 0x0102],
    [0x000b, 0x0014, 0x0026, 0x004a, 0x0092, 0x0122],
    [0x000c, 0x0016, 0x002a, 0x0052, 0x00a2, 0x0142],
    [0x000d, 0x0018, 0x002e, 0x005a, 0x00b2, 0x0162],
];
const ROW_MASK: [u16; 4] = [1 << 0, 1 << 1, 1 << 2, 1 << 10];
const MUX_SET_MASK: [u16; 8] = [
    0x0000, 0x0020, 0x0040, 0x0060, 0x0080, 0x00a0, 0x00c0, 0x00e0,
];
const MUX_RESET_MASK: [u16; 8] = [
    0x00e0, 0x00c0, 0x00a0, 0x0080, 0x0060, 0x0040, 0x0020, 0x0000,
];

const RCC_AHB1ENR: *mut u32 = 0x4002_3830 as *mut u32;
const RCC_APB1ENR: *mut u32 = 0x4002_3840 as *mut u32;
const GPIOB_BASE: u32 = 0x4002_0400;
const GPIOC_BASE: u32 = 0x4002_0800;
const SPI3_BASE: u32 = 0x4000_3c00;
const MODER: u32 = 0x00;
const OSPEEDR: u32 = 0x08;
const PUPDR: u32 = 0x0c;
const IDR: u32 = 0x10;
const BSRR: u32 = 0x18;
const AFRH: u32 = 0x24;
const SPI_CR1: *mut u32 = (SPI3_BASE + 0x00) as *mut u32;
const SPI_SR: *mut u32 = (SPI3_BASE + 0x08) as *mut u32;
const SPI_DR: *mut u32 = (SPI3_BASE + 0x0c) as *mut u32;

const RCC_AHB1ENR_GPIOBEN: u32 = 1 << 1;
const RCC_AHB1ENR_GPIOCEN: u32 = 1 << 2;
const RCC_APB1ENR_SPI3EN: u32 = 1 << 15;
const SPI_CR1_MSTR: u32 = 1 << 2;
const SPI_CR1_BR_1: u32 = 1 << 4;
const SPI_CR1_SPE: u32 = 1 << 6;
const SPI_CR1_SSI: u32 = 1 << 8;
const SPI_CR1_SSM: u32 = 1 << 9;
const SPI_SR_RXNE: u32 = 1 << 0;
const SPI_SR_TXE: u32 = 1 << 1;
const SPI_SR_BSY: u32 = 1 << 7;

pub struct Grid {
    scan_slot: u8,
    mux_bank: u8,
    active_mux_bank: u8,
    pressure_pending_bank: u8,
    pressure_pending_valid: bool,
    leds: Leds,
    inputs: Inputs,
}

impl Grid {
    pub fn new() -> Self {
        let mut inputs = Inputs::new();
        init_led_hardware();
        inputs.init_hardware();

        let mut this = Self {
            scan_slot: 0,
            mux_bank: 0,
            active_mux_bank: 0,
            pressure_pending_bank: 0,
            pressure_pending_valid: false,
            leds: Leds::new(),
            inputs,
        };
        this.leds.fill(0x00ff00);
        this
    }

    pub fn prepare_phase(&mut self) {
        if self.pressure_pending_valid
            && self
                .inputs
                .finish_pressure_capture(self.pressure_pending_bank)
        {
            self.pressure_pending_valid = false;
        }

        let row = SCAN_ROW_LUT[self.scan_slot as usize] as usize;
        let phase = SCAN_PHASE_LUT[self.scan_slot as usize] as usize;
        let start = row * 8;
        let prev_slot =
            ((self.scan_slot as usize) + SCAN_PHASE_LUT.len() - 1) % SCAN_PHASE_LUT.len();
        let group = ((prev_slot >> 2) & 0x03) as u8;
        let capture_row = SCAN_ROW_LUT[prev_slot];
        let mux_bank = self.mux_bank;

        self.inputs
            .capture_side(group, capture_row, sample_side_inputs());

        unsafe {
            gpio_reset(GPIOC_BASE, 1 << 11);
            gpio_set(GPIOB_BASE, (1 << 0) | (1 << 1) | (1 << 2) | (1 << 10));
            gpio_reset(GPIOB_BASE, MUX_RESET_MASK[mux_bank as usize]);
            gpio_set(GPIOB_BASE, MUX_SET_MASK[mux_bank as usize]);
        }

        self.active_mux_bank = mux_bank;
        self.mux_bank = (mux_bank + 1) & 0x07;

        spi3_transfer_8(&self.leds.fb[phase][start..start + 8]);
        self.inputs.start_pressure_capture(self.active_mux_bank);
    }

    pub fn drive_phase(&mut self) {
        let row = SCAN_ROW_LUT[self.scan_slot as usize] as usize;
        let phase = SCAN_PHASE_LUT[self.scan_slot as usize];
        let phase_mask = 1u8 << phase;

        unsafe {
            write_active_low(GPIOC_BASE, 1 << 7, (self.leds.overlay_r & phase_mask) != 0);
            write_active_low(GPIOC_BASE, 1 << 8, (self.leds.overlay_g & phase_mask) != 0);
            write_active_low(GPIOC_BASE, 1 << 9, (self.leds.overlay_b & phase_mask) != 0);
            gpio_set(GPIOC_BASE, 1 << 11);
            gpio_reset(GPIOB_BASE, ROW_MASK[row]);
        }

        self.pressure_pending_bank = self.active_mux_bank;
        self.pressure_pending_valid = true;
    }

    pub fn advance_slot(&mut self) {
        self.scan_slot += 1;
        if self.scan_slot as usize >= SCAN_PHASE_LUT.len() {
            self.scan_slot = 0;
        }
    }

    pub fn frame_complete(&self) -> bool {
        self.scan_slot == 0
    }

    pub fn poll_event(&mut self) -> Option<GridEvent> {
        self.inputs.pop_event()
    }

    pub fn set_led(&mut self, index: u8, color: u32) {
        self.leds.set_led(flip_index(index), color);
    }

    pub fn set_led_rgb(&mut self, index: u8, r: u8, g: u8, b: u8) {
        self.leds.set_led_rgb(flip_index(index), r, g, b);
    }

    pub fn fill(&mut self, color: u32) {
        self.leds.fill(color);
    }

    pub fn brightness(&self) -> u8 {
        self.leds.brightness()
    }

    pub fn set_brightness(&mut self, brightness: u8) {
        self.leds.set_brightness(brightness);
    }

    pub fn prepare_delay_us(&self) -> u64 {
        BRIGHTNESS_BASE_LUT[self.brightness_index()]
    }

    pub fn drive_delay_us(&self) -> u64 {
        let phase = SCAN_PHASE_LUT[self.scan_slot as usize] as usize;
        BRIGHTNESS_PHASE_LUT[self.brightness_index()][phase]
    }

    fn brightness_index(&self) -> usize {
        let raw = (self.leds.brightness().min(8) as u16 * 255 / 8) as u8;
        ((9 * raw as u16) >> 8) as usize
    }
}

fn flip_index(index: u8) -> u8 {
    (index % 10) + (9 - (index / 10)) * 10
}

fn init_led_hardware() {
    unsafe {
        modify_reg(RCC_AHB1ENR, |value| {
            value | RCC_AHB1ENR_GPIOBEN | RCC_AHB1ENR_GPIOCEN
        });
        modify_reg(RCC_APB1ENR, |value| value | RCC_APB1ENR_SPI3EN);

        configure_gpio_af(GPIOC_BASE, 10, 6);
        configure_gpio_af(GPIOC_BASE, 12, 6);

        for pin in [0, 1, 2, 5, 6, 7, 10] {
            configure_gpio_output(GPIOB_BASE, pin);
        }
        for pin in [7, 8, 9, 11] {
            configure_gpio_output(GPIOC_BASE, pin);
        }
        for pin in [0, 1, 2, 3] {
            configure_gpio_input(GPIOC_BASE, pin);
        }

        gpio_set(GPIOB_BASE, (1 << 0) | (1 << 1) | (1 << 2) | (1 << 10));
        gpio_set(GPIOC_BASE, (1 << 7) | (1 << 8) | (1 << 9));
        gpio_reset(GPIOC_BASE, 1 << 11);
        gpio_reset(GPIOB_BASE, (1 << 5) | (1 << 6) | (1 << 7));

        write_reg(
            SPI_CR1,
            SPI_CR1_MSTR | SPI_CR1_SSM | SPI_CR1_SSI | SPI_CR1_BR_1,
        );
        modify_reg(SPI_CR1, |value| value | SPI_CR1_SPE);
    }
}

unsafe fn configure_gpio_af(port: u32, pin: u8, af: u8) {
    unsafe {
        let mode_shift = (pin as u32) * 2;
        modify_reg((port + MODER) as *mut u32, |value| {
            (value & !(0b11 << mode_shift)) | (0b10 << mode_shift)
        });
        modify_reg((port + OSPEEDR) as *mut u32, |value| {
            (value & !(0b11 << mode_shift)) | (0b11 << mode_shift)
        });
        modify_reg((port + PUPDR) as *mut u32, |value| {
            value & !(0b11 << mode_shift)
        });

        let afr_shift = ((pin - 8) as u32) * 4;
        modify_reg((port + AFRH) as *mut u32, |value| {
            (value & !(0x0f << afr_shift)) | ((af as u32) << afr_shift)
        });
    }
}

unsafe fn configure_gpio_output(port: u32, pin: u8) {
    unsafe {
        let shift = (pin as u32) * 2;
        modify_reg((port + MODER) as *mut u32, |value| {
            (value & !(0b11 << shift)) | (0b01 << shift)
        });
        modify_reg((port + OSPEEDR) as *mut u32, |value| {
            (value & !(0b11 << shift)) | (0b11 << shift)
        });
        modify_reg((port + PUPDR) as *mut u32, |value| value & !(0b11 << shift));
    }
}

unsafe fn configure_gpio_input(port: u32, pin: u8) {
    unsafe {
        let shift = (pin as u32) * 2;
        modify_reg((port + MODER) as *mut u32, |value| value & !(0b11 << shift));
        modify_reg((port + PUPDR) as *mut u32, |value| value & !(0b11 << shift));
    }
}

fn sample_side_inputs() -> u16 {
    let idr = unsafe { read_reg((GPIOC_BASE + IDR) as *mut u32) };
    let mut sample = 0u16;

    if idr & (1 << 0) != 0 {
        sample |= 0x0001;
    }
    if idr & (1 << 1) != 0 {
        sample |= 0x0010;
    }
    if idr & (1 << 2) != 0 {
        sample |= 0x0100;
    }
    if idr & (1 << 3) != 0 {
        sample |= 0x1000;
    }

    sample
}

fn spi3_transfer_8(tx: &[u8]) {
    if tx.len() < 8 {
        return;
    }

    unsafe {
        for byte in &tx[..8] {
            while read_reg(SPI_SR) & SPI_SR_TXE == 0 {}
            ptr::write_volatile(SPI_DR as *mut u8, *byte);

            while read_reg(SPI_SR) & SPI_SR_RXNE == 0 {}
            let _ = ptr::read_volatile(SPI_DR as *mut u8);
        }

        while read_reg(SPI_SR) & SPI_SR_BSY != 0 {}
        let _ = read_reg(SPI_DR);
        let _ = read_reg(SPI_SR);
    }
}

unsafe fn write_active_low(port: u32, pin: u16, low: bool) {
    unsafe {
        if low {
            gpio_reset(port, pin);
        } else {
            gpio_set(port, pin);
        }
    }
}

unsafe fn gpio_set(port: u32, pins: u16) {
    unsafe {
        write_reg((port + BSRR) as *mut u32, pins as u32);
    }
}

unsafe fn gpio_reset(port: u32, pins: u16) {
    unsafe {
        write_reg((port + BSRR) as *mut u32, (pins as u32) << 16);
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
