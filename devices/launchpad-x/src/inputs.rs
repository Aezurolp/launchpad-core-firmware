use core::ptr;

const LP_LED_COUNT: usize = 100;
const LP_GRID_SENSOR_COUNT: usize = 64;
const LP_SIMPLE_BTN_COUNT: usize = 16;
const LP_ADC_CHANNELS: usize = 8;
const LP_GRID_QUEUE_LEN: usize = 64;

const LP_PRESS_START_NORM: u16 = 0x012c;
const LP_PRESS_RELEASE_NORM: u16 = 0x0090;
const LP_AFTER_START_NORM: u16 = 0x0259;
const LP_AFTER_DELTA_THR: u8 = 3;
const LP_PRESS_ON_COUNT: u8 = 1;
const LP_RELEASE_COUNT: u8 = 8;
const LP_RELEASE_HOLDOFF: u8 = 10;
const LP_AFTER_COOLDOWN: u8 = 2;
const LP_BASELINE_GUARD: u16 = 0x0060;
const LP_AFTER_FLOOR_16: u32 = 0x2222;
const LP_VELOCITY_GAIN_NUM: u32 = 140;

const PAD_SCAN_MAP: [u8; LP_LED_COUNT] = [
    0xff, 0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0xff, 0xff, 0x00, 0x08, 0x10, 0x18, 0x01,
    0x09, 0x11, 0x19, 0x88, 0xff, 0x20, 0x28, 0x30, 0x38, 0x21, 0x29, 0x31, 0x39, 0x89, 0xff, 0x02,
    0x0a, 0x12, 0x1a, 0x03, 0x0b, 0x13, 0x1b, 0x8a, 0xff, 0x22, 0x2a, 0x32, 0x3a, 0x23, 0x2b, 0x33,
    0x3b, 0x8b, 0xff, 0x04, 0x0c, 0x14, 0x1c, 0x05, 0x0d, 0x15, 0x1d, 0x8c, 0xff, 0x24, 0x2c, 0x34,
    0x3c, 0x25, 0x2d, 0x35, 0x3d, 0x8d, 0xff, 0x06, 0x0e, 0x16, 0x1e, 0x07, 0x0f, 0x17, 0x1f, 0x8e,
    0xff, 0x26, 0x2e, 0x36, 0x3e, 0x27, 0x2f, 0x37, 0x3f, 0x8f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xff,
];

const RCC_AHB1ENR: *mut u32 = 0x4002_3830 as *mut u32;
const RCC_APB2ENR: *mut u32 = 0x4002_3844 as *mut u32;
const GPIOA_MODER: *mut u32 = 0x4002_0000 as *mut u32;
const GPIOA_PUPDR: *mut u32 = 0x4002_000c as *mut u32;
const DMA2_LISR: *mut u32 = 0x4002_6400 as *mut u32;
const DMA2_LIFCR: *mut u32 = 0x4002_6408 as *mut u32;
const DMA2_STREAM0_CR: *mut u32 = 0x4002_6410 as *mut u32;
const DMA2_STREAM0_NDTR: *mut u32 = 0x4002_6414 as *mut u32;
const DMA2_STREAM0_PAR: *mut u32 = 0x4002_6418 as *mut u32;
const DMA2_STREAM0_M0AR: *mut u32 = 0x4002_641c as *mut u32;
const DMA2_STREAM0_FCR: *mut u32 = 0x4002_6424 as *mut u32;
const ADC_CCR: *mut u32 = 0x4001_2304 as *mut u32;
const ADC1_SR: *mut u32 = 0x4001_2000 as *mut u32;
const ADC1_CR1: *mut u32 = 0x4001_2004 as *mut u32;
const ADC1_CR2: *mut u32 = 0x4001_2008 as *mut u32;
const ADC1_SMPR2: *mut u32 = 0x4001_2010 as *mut u32;
const ADC1_SQR1: *mut u32 = 0x4001_202c as *mut u32;
const ADC1_SQR2: *mut u32 = 0x4001_2030 as *mut u32;
const ADC1_SQR3: *mut u32 = 0x4001_2034 as *mut u32;
const ADC1_DR: *mut u32 = 0x4001_204c as *mut u32;

const RCC_AHB1ENR_GPIOAEN: u32 = 1 << 0;
const RCC_AHB1ENR_DMA2EN: u32 = 1 << 22;
const RCC_APB2ENR_ADC1EN: u32 = 1 << 8;
const DMA_SXCR_EN: u32 = 1 << 0;
const DMA_SXCR_MINC: u32 = 1 << 10;
const DMA_SXCR_PSIZE_0: u32 = 1 << 11;
const DMA_SXCR_MSIZE_0: u32 = 1 << 13;
const DMA_SXCR_PL_1: u32 = 1 << 17;
const DMA_LISR_FEIF0: u32 = 1 << 0;
const DMA_LISR_DMEIF0: u32 = 1 << 2;
const DMA_LISR_TEIF0: u32 = 1 << 3;
const DMA_LISR_TCIF0: u32 = 1 << 5;
const DMA_LIFCR_CFEIF0: u32 = 1 << 0;
const DMA_LIFCR_CDMEIF0: u32 = 1 << 2;
const DMA_LIFCR_CTEIF0: u32 = 1 << 3;
const DMA_LIFCR_CHTIF0: u32 = 1 << 4;
const DMA_LIFCR_CTCIF0: u32 = 1 << 5;
const ADC_CCR_ADCPRE_0: u32 = 1 << 16;
const ADC_CCR_ADCPRE: u32 = 0b11 << 16;
const ADC_CR1_SCAN: u32 = 1 << 8;
const ADC_CR2_ADON: u32 = 1 << 0;
const ADC_CR2_CONT: u32 = 1 << 1;
const ADC_CR2_DMA: u32 = 1 << 8;
const ADC_CR2_DDS: u32 = 1 << 9;
const ADC_CR2_EOCS: u32 = 1 << 10;
const ADC_CR2_SWSTART: u32 = 1 << 30;

const ADC_DMA_CLEAR_FLAGS: u32 =
    DMA_LIFCR_CFEIF0 | DMA_LIFCR_CDMEIF0 | DMA_LIFCR_CTEIF0 | DMA_LIFCR_CHTIF0 | DMA_LIFCR_CTCIF0;
const ADC_DMA_ERROR_FLAGS: u32 = DMA_LISR_FEIF0 | DMA_LISR_DMEIF0 | DMA_LISR_TEIF0;
const ADC_DMA_CR_BASE: u32 = DMA_SXCR_PL_1 | DMA_SXCR_MSIZE_0 | DMA_SXCR_PSIZE_0 | DMA_SXCR_MINC;

#[derive(Clone, Copy)]
pub enum GridEvent {
    Press { note: u8, value: u8 },
    Release { note: u8 },
    Aftertouch { note: u8, value: u8 },
}

pub struct Inputs {
    side_hist: [u16; 4],
    side_stable: u16,
    adc_dma: [u16; LP_ADC_CHANNELS],
    pressure_capture_bank: u8,
    pressure_capture_active: bool,
    pressure_raw: [[u16; LP_ADC_CHANNELS]; 8],
    pressure_dirty_mask: u8,
    grid_base: [u16; LP_GRID_SENSOR_COUNT],
    grid_filt: [u16; LP_GRID_SENSOR_COUNT],
    grid_hist: [[u16; 3]; LP_GRID_SENSOR_COUNT],
    grid_hist_pos: [u8; LP_GRID_SENSOR_COUNT],
    grid_hist_count: [u8; LP_GRID_SENSOR_COUNT],
    grid_ready: [bool; LP_GRID_SENSOR_COUNT],
    grid_pressed: [bool; LP_GRID_SENSOR_COUNT],
    grid_last_at: [u8; LP_GRID_SENSOR_COUNT],
    grid_on_count: [u8; LP_GRID_SENSOR_COUNT],
    grid_off_count: [u8; LP_GRID_SENSOR_COUNT],
    grid_release_holdoff: [u8; LP_GRID_SENSOR_COUNT],
    grid_after_cooldown: [u8; LP_GRID_SENSOR_COUNT],
    sensor_to_note: [u8; LP_GRID_SENSOR_COUNT],
    simple_to_note: [u8; LP_SIMPLE_BTN_COUNT],
    queue: [Option<GridEvent>; LP_GRID_QUEUE_LEN],
    q_head: u8,
    q_tail: u8,
}

impl Inputs {
    pub fn new() -> Self {
        let mut this = Self {
            side_hist: [0; 4],
            side_stable: 0,
            adc_dma: [0; LP_ADC_CHANNELS],
            pressure_capture_bank: 0xff,
            pressure_capture_active: false,
            pressure_raw: [[0; LP_ADC_CHANNELS]; 8],
            pressure_dirty_mask: 0,
            grid_base: [0; LP_GRID_SENSOR_COUNT],
            grid_filt: [0; LP_GRID_SENSOR_COUNT],
            grid_hist: [[0; 3]; LP_GRID_SENSOR_COUNT],
            grid_hist_pos: [0; LP_GRID_SENSOR_COUNT],
            grid_hist_count: [0; LP_GRID_SENSOR_COUNT],
            grid_ready: [false; LP_GRID_SENSOR_COUNT],
            grid_pressed: [false; LP_GRID_SENSOR_COUNT],
            grid_last_at: [0; LP_GRID_SENSOR_COUNT],
            grid_on_count: [0; LP_GRID_SENSOR_COUNT],
            grid_off_count: [0; LP_GRID_SENSOR_COUNT],
            grid_release_holdoff: [0; LP_GRID_SENSOR_COUNT],
            grid_after_cooldown: [0; LP_GRID_SENSOR_COUNT],
            sensor_to_note: [0xff; LP_GRID_SENSOR_COUNT],
            simple_to_note: [0xff; LP_SIMPLE_BTN_COUNT],
            queue: [None; LP_GRID_QUEUE_LEN],
            q_head: 0,
            q_tail: 0,
        };

        for (index, &map) in PAD_SCAN_MAP.iter().enumerate() {
            let note = idx_to_yx(index as u8);
            if (map as usize) < LP_GRID_SENSOR_COUNT {
                this.sensor_to_note[map as usize] = note;
            } else if (map & 0xf0) == 0x80 {
                this.simple_to_note[(map & 0x0f) as usize] = note;
            }
        }

        this
    }

    pub fn init_hardware(&mut self) {
        unsafe {
            modify_reg(RCC_AHB1ENR, |value| {
                value | RCC_AHB1ENR_GPIOAEN | RCC_AHB1ENR_DMA2EN
            });
            modify_reg(RCC_APB2ENR, |value| value | RCC_APB2ENR_ADC1EN);

            modify_reg(GPIOA_MODER, |value| value | 0x0000_ffff);
            modify_reg(GPIOA_PUPDR, |value| value & !0x0000_ffff);

            modify_reg(DMA2_STREAM0_CR, |value| value & !DMA_SXCR_EN);
            while read_reg(DMA2_STREAM0_CR) & DMA_SXCR_EN != 0 {}

            write_reg(DMA2_LIFCR, ADC_DMA_CLEAR_FLAGS);
            write_reg(DMA2_STREAM0_PAR, ADC1_DR as u32);
            write_reg(DMA2_STREAM0_M0AR, self.adc_dma.as_mut_ptr() as u32);
            write_reg(DMA2_STREAM0_NDTR, LP_ADC_CHANNELS as u32);
            write_reg(DMA2_STREAM0_FCR, 0);
            write_reg(DMA2_STREAM0_CR, ADC_DMA_CR_BASE);

            modify_reg(ADC_CCR, |value| {
                (value & !ADC_CCR_ADCPRE) | ADC_CCR_ADCPRE_0
            });
            write_reg(ADC1_CR1, ADC_CR1_SCAN);
            write_reg(ADC1_CR2, ADC_CR2_DMA | ADC_CR2_DDS | ADC_CR2_EOCS);
            write_reg(ADC1_SMPR2, 0);
            write_reg(ADC1_SQR1, 7 << 20);
            write_reg(ADC1_SQR2, (6 << 0) | (7 << 5));
            write_reg(
                ADC1_SQR3,
                (0 << 0) | (1 << 5) | (2 << 10) | (3 << 15) | (4 << 20) | (5 << 25),
            );
            modify_reg(ADC1_CR2, |value| value | ADC_CR2_ADON);
        }
    }

    pub fn capture_side(&mut self, group: u8, row: u8, sample: u16) {
        if group >= 4 || row >= 4 {
            return;
        }

        let mut value = self.side_hist[group as usize];
        for col in 0..4 {
            let pressed = (sample & (1 << (col * 4))) != 0;
            let bit = col * 4 + row as usize;
            let mask = 1u16 << bit;
            if pressed {
                value |= mask;
            } else {
                value &= !mask;
            }
        }
        self.side_hist[group as usize] = value;
    }

    pub fn start_pressure_capture(&mut self, bank: u8) {
        if bank >= 8 {
            return;
        }

        unsafe {
            if self.pressure_capture_active {
                modify_reg(DMA2_STREAM0_CR, |value| value & !DMA_SXCR_EN);
                while read_reg(DMA2_STREAM0_CR) & DMA_SXCR_EN != 0 {}
            }

            write_reg(DMA2_LIFCR, ADC_DMA_CLEAR_FLAGS);
            write_reg(DMA2_STREAM0_M0AR, self.adc_dma.as_mut_ptr() as u32);
            write_reg(DMA2_STREAM0_NDTR, LP_ADC_CHANNELS as u32);
            write_reg(DMA2_STREAM0_CR, ADC_DMA_CR_BASE);
            write_reg(ADC1_SR, 0);
            modify_reg(ADC1_CR2, |value| {
                (value & !(ADC_CR2_CONT | ADC_CR2_SWSTART))
                    | ADC_CR2_DMA
                    | ADC_CR2_DDS
                    | ADC_CR2_EOCS
            });

            self.pressure_capture_bank = bank;
            self.pressure_capture_active = true;

            modify_reg(DMA2_STREAM0_CR, |value| value | DMA_SXCR_EN);
            modify_reg(ADC1_CR2, |value| value | ADC_CR2_SWSTART);
        }
    }

    pub fn finish_pressure_capture(&mut self, bank: u8) -> bool {
        if bank >= 8 || !self.pressure_capture_active || self.pressure_capture_bank != bank {
            return true;
        }

        unsafe {
            let lisr = read_reg(DMA2_LISR);
            if lisr & DMA_LISR_TCIF0 == 0 {
                if lisr & ADC_DMA_ERROR_FLAGS != 0 {
                    modify_reg(DMA2_STREAM0_CR, |value| value & !DMA_SXCR_EN);
                    while read_reg(DMA2_STREAM0_CR) & DMA_SXCR_EN != 0 {}
                    write_reg(DMA2_LIFCR, ADC_DMA_CLEAR_FLAGS);
                    self.pressure_capture_bank = 0xff;
                    self.pressure_capture_active = false;
                    return true;
                }
                return false;
            }

            modify_reg(DMA2_STREAM0_CR, |value| value & !DMA_SXCR_EN);
            while read_reg(DMA2_STREAM0_CR) & DMA_SXCR_EN != 0 {}
            write_reg(DMA2_LIFCR, ADC_DMA_CLEAR_FLAGS);
        }

        for ch in 0..LP_ADC_CHANNELS {
            self.pressure_raw[bank as usize][ch] = self.adc_dma[ch] & 0x0fff;
        }

        self.pressure_capture_bank = 0xff;
        self.pressure_capture_active = false;
        self.pressure_dirty_mask |= 1 << bank;
        true
    }

    pub fn pop_event(&mut self) -> Option<GridEvent> {
        self.service_pressure_foreground();
        self.service_side_buttons_foreground();

        if self.q_tail == self.q_head {
            return None;
        }

        let event = self.queue[self.q_tail as usize].take();
        self.q_tail = (self.q_tail + 1) % LP_GRID_QUEUE_LEN as u8;
        event
    }

    fn service_pressure_foreground(&mut self) {
        while self.pressure_dirty_mask != 0 {
            let bank = self.pressure_dirty_mask.trailing_zeros() as usize;
            self.pressure_dirty_mask &= !(1 << bank);

            let base_sensor = bank * LP_ADC_CHANNELS;
            for ch in 0..LP_ADC_CHANNELS {
                self.update_grid_sensor(base_sensor + ch, self.pressure_raw[bank][ch]);
            }
        }
    }

    fn service_side_buttons_foreground(&mut self) {
        let mut new_stable = 0u16;

        for bit in 0..LP_SIMPLE_BTN_COUNT {
            let mask = 1u16 << bit;
            let mut sum = 0;
            for sample in self.side_hist {
                if sample & mask != 0 {
                    sum += 1;
                }
            }

            if sum > 1 {
                new_stable |= mask;
            }
        }

        let changed = new_stable ^ self.side_stable;
        if changed == 0 {
            return;
        }

        for bit in 0..LP_SIMPLE_BTN_COUNT {
            let mask = 1u16 << bit;
            if changed & mask == 0 {
                continue;
            }

            let note = self.simple_to_note[bit];
            if note == 0xff {
                continue;
            }

            if new_stable & mask != 0 {
                self.queue_event(GridEvent::Press { note, value: 127 });
            } else {
                self.queue_event(GridEvent::Release { note });
            }
        }

        self.side_stable = new_stable;
    }

    fn update_grid_sensor(&mut self, sensor: usize, raw: u16) {
        if sensor >= LP_GRID_SENSOR_COUNT {
            return;
        }

        let pos = self.grid_hist_pos[sensor] as usize;
        self.grid_hist[sensor][pos] = raw;
        self.grid_hist_pos[sensor] = ((pos + 1) % 3) as u8;
        self.grid_hist_count[sensor] = self.grid_hist_count[sensor].saturating_add(1).min(3);

        let mut sample = raw;
        if self.grid_hist_count[sensor] >= 3 {
            sample = median3(
                self.grid_hist[sensor][0],
                self.grid_hist[sensor][1],
                self.grid_hist[sensor][2],
            );
        }

        let norm = normalize_raw_to_norm(sample);
        let mut filt = self.grid_filt[sensor];
        if filt == 0 {
            filt = norm;
        } else if norm >= filt {
            filt = ((filt as u32 + norm as u32 + 1) / 2) as u16;
        } else {
            filt = (((filt as u32) * 7 + norm as u32 + 4) / 8) as u16;
        }
        self.grid_filt[sensor] = filt;

        if !self.grid_ready[sensor] {
            self.grid_base[sensor] = filt;
            self.grid_ready[sensor] = true;
            return;
        }

        let mut base = self.grid_base[sensor];
        if filt > base {
            if filt - base < LP_BASELINE_GUARD && !self.grid_pressed[sensor] {
                base = (((base as u32) * 31 + filt as u32 + 16) / 32) as u16;
            }
        } else if !self.grid_pressed[sensor] {
            base = (((base as u32) * 31 + filt as u32 + 16) / 32) as u16;
        }
        self.grid_base[sensor] = base;

        let delta = filt.saturating_sub(base);
        let velocity = norm_to_velocity(norm);
        let pressure = norm_to_aftertouch(norm);

        if !self.grid_pressed[sensor] {
            if delta >= LP_PRESS_START_NORM {
                self.grid_on_count[sensor] = self.grid_on_count[sensor].saturating_add(1);
                if self.grid_on_count[sensor] >= LP_PRESS_ON_COUNT {
                    self.grid_pressed[sensor] = true;
                    self.grid_on_count[sensor] = 0;
                    self.grid_off_count[sensor] = 0;
                    self.grid_release_holdoff[sensor] = LP_RELEASE_HOLDOFF;
                    self.grid_after_cooldown[sensor] = 0;
                    self.grid_last_at[sensor] = pressure;
                    self.queue_grid_event(GridEventKind::Press, sensor, velocity);
                }
            } else {
                self.grid_on_count[sensor] = 0;
            }
            return;
        }

        self.grid_release_holdoff[sensor] = self.grid_release_holdoff[sensor].saturating_sub(1);

        if self.grid_after_cooldown[sensor] != 0 {
            self.grid_after_cooldown[sensor] -= 1;
        } else {
            let prev = self.grid_last_at[sensor];
            let diff = pressure.abs_diff(prev);
            if diff >= LP_AFTER_DELTA_THR {
                self.grid_last_at[sensor] = pressure;
                self.grid_after_cooldown[sensor] = LP_AFTER_COOLDOWN;
                self.queue_grid_event(GridEventKind::Aftertouch, sensor, pressure);
            }
        }

        if delta <= LP_PRESS_RELEASE_NORM {
            self.grid_off_count[sensor] = self.grid_off_count[sensor].saturating_add(1);
            if self.grid_off_count[sensor] >= LP_RELEASE_COUNT
                && self.grid_release_holdoff[sensor] == 0
            {
                self.grid_pressed[sensor] = false;
                self.grid_off_count[sensor] = 0;
                self.grid_release_holdoff[sensor] = 0;
                self.grid_after_cooldown[sensor] = 0;
                self.grid_last_at[sensor] = 0;
                self.grid_base[sensor] = filt;
                self.queue_grid_event(GridEventKind::Release, sensor, 0);
            }
            return;
        }

        self.grid_off_count[sensor] = 0;
    }

    fn queue_grid_event(&mut self, kind: GridEventKind, sensor: usize, value: u8) {
        if sensor >= LP_GRID_SENSOR_COUNT {
            return;
        }

        let note = self.sensor_to_note[sensor];
        if note == 0xff {
            return;
        }

        match kind {
            GridEventKind::Press => self.queue_event(GridEvent::Press { note, value }),
            GridEventKind::Release => self.queue_event(GridEvent::Release { note }),
            GridEventKind::Aftertouch => self.queue_event(GridEvent::Aftertouch { note, value }),
        }
    }

    fn queue_event(&mut self, event: GridEvent) {
        let next = (self.q_head + 1) % LP_GRID_QUEUE_LEN as u8;
        if next == self.q_tail {
            return;
        }

        self.queue[self.q_head as usize] = Some(event);
        self.q_head = next;
    }
}

enum GridEventKind {
    Press,
    Release,
    Aftertouch,
}

fn idx_to_yx(index: u8) -> u8 {
    (9 - (index / 10)) * 10 + (index % 10)
}

fn normalize_raw_to_norm(raw: u16) -> u16 {
    if raw < 0x0097 {
        return 0;
    }

    if raw >= 0x0dac {
        return 0x0fff;
    }

    ((((raw - 0x0096) as u32) << 12) / 0x0d16) as u16
}

fn norm_to_velocity(norm: u16) -> u8 {
    if norm <= LP_PRESS_START_NORM {
        return 1;
    }

    let mut value = (((norm - LP_PRESS_START_NORM) as u32) * LP_VELOCITY_GAIN_NUM) / 0x0ed4;
    if value == 0 {
        value = 1;
    }
    value.min(127) as u8
}

fn norm_to_aftertouch(norm: u16) -> u8 {
    if norm <= LP_AFTER_START_NORM {
        return 0;
    }

    let scaled16 = if norm >= 0x0fff {
        0xffff
    } else {
        (((norm - 0x0258) as u32) << 16) / 0x0da7
    };

    let stretched16 = ((0x9000 * scaled16) >> 15).min(0xffff);
    let out16 = if stretched16 > LP_AFTER_FLOOR_16 {
        ((stretched16 - LP_AFTER_FLOOR_16) << 16) / (0x10000 - LP_AFTER_FLOOR_16)
    } else {
        0
    };

    (out16 >> 9).min(127) as u8
}

fn median3(mut a: u16, mut b: u16, mut c: u16) -> u16 {
    if a > b {
        core::mem::swap(&mut a, &mut b);
    }
    if b > c {
        core::mem::swap(&mut b, &mut c);
    }
    if a > b {
        core::mem::swap(&mut a, &mut b);
    }
    b
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
