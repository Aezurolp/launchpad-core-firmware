// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

use heapless::spsc::{Consumer, Producer, Queue};
use static_cell::StaticCell;

pub const PAD_COUNT: usize = 64;
pub const SIDE_BUTTON_COUNT: usize = 32;
pub const SWITCH_ENTRY_COUNT: usize = SIDE_BUTTON_COUNT + 1;
pub const SETUP_INDEX: u8 = 0;
pub const NO_BUTTON: u8 = 0xff;
pub const PRESS_VALUE: u8 = 127;

const EVENT_QUEUE_SIZE: usize = 64;
const SWITCH_RELEASE_DELAY: u8 = 0x28;
const ADC_BANK_SIZE: usize = 16;
const PAD_PRESS_START: u16 = 0x100;
const PAD_PRESS_PEAK_START: u16 = PAD_PRESS_START + 0x50;
const PAD_PRESS_END: u16 = 0x3ff;
const PAD_AFTERTOUCH_START: u16 = 0x200;
const PAD_AFTERTOUCH_END: u16 = 0x301;
const PAD_AFTERTOUCH_RANGE: u16 = PAD_AFTERTOUCH_END - PAD_AFTERTOUCH_START;
const PAD_HOLDOFF_TICKS: u8 = 0x10;
const PAD_RELEASE_HOLDOFF_TICKS: u8 = 4;

const PADADC: [u8; PAD_COUNT] = [
    49, 51, 53, 55, 57, 59, 61, 63, 33, 35, 37, 39, 41, 43, 45, 47, 17, 19, 21, 23, 25, 27, 29, 31,
    1, 3, 5, 7, 9, 11, 13, 15, 48, 50, 52, 54, 56, 58, 60, 62, 32, 34, 36, 38, 40, 42, 44, 46, 16,
    18, 20, 22, 24, 26, 28, 30, 0, 2, 4, 6, 8, 10, 12, 14,
];

pub const PAD_SENSOR_TO_INDEX: [u8; PAD_COUNT] = [
    81, 82, 83, 84, 85, 86, 87, 88, 71, 72, 73, 74, 75, 76, 77, 78, 61, 62, 63, 64, 65, 66, 67, 68,
    51, 52, 53, 54, 55, 56, 57, 58, 41, 42, 43, 44, 45, 46, 47, 48, 31, 32, 33, 34, 35, 36, 37, 38,
    21, 22, 23, 24, 25, 26, 27, 28, 11, 12, 13, 14, 15, 16, 17, 18,
];

pub const SWITCH_TO_INDEX: [u8; SWITCH_ENTRY_COUNT] = [
    // Group 0
    94,
    98,
    50,
    10,
    SETUP_INDEX,
    4,
    8,
    59,
    19,
    // Group 1
    91,
    95,
    80,
    40,
    1,
    5,
    89,
    49,
    // Group 2
    92,
    96,
    70,
    30,
    2,
    6,
    79,
    39,
    // Group 3
    93,
    97,
    60,
    20,
    3,
    7,
    69,
    29,
];

#[derive(Copy, Clone)]
pub enum GridEvent {
    Press { index: u8, value: u8 },
    Release { index: u8 },
    Aftertouch { index: u8, value: u8 },
}

pub struct Inputs {
    producer: Producer<'static, GridEvent>,
    consumer: Consumer<'static, GridEvent>,
    adc_direct: [u16; PAD_COUNT],
    adc_max: [u16; PAD_COUNT],
    pads_pressed: [bool; PAD_COUNT],
    pad_state: [u8; PAD_COUNT],
    pad_time: [u8; PAD_COUNT],
    last_aftertouch: [u8; PAD_COUNT],
    switches_raw: [u8; SWITCH_ENTRY_COUNT],
    switches_processed: [u8; SWITCH_ENTRY_COUNT],
}

impl Inputs {
    pub fn new() -> Self {
        static QUEUE: StaticCell<Queue<GridEvent, EVENT_QUEUE_SIZE>> = StaticCell::new();
        let (producer, consumer) = QUEUE.init(Queue::new()).split();

        Self {
            producer,
            consumer,
            adc_direct: [0; PAD_COUNT],
            adc_max: [0; PAD_COUNT],
            pads_pressed: [false; PAD_COUNT],
            pad_state: [0; PAD_COUNT],
            pad_time: [0; PAD_COUNT],
            last_aftertouch: [0; PAD_COUNT],
            switches_raw: [0; SWITCH_ENTRY_COUNT],
            switches_processed: [0; SWITCH_ENTRY_COUNT],
        }
    }

    pub fn poll_event(&mut self) -> Option<GridEvent> {
        self.consumer.dequeue()
    }

    pub fn capture_pad_velocity(&mut self, sensor: usize, value: u8) {
        let Some(&index) = PAD_SENSOR_TO_INDEX.get(sensor) else {
            return;
        };

        let pressed = value != 0;
        if self.pads_pressed[sensor] == pressed {
            return;
        }

        self.pads_pressed[sensor] = pressed;
        let event = if pressed {
            GridEvent::Press { index, value }
        } else {
            GridEvent::Release { index }
        };
        let _ = self.producer.enqueue(event);
    }

    pub fn capture_pad_aftertouch(&mut self, sensor: usize, value: u8) {
        let Some(&index) = PAD_SENSOR_TO_INDEX.get(sensor) else {
            return;
        };

        let _ = self
            .producer
            .enqueue(GridEvent::Aftertouch { index, value });
    }

    pub fn capture_switch_entry(&mut self, entry: usize, pressed: bool) {
        let Some(&index) = SWITCH_TO_INDEX.get(entry) else {
            return;
        };

        if index == NO_BUTTON {
            return;
        }

        let state = &mut self.switches_processed[entry];
        let event = if pressed {
            switch_is_on(state).then_some(GridEvent::Press {
                index,
                value: PRESS_VALUE,
            })
        } else {
            switch_is_off(state, SWITCH_RELEASE_DELAY).then_some(GridEvent::Release { index })
        };

        if let Some(event) = event {
            let _ = self.producer.enqueue(event);
        }
    }

    pub fn set_switch_raw(&mut self, entry: usize, value: bool) {
        if let Some(slot) = self.switches_raw.get_mut(entry) {
            *slot = value as u8;
        }
    }

    pub fn capture_adc_bank(&mut self, bank: usize, samples: &[u16; ADC_BANK_SIZE]) {
        let base = bank * ADC_BANK_SIZE;
        self.adc_direct[base..base + ADC_BANK_SIZE].copy_from_slice(samples);
    }

    pub fn accumulate_adc_max(&mut self) {
        for sensor in 0..PAD_COUNT {
            let value = ((self.adc_direct[PADADC[sensor] as usize] + 2) >> 2) as u16;
            if self.adc_max[sensor] < value {
                self.adc_max[sensor] = value;
            }
        }
    }

    pub fn tick_1khz(&mut self) {
        for entry in 0..SWITCH_ENTRY_COUNT {
            self.capture_switch_entry(entry, self.switches_raw[entry] != 0);
        }
    }

    pub fn tick_200hz(&mut self) {
        for sensor in 0..PAD_COUNT {
            self.process_pad(sensor);
            self.adc_max[sensor] = 0;
        }
    }

    fn process_pad(&mut self, sensor: usize) {
        let sample = self.adc_max[sensor];
        let state = self.pad_state[sensor];

        match state {
            0 => {
                let velocity = velocity_from_sample(sample);
                if velocity != 0 {
                    self.pad_state[sensor] = 2;
                    self.pad_time[sensor] = PAD_HOLDOFF_TICKS;
                    self.capture_pad_velocity(sensor, velocity);
                }
            }
            2 => {
                if self.pad_time[sensor] != 0 {
                    self.pad_time[sensor] = self.pad_time[sensor].saturating_sub(1);
                }

                if sample <= PAD_PRESS_START {
                    if self.last_aftertouch[sensor] != 0 {
                        self.last_aftertouch[sensor] = 0;
                        self.capture_pad_aftertouch(sensor, 0);
                    }
                    self.pad_state[sensor] = 3;
                    self.pad_time[sensor] = PAD_RELEASE_HOLDOFF_TICKS;
                    return;
                }

                if self.pad_time[sensor] == 0 {
                    let aftertouch = aftertouch_from_sample(sample);
                    if self.last_aftertouch[sensor] != aftertouch {
                        self.last_aftertouch[sensor] = aftertouch;
                        self.capture_pad_aftertouch(sensor, aftertouch);
                    }
                }
            }
            3 => {
                if self.pad_time[sensor] != 0 {
                    self.pad_time[sensor] = self.pad_time[sensor].saturating_sub(1);
                } else {
                    self.pad_state[sensor] = 0;
                    self.last_aftertouch[sensor] = 0;
                    self.capture_pad_velocity(sensor, 0);
                }
            }
            _ => {
                self.pad_state[sensor] = 0;
            }
        }
    }
}

fn switch_is_on(state: &mut u8) -> bool {
    let current = *state;
    if current == 0 {
        *state = 2;
        return false;
    }

    if current < 0x80 {
        let next = current - 1;
        if next == 0 {
            *state = 0x80;
            return true;
        }
        *state = next;
        return false;
    }

    if current != 0x80 {
        *state = 0x80;
    }

    false
}

fn switch_is_off(state: &mut u8, release_delay: u8) -> bool {
    let current = *state;
    if current == 0 {
        return false;
    }

    if current < 0x80 {
        *state = 0;
        return false;
    }

    if current == 0x80 {
        *state = release_delay.wrapping_add(0x80);
        return false;
    }

    let next = current.wrapping_sub(1);
    if next == 0x80 {
        *state = 0;
        return true;
    }

    *state = next;
    false
}

fn velocity_from_sample(sample: u16) -> u8 {
    if sample <= PAD_PRESS_PEAK_START {
        return 0;
    }

    let span = PAD_PRESS_END.saturating_sub(PAD_PRESS_PEAK_START).max(1);
    let value = sample.saturating_sub(PAD_PRESS_PEAK_START).min(span);
    let velocity = ((value as u32 * 127) / span as u32) as u8;
    velocity.max(1)
}

fn aftertouch_from_sample(sample: u16) -> u8 {
    if sample <= PAD_AFTERTOUCH_START {
        return 0;
    }

    let value = sample
        .saturating_sub(PAD_AFTERTOUCH_START)
        .min(PAD_AFTERTOUCH_RANGE);
    ((value as u32 * 127) / PAD_AFTERTOUCH_RANGE as u32) as u8
}
