// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

//! Surface hardware glue: GPIO/SPI2/ADC/DMA setup, the four LED-scan phase
//! callbacks driven by `leds::TIM3`, ADC bank scanning for the pressure
//! pads, and VBUS-based `PowerMode` detection.
//!
//! This is a clean-room reimplementation of the reference Launchpad Pro
//! firmware's surface driver, reverse engineered from the original firmware
//! disassembly. The key change versus a naive reimplementation is that the
//! LED shift-register transfer (SPI2, full duplex with the switch matrix
//! read-back) is driven by DMA (SPI2 TX = DMA1 Channel 5, SPI2 RX = DMA1
//! Channel 4) instead of a blocking busy-wait inside the TIM3 interrupt.
//! Blocking there stalls the CPU for a variable amount of time depending on
//! bus/interrupt contention, which stretches the very short "bright" pulses
//! used for low bit-planes (e.g. bit 0, ~2-5 ticks) by an amount that swamps
//! their nominal duration - this is what caused the visible low-brightness
//! flicker. Using DMA lets the transfer complete deterministically off the
//! CPU, and `leds.rs` gates unblanking on the transfer's completion
//! (`DMAFinished`) rather than assuming a fixed duration.

use core::ptr;

use crate::inputs::{GridEvent, Inputs};
use crate::leds::{self, Leds};

const GROUP_COUNT: usize = 4;
const SHIFT_BYTES_PER_SCAN: usize = 10;
const BRIGHT_BIT_ORDER: [usize; 6] = [0, 5, 1, 4, 2, 3];

const RAW_INDICES: [usize; GROUP_COUNT] = [0, 9, 17, 25];

const RCC_AHBENR: *mut u32 = 0x4002_1014 as *mut u32;
const RCC_APB2ENR: *mut u32 = 0x4002_1018 as *mut u32;
const RCC_APB1ENR: *mut u32 = 0x4002_101c as *mut u32;
const GPIOA_CRL: *mut u32 = 0x4001_0800 as *mut u32;
const GPIOB_CRL: *mut u32 = 0x4001_0c00 as *mut u32;
const GPIOB_CRH: *mut u32 = 0x4001_0c04 as *mut u32;
const GPIOB_IDR: *mut u32 = 0x4001_0c08 as *mut u32;
const GPIOC_CRL: *mut u32 = 0x4001_1000 as *mut u32;
const GPIOC_CRH: *mut u32 = 0x4001_1004 as *mut u32;
const GPIOD_CRL: *mut u32 = 0x4001_1400 as *mut u32;
const GPIOB_BSRR: *mut u32 = 0x4001_0c10 as *mut u32;
const GPIOB_BRR: *mut u32 = 0x4001_0c14 as *mut u32;
const GPIOC_BSRR: *mut u32 = 0x4001_1010 as *mut u32;
const GPIOC_BRR: *mut u32 = 0x4001_1014 as *mut u32;
const GPIOD_BSRR: *mut u32 = 0x4001_1410 as *mut u32;
const SPI2_CR1: *mut u32 = 0x4000_3800 as *mut u32;
const SPI2_CR2: *mut u32 = 0x4000_3804 as *mut u32;
const SPI2_DR: *mut u32 = 0x4000_380c as *mut u32;
const ADC1_SR: *mut u32 = 0x4001_2400 as *mut u32;
const ADC1_CR1: *mut u32 = 0x4001_2404 as *mut u32;
const ADC1_CR2: *mut u32 = 0x4001_2408 as *mut u32;
const ADC1_SMPR1: *mut u32 = 0x4001_240c as *mut u32;
const ADC1_SMPR2: *mut u32 = 0x4001_2410 as *mut u32;
const ADC1_SQR1: *mut u32 = 0x4001_242c as *mut u32;
const ADC1_SQR2: *mut u32 = 0x4001_2430 as *mut u32;
const ADC1_SQR3: *mut u32 = 0x4001_2434 as *mut u32;
const ADC1_DR: *mut u32 = 0x4001_244c as *mut u32;

const DMA1_IFCR: *mut u32 = 0x4002_0004 as *mut u32;
const DMA1_CCR1: *mut u32 = 0x4002_0008 as *mut u32;
const DMA1_CNDTR1: *mut u32 = 0x4002_000c as *mut u32;
const DMA1_CPAR1: *mut u32 = 0x4002_0010 as *mut u32;
const DMA1_CMAR1: *mut u32 = 0x4002_0014 as *mut u32;

// DMA1 Channel 4 = SPI2_RX, Channel 5 = SPI2_TX (fixed STM32F103 mapping).
const DMA1_CCR4: *mut u32 = 0x4002_0044 as *mut u32;
const DMA1_CNDTR4: *mut u32 = 0x4002_0048 as *mut u32;
const DMA1_CPAR4: *mut u32 = 0x4002_004c as *mut u32;
const DMA1_CMAR4: *mut u32 = 0x4002_0050 as *mut u32;
const DMA1_CCR5: *mut u32 = 0x4002_0058 as *mut u32;
const DMA1_CNDTR5: *mut u32 = 0x4002_005c as *mut u32;
const DMA1_CPAR5: *mut u32 = 0x4002_0060 as *mut u32;
const DMA1_CMAR5: *mut u32 = 0x4002_0064 as *mut u32;
const DMA1_CH4_ALL_FLAGS: u32 = 0xf << 12;
const DMA1_CH5_ALL_FLAGS: u32 = 0xf << 16;

const AFIOEN: u32 = 1 << 0;
const IOPAEN: u32 = 1 << 2;
const IOPBEN: u32 = 1 << 3;
const IOPCEN: u32 = 1 << 4;
const IOPDEN: u32 = 1 << 5;
const ADC1EN: u32 = 1 << 9;
const SPI2EN: u32 = 1 << 14;
const DMA1EN: u32 = 1 << 0;

const SPI_CR1_MSTR: u32 = 1 << 2;
const SPI_CR1_CPOL: u32 = 1 << 1;
const SPI_CR1_CPHA: u32 = 1 << 0;
const SPI_CR1_BR_0: u32 = 1 << 3;
const SPI_CR1_BR_1: u32 = 1 << 4;
const SPI_CR1_SPE: u32 = 1 << 6;
const SPI_CR1_LSBFIRST: u32 = 1 << 7;
const SPI_CR1_SSI: u32 = 1 << 8;
const SPI_CR1_SSM: u32 = 1 << 9;
const SPI_CR2_RXDMAEN: u32 = 1 << 0;
const SPI_CR2_TXDMAEN: u32 = 1 << 1;

const ADC_SR_EOC: u32 = 1 << 1;
const ADC_CR1_SCAN: u32 = 1 << 8;
const ADC_CR2_ADON: u32 = 1 << 0;
const ADC_CR2_CAL: u32 = 1 << 2;
const ADC_CR2_RSTCAL: u32 = 1 << 3;
const ADC_CR2_EXTTRIG: u32 = 1 << 20;
const ADC_CR2_SWSTART: u32 = 1 << 22;
const ADC_CR2_EXTSEL_SWSTART: u32 = 0b111 << 17;
const ADC_CR2_DMA: u32 = 1 << 8;
const DMA_CCR_EN: u32 = 1 << 0;
const DMA_CCR_TCIE: u32 = 1 << 1;
const DMA_CCR_DIR: u32 = 1 << 4;
const DMA_CCR_MINC: u32 = 1 << 7;
const DMA_CCR_PSIZE_16: u32 = 1 << 8;
const DMA_CCR_MSIZE_16: u32 = 1 << 10;

const DMA_IFCR_CTCIF1: u32 = 1 << 1;
const ADC_SEQUENCE: [u8; 16] = [11, 10, 13, 12, 1, 0, 3, 2, 5, 4, 7, 6, 15, 14, 8, 9];

// VBUS/PowerMode detection: GPIOB pin 9 (mask 0x200). High = self-powered
// (PowerMode 2), low = bus-powered (PowerMode 1). Confirmed only after 3
// consecutive agreeing samples, matching the reference firmware's filter.
const VBUS_PIN_MASK: u32 = 1 << 9;
const VBUS_CONFIRM_SAMPLES: u8 = 3;

pub struct Grid {
    inputs: Inputs,
    leds: Leds,
    selected_group: u8,
    selected_bit: u8,
    shift_tx: [u8; SHIFT_BYTES_PER_SCAN],
    shift_rx: [u8; SHIFT_BYTES_PER_SCAN],
    adc_bank: u8,
    adc_buffer: [u16; 16],
    setup_accum: bool,
    vbus_last_raw: bool,
    vbus_stable_count: u8,
    vbus_confirmed_high: bool,
}

impl Grid {
    pub fn new() -> Self {
        init_surface_hardware();
        init_adc_hardware();

        let initial_vbus_high = read_vbus_raw();

        let this = Self {
            inputs: Inputs::new(),
            leds: Leds::new(),
            selected_group: 0,
            selected_bit: 0,
            shift_tx: [0xff; SHIFT_BYTES_PER_SCAN],
            shift_rx: [0xff; SHIFT_BYTES_PER_SCAN],
            adc_bank: 0,
            adc_buffer: [0; 16],
            setup_accum: false,
            vbus_last_raw: initial_vbus_high,
            vbus_stable_count: VBUS_CONFIRM_SAMPLES,
            vbus_confirmed_high: initial_vbus_high,
        };
        this.blank_assert();
        this.deselect_all_groups();
        this.set_adc_bank_lines(0);

        // One-time unfiltered initial PowerMode assignment, matching the
        // reference's `init_exti` behaviour.
        leds::set_power_mode(if initial_vbus_high { 2 } else { 1 });

        this
    }

    pub fn poll_event(&mut self) -> Option<GridEvent> {
        self.inputs.poll_event()
    }

    // NOTE: LED buffer writes are deliberately NOT wrapped in a critical
    // section. The TIM3 scan ISR only ever *reads* the payload buffer, so
    // the worst case of a concurrent write is a torn read that shows one or
    // two LEDs a slightly wrong colour for a single ~sub-millisecond
    // subframe - visually imperceptible. Disabling interrupts here (as the
    // previous implementation did) instead stalls the TIM3 ISR for the whole
    // duration of the write, which stretches whichever "bright" pulse is
    // currently active and produces exactly the kind of update-correlated
    // flicker we are trying to eliminate. The reference firmware likewise
    // writes its LED buffer from the main context with no critical section.
    pub fn set_led(&mut self, index: u8, color: u32) {
        self.leds.set_led(index, color);
    }

    pub fn set_led_rgb(&mut self, index: u8, r: u8, g: u8, b: u8) {
        self.leds.set_led_rgb(index, r, g, b);
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

    // --- TIM3 phase callbacks, invoked from `leds::TIM3` -------------------

    /// SurfaceMode 0 (BLANK): assert blank, deselect every group line and
    /// build the shift-out payload for the (group, bright-bit) pair that is
    /// about to be scanned.
    pub fn blank_phase(&mut self) {
        self.blank_assert();
        self.deselect_all_groups();
        self.leds.build_group_payload(
            self.selected_group as usize,
            BRIGHT_BIT_ORDER[self.selected_bit as usize],
            &mut self.shift_tx,
        );
    }

    /// SurfaceMode 1 (NULLSURFACE): (re-)arm the DMA transfer-count
    /// registers for both the TX (LED data out, Channel 5) and RX (switch
    /// data in, Channel 4) sides, without starting the transfer yet.
    pub fn null_surface_phase(&mut self) {
        unsafe {
            modify_reg(DMA1_CCR4, |v| v & !DMA_CCR_EN);
            modify_reg(DMA1_CCR5, |v| v & !DMA_CCR_EN);
            write_reg(DMA1_IFCR, DMA1_CH4_ALL_FLAGS | DMA1_CH5_ALL_FLAGS);

            write_reg(DMA1_CPAR4, SPI2_DR as u32);
            write_reg(DMA1_CMAR4, self.shift_rx.as_ptr() as u32);
            write_reg(DMA1_CNDTR4, SHIFT_BYTES_PER_SCAN as u32);
            write_reg(DMA1_CCR4, DMA_CCR_MINC | DMA_CCR_TCIE);

            write_reg(DMA1_CPAR5, SPI2_DR as u32);
            write_reg(DMA1_CMAR5, self.shift_tx.as_ptr() as u32);
            write_reg(DMA1_CNDTR5, SHIFT_BYTES_PER_SCAN as u32);
            write_reg(DMA1_CCR5, DMA_CCR_MINC | DMA_CCR_DIR);
        }
    }

    /// SurfaceMode 2 (LEDSHIFT): start the SPI2 full-duplex DMA transfer.
    /// Non-blocking - completion is signalled asynchronously via the
    /// DMA1 Channel 4 (RX) interrupt, which sets `DMAFinished` in `leds.rs`.
    pub fn ledshift_phase(&mut self) {
        unsafe {
            // Enable RX before TX so the first incoming byte is never missed.
            modify_reg(DMA1_CCR4, |v| v | DMA_CCR_EN);
            modify_reg(DMA1_CCR5, |v| v | DMA_CCR_EN);
        }
    }

    /// SurfaceMode 3 (BRIGHT): only called once the previous shift's DMA
    /// transfer has completed. Releases blank, selects the current group's
    /// line, captures switch data from the just-completed shift-in on the
    /// first bright-bit of the frame, and advances the scan position.
    pub fn bright_phase(&mut self) -> u8 {
        let bright_step = self.selected_bit;
        self.blank_release();
        self.select_group(self.selected_group);
        if self.selected_bit == 0 {
            self.capture_switch_group(self.selected_group as usize);
        }
        self.advance_scan();
        bright_step
    }

    pub fn tick_1khz_collect(&mut self) {
        self.collect_adc_scan();
        self.inputs.tick_1khz();
        self.poll_power_mode();
    }

    pub fn tick_1khz_start(&mut self) {
        self.start_adc_scan();
    }

    pub fn tick_200hz(&mut self) {
        self.inputs.tick_200hz();
    }

    pub fn capture_pad_velocity(&mut self, sensor: usize, value: u8) {
        self.inputs.capture_pad_velocity(sensor, value);
    }

    pub fn capture_pad_aftertouch(&mut self, sensor: usize, value: u8) {
        self.inputs.capture_pad_aftertouch(sensor, value);
    }

    pub fn capture_switch_entry(&mut self, entry: usize, pressed: bool) {
        self.inputs.capture_switch_entry(entry, pressed);
    }

    fn capture_switch_group(&mut self, group: usize) {
        let raw_base = RAW_INDICES[group];
        let first = self.shift_rx[0];
        let second = self.shift_rx[1];

        self.inputs.set_switch_raw(raw_base, (first >> 7) & 1 != 0);
        self.inputs
            .set_switch_raw(raw_base + 1, (first >> 6) & 1 != 0);
        self.inputs
            .set_switch_raw(raw_base + 2, (first >> 5) & 1 != 0);
        self.inputs
            .set_switch_raw(raw_base + 3, (first >> 4) & 1 != 0);

        if (first >> 3) & 1 != 0 {
            self.setup_accum = true;
        }

        let second_base = if group == 0 {
            // raw_base + 4 is managed by the accumulator below
            raw_base + 5
        } else {
            raw_base + 4
        };

        if group == 3 {
            self.inputs.set_switch_raw(4, self.setup_accum);
            self.setup_accum = false;
        }

        self.inputs
            .set_switch_raw(second_base, (second >> 7) & 1 != 0);
        self.inputs
            .set_switch_raw(second_base + 1, (second >> 6) & 1 != 0);
        self.inputs
            .set_switch_raw(second_base + 2, (second >> 5) & 1 != 0);
        self.inputs
            .set_switch_raw(second_base + 3, (second >> 4) & 1 != 0);
    }

    fn advance_scan(&mut self) {
        self.selected_group = (self.selected_group + 1) % (GROUP_COUNT as u8);
        if self.selected_group == 0 {
            self.selected_bit += 1;
            if self.selected_bit >= 6 {
                self.selected_bit = 0;
            }
        }
    }

    pub fn start_adc_scan(&mut self) {
        unsafe {
            modify_reg(DMA1_CCR1, |val| val & !DMA_CCR_EN);
            write_reg(DMA1_CMAR1, self.adc_buffer.as_ptr() as u32);
            write_reg(DMA1_CNDTR1, 16);
            write_reg(DMA1_IFCR, DMA_IFCR_CTCIF1);
            modify_reg(DMA1_CCR1, |val| val | DMA_CCR_EN);
        }

        start_adc_conversion();
    }

    pub fn collect_adc_scan(&mut self) {
        unsafe { while read_reg(DMA1_CNDTR1) != 0 {} }

        self.inputs
            .capture_adc_bank(self.adc_bank as usize, &self.adc_buffer);
        self.adc_bank = (self.adc_bank + 1) & 3;
        self.set_adc_bank_lines(self.adc_bank);
        if self.adc_bank == 0 {
            self.inputs.accumulate_adc_max();
        }
    }

    /// Debounced VBUS read: only updates `PowerMode` once 3 consecutive
    /// samples agree on a new level, matching the reference firmware's
    /// filter and avoiding spurious PowerMode flips from a noisy/bouncing
    /// VBUS line while a cable is being inserted or removed.
    fn poll_power_mode(&mut self) {
        let raw = read_vbus_raw();
        if raw == self.vbus_last_raw {
            if self.vbus_stable_count < VBUS_CONFIRM_SAMPLES {
                self.vbus_stable_count += 1;
            }
        } else {
            self.vbus_last_raw = raw;
            self.vbus_stable_count = 1;
        }

        if self.vbus_stable_count >= VBUS_CONFIRM_SAMPLES && self.vbus_confirmed_high != raw {
            self.vbus_confirmed_high = raw;
            leds::set_power_mode(if raw { 2 } else { 1 });
        }
    }

    fn set_adc_bank_lines(&self, bank: u8) {
        unsafe {
            match bank & 3 {
                0 => write_reg(GPIOC_BRR, (1 << 8) | (1 << 9)),
                1 => {
                    write_reg(GPIOC_BSRR, 1 << 9);
                    write_reg(GPIOC_BRR, 1 << 8);
                }
                2 => {
                    write_reg(GPIOC_BRR, 1 << 9);
                    write_reg(GPIOC_BSRR, 1 << 8);
                }
                _ => write_reg(GPIOC_BSRR, (1 << 8) | (1 << 9)),
            }
        }
    }

    fn blank_assert(&self) {
        unsafe {
            write_reg(GPIOB_BSRR, 1 << 12);
            write_reg(GPIOB_BRR, 1 << 8);
        }
    }

    fn blank_release(&self) {
        unsafe {
            write_reg(GPIOB_BRR, 1 << 12);
            write_reg(GPIOB_BSRR, 1 << 8);
        }
    }

    fn deselect_all_groups(&self) {
        unsafe {
            write_reg(GPIOC_BSRR, (1 << 10) | (1 << 11) | (1 << 12));
            write_reg(GPIOD_BSRR, 1 << 2);
        }
    }

    fn select_group(&self, group: u8) {
        unsafe {
            match group {
                0 => write_reg(GPIOC_BSRR, 1 << (10 + 16)),
                1 => write_reg(GPIOC_BSRR, 1 << (11 + 16)),
                2 => write_reg(GPIOC_BSRR, 1 << (12 + 16)),
                _ => write_reg(GPIOD_BSRR, 1 << (2 + 16)),
            }
        }
    }
}

fn read_vbus_raw() -> bool {
    unsafe { read_reg(GPIOB_IDR) & VBUS_PIN_MASK != 0 }
}

fn init_surface_hardware() {
    unsafe {
        modify_reg(RCC_APB2ENR, |value| {
            value | AFIOEN | IOPAEN | IOPBEN | IOPCEN | IOPDEN
        });
        modify_reg(RCC_APB1ENR, |value| value | SPI2EN);

        write_reg(GPIOA_CRL, 0);

        let gpiob_crl = read_reg(GPIOB_CRL);
        let gpiob_crl = set_pin_mode(gpiob_crl, 0, 0b0000);
        let gpiob_crl = set_pin_mode(gpiob_crl, 1, 0b0000);
        let gpiob_crl = set_pin_mode(gpiob_crl, 3, 0b0100);
        let gpiob_crl = set_pin_mode(gpiob_crl, 4, 0b0100);
        let gpiob_crl = set_pin_mode(gpiob_crl, 5, 0b0001);
        let gpiob_crl = set_pin_mode(gpiob_crl, 6, 0b0001);
        let gpiob_crl = set_pin_mode(gpiob_crl, 7, 0b0001);
        write_reg(GPIOB_CRL, gpiob_crl);

        let gpiob_crh = read_reg(GPIOB_CRH);
        let gpiob_crh = set_pin_mode(gpiob_crh, 8, 0b0001);
        let gpiob_crh = set_pin_mode(gpiob_crh, 9, 0b0100);
        let gpiob_crh = set_pin_mode(gpiob_crh, 10, 0b1001);
        let gpiob_crh = set_pin_mode(gpiob_crh, 11, 0b0100);
        let gpiob_crh = set_pin_mode(gpiob_crh, 12, 0b0001);
        let gpiob_crh = set_pin_mode(gpiob_crh, 13, 0b1001);
        let gpiob_crh = set_pin_mode(gpiob_crh, 14, 0b1000);
        let gpiob_crh = set_pin_mode(gpiob_crh, 15, 0b1001);
        write_reg(GPIOB_CRH, gpiob_crh);

        let gpioc_crl = read_reg(GPIOC_CRL);
        let gpioc_crl = set_pin_mode(gpioc_crl, 0, 0b0000);
        let gpioc_crl = set_pin_mode(gpioc_crl, 1, 0b0000);
        let gpioc_crl = set_pin_mode(gpioc_crl, 2, 0b0000);
        let gpioc_crl = set_pin_mode(gpioc_crl, 3, 0b0000);
        let gpioc_crl = set_pin_mode(gpioc_crl, 4, 0b0000);
        let gpioc_crl = set_pin_mode(gpioc_crl, 5, 0b0000);
        let gpioc_crl = set_pin_mode(gpioc_crl, 7, 0b0001);
        write_reg(GPIOC_CRL, gpioc_crl);

        let gpioc_crh = read_reg(GPIOC_CRH);
        let gpioc_crh = set_pin_mode(gpioc_crh, 8, 0b0001);
        let gpioc_crh = set_pin_mode(gpioc_crh, 9, 0b0001);
        let gpioc_crh = set_pin_mode(gpioc_crh, 10, 0b0001);
        let gpioc_crh = set_pin_mode(gpioc_crh, 11, 0b0001);
        let gpioc_crh = set_pin_mode(gpioc_crh, 12, 0b0001);
        let gpioc_crh = set_pin_mode(gpioc_crh, 13, 0b0001);
        let gpioc_crh = set_pin_mode(gpioc_crh, 14, 0b0001);
        let gpioc_crh = set_pin_mode(gpioc_crh, 15, 0b0001);
        write_reg(GPIOC_CRH, gpioc_crh);

        let gpiod_crl = read_reg(GPIOD_CRL);
        let gpiod_crl = set_pin_mode(gpiod_crl, 2, 0b0001);
        write_reg(GPIOD_CRL, gpiod_crl);

        write_reg(GPIOB_BRR, 0xffff);
        write_reg(GPIOB_BSRR, (1 << 14) | (1 << 8));
        write_reg(GPIOC_BSRR, (0xffff << 16) | 0x1c80);
        write_reg(GPIOD_BSRR, (0xffff << 16) | (1 << 2));

        write_reg(
            SPI2_CR1,
            SPI_CR1_MSTR
                | SPI_CR1_CPOL
                | SPI_CR1_CPHA
                | SPI_CR1_BR_0
                | SPI_CR1_BR_1
                | SPI_CR1_LSBFIRST
                | SPI_CR1_SSM
                | SPI_CR1_SSI
                | SPI_CR1_SPE,
        );
        // Enable SPI2's DMA request lines once; individual transfers are
        // gated purely by each DMA channel's own EN bit (see
        // null_surface_phase/ledshift_phase).
        write_reg(SPI2_CR2, SPI_CR2_RXDMAEN | SPI_CR2_TXDMAEN);
    }
}

fn init_adc_hardware() {
    unsafe {
        modify_reg(RCC_AHBENR, |value| value | DMA1EN);
        modify_reg(RCC_APB2ENR, |value| value | ADC1EN);

        write_reg(ADC1_CR1, ADC_CR1_SCAN);
        write_reg(ADC1_SMPR1, 0x00ff_ffff);
        write_reg(ADC1_SMPR2, 0xffff_ffff);
        write_reg(ADC1_SQR1, 15 << 20);
        write_reg(ADC1_SQR3, pack_sequence(&ADC_SEQUENCE[0..6]));
        write_reg(ADC1_SQR2, pack_sequence(&ADC_SEQUENCE[6..12]));
        write_reg(ADC1_SQR1, (15 << 20) | pack_sequence(&ADC_SEQUENCE[12..16]));

        write_reg(DMA1_CPAR1, ADC1_DR as u32);
        write_reg(
            DMA1_CCR1,
            DMA_CCR_MINC | DMA_CCR_PSIZE_16 | DMA_CCR_MSIZE_16,
        );

        write_reg(
            ADC1_CR2,
            ADC_CR2_ADON | ADC_CR2_EXTTRIG | ADC_CR2_EXTSEL_SWSTART | ADC_CR2_DMA,
        );
        modify_reg(ADC1_CR2, |value| value | ADC_CR2_RSTCAL);
        while read_reg(ADC1_CR2) & ADC_CR2_RSTCAL != 0 {}
        modify_reg(ADC1_CR2, |value| value | ADC_CR2_CAL);
        while read_reg(ADC1_CR2) & ADC_CR2_CAL != 0 {}
    }
}

fn start_adc_conversion() {
    unsafe {
        modify_reg(ADC1_SR, |value| value & !ADC_SR_EOC);
        modify_reg(ADC1_CR2, |value| {
            value | ADC_CR2_ADON | ADC_CR2_EXTTRIG | ADC_CR2_EXTSEL_SWSTART
        });
        modify_reg(ADC1_CR2, |value| value | ADC_CR2_SWSTART);
    }
}

fn pack_sequence(channels: &[u8]) -> u32 {
    let mut value = 0u32;
    for (index, &channel) in channels.iter().enumerate() {
        value |= (channel as u32) << (index * 5);
    }
    value
}

fn set_pin_mode(register: u32, pin: u8, mode: u32) -> u32 {
    let shift = ((pin % 8) as u32) * 4;
    (register & !(0xf << shift)) | (mode << shift)
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
    let value = unsafe { ptr::read_volatile(reg) };
    unsafe {
        ptr::write_volatile(reg, f(value));
    }
}
