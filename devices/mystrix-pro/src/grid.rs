// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

use esp_hal::Blocking;
use esp_hal::analog::adc::{Adc, AdcCalBasic, AdcConfig, AdcPin, Attenuation};
use esp_hal::delay::Delay;
use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull};
use esp_hal::peripherals::{ADC1, GPIO2, GPIO3, GPIO4, GPIO5, GPIO7, GPIO8, GPIO9, GPIO10};
use heapless::Deque;
use crate::board::{BoardConfig, Revision, LED_COUNT};

#[derive(Clone, Copy)]
pub enum HardwareEvent {
    Surface { pressed: bool, index: u8, value: u8 },
    Aftertouch { index: u8, value: u8 },
}

/// Maps the physical LC8812 chain to the Launchpad Pro 10x10 coordinate IDs.
pub const PHYSICAL_TO_LOGICAL: [u8; LED_COUNT] = [
    81, 82, 83, 84, 85, 86, 87, 88, 71, 72, 73, 74, 75, 76, 77, 78, 
    61, 62, 63, 64, 65, 66, 67, 68, 51, 52, 53, 54, 55, 56, 57, 58, 
    41, 42, 43, 44, 45, 46, 47, 48, 31, 32, 33, 34, 35, 36, 37, 38, 
    21, 22, 23, 24, 25, 26, 27, 28, 11, 12, 13, 14, 15, 16, 17, 18, 
    19, 29, 39, 49, 59, 69, 79, 89, 98, 97, 96, 95, 94, 93, 92, 91, 
    80, 70, 60, 50, 40, 30, 20, 10, 1, 2, 3, 4, 5, 6, 7, 8
];

pub const fn physical_to_logical(index: usize) -> Option<u8> {
    if index < LED_COUNT {
        Some(PHYSICAL_TO_LOGICAL[index])
    } else {
        None
    }
}

pub fn logical_to_physical(index: u8) -> Option<usize> {
    PHYSICAL_TO_LOGICAL.iter().position(|value| *value == index)
}

pub const fn pad_logical_index(x: usize, y: usize) -> Option<u8> {
    if x < 8 && y < 8 {
        Some(((8 - y) * 10 + x + 1) as u8)
    } else {
        None
    }
}

pub const fn touch_logical_index(segment: u8) -> Option<u8> {
    match segment {
        0..=7 => Some((8 - segment) * 10),
        8..=15 => Some((16 - segment) * 10 + 9),
        _ => None,
    }
}

// Ultra-sensitive profile requested for Mystrix Pro. Pressing engages at
// 128 counts; releasing at 64 retains 64 counts of hysteresis.
pub const LOW_THRESHOLD: u16 = 64;
pub const HIGH_THRESHOLD: u16 = 8_192;
pub const ACTIVATION_OFFSET: u16 = 64;
pub const DEBOUNCE_MS: u32 = 4;
pub const AFTERTOUCH_DELTA: u8 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PressureEvent {
    Press { velocity: u8 },
    Release,
    Aftertouch { pressure: u8 },
}

#[derive(Clone, Copy)]
pub struct PadState {
    active: bool,
    edge_since_ms: u32,
    candidate: bool,
    baseline: u16,
    peak_slope: u16,
    last_raw: u16,
    last_aftertouch: u8,
}

impl PadState {
    pub const fn new() -> Self {
        Self {
            active: false,
            edge_since_ms: 0,
            candidate: false,
            baseline: 0,
            peak_slope: 0,
            last_raw: 0,
            last_aftertouch: 0,
        }
    }

    pub fn update(
        &mut self,
        now_ms: u32,
        stable: u16,
        filtered: u16,
        low: u16,
        high: u16,
    ) -> Option<PressureEvent> {
        let press_threshold = low.saturating_add(ACTIVATION_OFFSET);
        if !self.active {
            if stable > press_threshold {
                if !self.candidate {
                    self.candidate = true;
                    self.edge_since_ms = now_ms;
                    self.baseline = self.last_raw.min(low);
                    self.peak_slope = stable.saturating_sub(self.last_raw);
                } else {
                    self.peak_slope = self.peak_slope.max(stable.saturating_sub(self.last_raw));
                }
                if now_ms.wrapping_sub(self.edge_since_ms) >= DEBOUNCE_MS {
                    self.active = true;
                    self.candidate = false;
                    self.last_aftertouch = normalize_pressure(filtered, low, high);
                    self.last_raw = stable;
                    return Some(PressureEvent::Press {
                        velocity: velocity_from_slope(
                            self.peak_slope,
                            stable.saturating_sub(self.baseline),
                        ),
                    });
                }
            } else {
                self.candidate = false;
            }
        } else if stable <= low {
            if !self.candidate {
                self.candidate = true;
                self.edge_since_ms = now_ms;
            } else if now_ms.wrapping_sub(self.edge_since_ms) >= DEBOUNCE_MS {
                self.active = false;
                self.candidate = false;
                self.last_aftertouch = 0;
                self.last_raw = stable;
                return Some(PressureEvent::Release);
            }
        } else {
            self.candidate = false;
            let pressure = normalize_pressure(filtered, low, high);
            if pressure.abs_diff(self.last_aftertouch) >= AFTERTOUCH_DELTA
                || (pressure == 127 && self.last_aftertouch != 127)
            {
                self.last_aftertouch = pressure;
                self.last_raw = stable;
                return Some(PressureEvent::Aftertouch { pressure });
            }
        }
        self.last_raw = stable;
        None
    }
}

impl Default for PadState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn normalize_pressure(raw: u16, low: u16, high: u16) -> u8 {
    if raw <= low {
        return 0;
    }
    let range = high.saturating_sub(low).max(1) as u32;
    (((raw - low) as u32 * 127) / range).min(127) as u8
}

fn velocity_from_slope(slope: u16, excursion: u16) -> u8 {
    let slope_velocity = ((slope as u32 * 127) / 4_096).min(127);
    let excursion_velocity = ((excursion as u32 * 127) / 8_192).min(127);
    ((slope_velocity * 3 + excursion_velocity) / 4).clamp(1, 127) as u8
}

type Adc1 = ADC1<'static>;
type CalibratedAdcPin<PIN> = AdcPin<PIN, Adc1, AdcCalBasic<Adc1>>;

pub enum AdcChannels {
    Ch0(CalibratedAdcPin<GPIO2<'static>>),
    Ch1(CalibratedAdcPin<GPIO3<'static>>),
    Ch2(CalibratedAdcPin<GPIO4<'static>>),
    Ch3(CalibratedAdcPin<GPIO5<'static>>),
    Ch4(CalibratedAdcPin<GPIO7<'static>>),
    Ch5(CalibratedAdcPin<GPIO8<'static>>),
    Ch6(CalibratedAdcPin<GPIO9<'static>>),
    Ch7(CalibratedAdcPin<GPIO10<'static>>),
}

pub struct Grid {
    adc: Adc<'static, Adc1, Blocking>,
    adc_pins: [AdcChannels; 8],
    rows: [Output<'static>; 8],
    fn_button: Input<'static>,
    touch_data: Input<'static>,
    touch_clock: Output<'static>,
    touch_map: [u8; 16],
    filtered: [u16; 64],
    pad_states: [PadState; 64],
    touch_integrator: [u8; 16],
    touch_active: [bool; 16],
    fn_integrator: u8,
    fn_active: bool,
    events: Deque<HardwareEvent, 128>,
}

impl Grid {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        adc1: ADC1<'static>,
        gpio1: esp_hal::peripherals::GPIO1<'static>,
        gpio2: GPIO2<'static>,
        gpio3: GPIO3<'static>,
        gpio4: GPIO4<'static>,
        gpio5: GPIO5<'static>,
        gpio6: esp_hal::peripherals::GPIO6<'static>,
        gpio7: GPIO7<'static>,
        gpio8: GPIO8<'static>,
        gpio9: GPIO9<'static>,
        gpio10: GPIO10<'static>,
        gpio12: esp_hal::peripherals::GPIO12<'static>,
        gpio13: esp_hal::peripherals::GPIO13<'static>,
        gpio14: esp_hal::peripherals::GPIO14<'static>,
        gpio15: esp_hal::peripherals::GPIO15<'static>,
        gpio16: esp_hal::peripherals::GPIO16<'static>,
        gpio17: esp_hal::peripherals::GPIO17<'static>,
        gpio21: esp_hal::peripherals::GPIO21<'static>,
        gpio33: esp_hal::peripherals::GPIO33<'static>,
        gpio34: esp_hal::peripherals::GPIO34<'static>,
        gpio47: esp_hal::peripherals::GPIO47<'static>,
        config: BoardConfig,
    ) -> Self {
        let mut adc_config = AdcConfig::new();
        let adc_pins = [
            AdcChannels::Ch0(adc_config.enable_pin_with_cal::<_, AdcCalBasic<Adc1>>(gpio2, Attenuation::_11dB)),
            AdcChannels::Ch1(adc_config.enable_pin_with_cal::<_, AdcCalBasic<Adc1>>(gpio3, Attenuation::_11dB)),
            AdcChannels::Ch2(adc_config.enable_pin_with_cal::<_, AdcCalBasic<Adc1>>(gpio4, Attenuation::_11dB)),
            AdcChannels::Ch3(adc_config.enable_pin_with_cal::<_, AdcCalBasic<Adc1>>(gpio5, Attenuation::_11dB)),
            AdcChannels::Ch4(adc_config.enable_pin_with_cal::<_, AdcCalBasic<Adc1>>(gpio7, Attenuation::_11dB)),
            AdcChannels::Ch5(adc_config.enable_pin_with_cal::<_, AdcCalBasic<Adc1>>(gpio8, Attenuation::_11dB)),
            AdcChannels::Ch6(adc_config.enable_pin_with_cal::<_, AdcCalBasic<Adc1>>(gpio9, Attenuation::_11dB)),
            AdcChannels::Ch7(adc_config.enable_pin_with_cal::<_, AdcCalBasic<Adc1>>(gpio10, Attenuation::_11dB)),
        ];
        let adc = Adc::new(adc1, adc_config);

        let output_config = OutputConfig::default();
        let rows = [
            Output::new(gpio21, Level::Low, output_config),
            Output::new(gpio17, Level::Low, output_config),
            Output::new(gpio1, Level::Low, output_config),
            Output::new(gpio6, Level::Low, output_config),
            Output::new(gpio12, Level::Low, output_config),
            Output::new(gpio13, Level::Low, output_config),
            Output::new(gpio14, Level::Low, output_config),
            Output::new(gpio15, Level::Low, output_config),
        ];
        let fn_button = Input::new(gpio16, InputConfig::default().with_pull(Pull::Up));

        let (touch_data, touch_clock) = match config.revision {
            Revision::V100 => {
                let data = Input::new(gpio33, InputConfig::default().with_pull(Pull::Down));
                let clock = Output::new(gpio34, Level::Low, output_config);
                let _unused = Input::new(gpio47, InputConfig::default());
                (data, clock)
            }
            Revision::V110 | Revision::RevC => {
                let data = Input::new(gpio47, InputConfig::default().with_pull(Pull::Down));
                let clock = Output::new(gpio33, Level::Low, output_config);
                let _unused = Input::new(gpio34, InputConfig::default());
                (data, clock)
            }
        };

        Self {
            adc,
            adc_pins,
            rows,
            fn_button,
            touch_data,
            touch_clock,
            touch_map: config.touch_map,
            filtered: [0; 64],
            pad_states: [PadState::new(); 64],
            touch_integrator: [0; 16],
            touch_active: [false; 16],
            fn_integrator: 0,
            fn_active: false,
            events: Deque::new(),
        }
    }

    pub fn fn_held(&self) -> bool {
        self.fn_button.is_low()
    }

    pub fn scan_controls(&mut self, now_ms: u32, scan_touch: bool) {
        self.scan_fn();
        if scan_touch {
            self.scan_touch();
        }
        self.scan_grid(now_ms);
    }

    pub fn poll_event(&mut self) -> Option<HardwareEvent> {
        self.events.pop_front()
    }

    pub fn pressure_levels(&self) -> &[u16; 64] {
        &self.filtered
    }

    fn scan_fn(&mut self) {
        let pressed = self.fn_button.is_low();
        let events = &mut self.events;
        debounce_binary(
            pressed,
            &mut self.fn_integrator,
            &mut self.fn_active,
            |active| {
                let _ = events.push_back(HardwareEvent::Surface {
                    pressed: active,
                    index: 0,
                    value: if active { 127 } else { 0 },
                });
            },
        );
    }

    fn scan_touch(&mut self) {
        let events = &mut self.events;
        for serial_index in 0..16 {
            self.touch_clock.set_high();
            let pressed = self.touch_data.is_high();
            self.touch_clock.set_low();

            let segment = self.touch_map[serial_index] as usize;
            debounce_binary(
                pressed,
                &mut self.touch_integrator[segment],
                &mut self.touch_active[segment],
                |active| {
                    if let Some(index) = touch_logical_index(segment as u8) {
                        let _ = events.push_back(HardwareEvent::Surface {
                            pressed: active,
                            index,
                            value: if active { 127 } else { 0 },
                        });
                    }
                },
            );
        }
    }

    fn scan_grid(&mut self, now_ms: u32) {
        let delay = Delay::new();
        for x in 0..8 {
            self.rows[x].set_high();
            delay.delay_micros(2);
            for y in 0..8 {
                let raw = self.stable_adc_read(y);
                let sensor = y * 8 + x;
                let filtered = if self.filtered[sensor] == 0 {
                    raw
                } else {
                    ((self.filtered[sensor] as u32 * 3 + raw as u32) / 4) as u16
                };
                self.filtered[sensor] = filtered;

                if let Some(event) = self.pad_states[sensor].update(
                    now_ms,
                    raw,
                    filtered,
                    LOW_THRESHOLD,
                    HIGH_THRESHOLD,
                ) {
                    let index = pad_logical_index(x, y).unwrap();
                    let hardware_event = match event {
                        PressureEvent::Press { velocity } => HardwareEvent::Surface {
                            pressed: true,
                            index,
                            value: velocity,
                        },
                        PressureEvent::Release => HardwareEvent::Surface {
                            pressed: false,
                            index,
                            value: 0,
                        },
                        PressureEvent::Aftertouch { pressure } => HardwareEvent::Aftertouch {
                            index,
                            value: pressure,
                        },
                    };
                    let _ = self.events.push_back(hardware_event);
                }
            }
            self.rows[x].set_low();
        }
    }

    fn stable_adc_read(&mut self, channel: usize) -> u16 {
        let first = scale_adc(self.read_adc(channel));
        if first <= 640 {
            return first;
        }
        let second = scale_adc(self.read_adc(channel));
        if first.abs_diff(second) <= 192 {
            return second;
        }
        let third = scale_adc(self.read_adc(channel));
        ((second as u32 + third as u32) / 2) as u16
    }

    fn read_adc(&mut self, channel: usize) -> u16 {
        match &mut self.adc_pins[channel] {
            AdcChannels::Ch0(pin) => self.adc.read_blocking(pin),
            AdcChannels::Ch1(pin) => self.adc.read_blocking(pin),
            AdcChannels::Ch2(pin) => self.adc.read_blocking(pin),
            AdcChannels::Ch3(pin) => self.adc.read_blocking(pin),
            AdcChannels::Ch4(pin) => self.adc.read_blocking(pin),
            AdcChannels::Ch5(pin) => self.adc.read_blocking(pin),
            AdcChannels::Ch6(pin) => self.adc.read_blocking(pin),
            AdcChannels::Ch7(pin) => self.adc.read_blocking(pin),
        }
    }
}

fn scale_adc(reading: u16) -> u16 {
    reading.saturating_mul(16).saturating_add(reading >> 8)
}

fn debounce_binary(
    mut pressed: bool,
    integrator: &mut u8,
    active: &mut bool,
    mut emit: impl FnMut(bool),
) {
    if pressed {
        *integrator = integrator.saturating_add(1).min(3);
    } else {
        *integrator = integrator.saturating_sub(1);
    }
    pressed = *integrator == 3;
    let released = *integrator == 0;
    if pressed && !*active {
        *active = true;
        emit(true);
    } else if released && *active {
        *active = false;
        emit(false);
    }
}

