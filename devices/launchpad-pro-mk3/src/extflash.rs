// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

use core::cmp::min;

use embassy_stm32::Peri;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::mode::Blocking;
use embassy_stm32::peripherals;
use embassy_stm32::spi::mode::Master;
use embassy_stm32::spi::{Config as SpiConfig, Spi};
use embassy_stm32::time::Hertz;

const CMD_WRITE_ENABLE: u8 = 0x06;
const CMD_READ_STATUS1: u8 = 0x05;
const CMD_READ_JEDEC_ID: u8 = 0x9f;
const CMD_PAGE_PROGRAM: u8 = 0x02;
const CMD_READ_DATA: u8 = 0x03;
const CMD_SECTOR_ERASE: u8 = 0x20;
const STATUS_WIP: u8 = 0x01;
const EXPECTED_JEDEC_MANUFACTURER: u8 = 0xc2;
const EXPECTED_JEDEC_CAPACITY: u8 = 0x18;

const PAGE_SIZE: usize = 256;
const SECTOR_SIZE: usize = 4096;
const TOTAL_SIZE: u32 = 16 * 1024 * 1024;
const SETTINGS_SIZE: u32 = 8 * 1024;
const SETTINGS_OFFSET: u32 = TOTAL_SIZE - SETTINGS_SIZE;

pub struct ExtFlash<'d> {
    spi: Spi<'d, Blocking, Master>,
    cs: Output<'d>,
    jedec_id: [u8; 3],
    present: bool,
}

#[derive(Clone, Copy)]
pub struct ExtFlashInfo {
    pub present: bool,
    pub jedec_id: [u8; 3],
    pub status1: u8,
}

impl<'d> ExtFlash<'d> {
    pub fn new(
        spi1: Peri<'d, peripherals::SPI1>,
        pa5: Peri<'d, peripherals::PA5>,
        pb5: Peri<'d, peripherals::PB5>,
        pb4: Peri<'d, peripherals::PB4>,
        pa15: Peri<'d, peripherals::PA15>,
    ) -> Self {
        let mut spi_cfg = SpiConfig::default();
        spi_cfg.frequency = Hertz(10_500_000);

        let cs = Output::new(pa15, Level::High, Speed::VeryHigh);
        let spi = Spi::new_blocking(spi1, pa5, pb5, pb4, spi_cfg);
        let mut flash = Self {
            spi,
            cs,
            jedec_id: [0; 3],
            present: false,
        };
        flash.probe();
        flash
    }

    pub fn settings_size(&mut self) -> u32 {
        if !self.present {
            self.probe();
        }
        if self.present { SETTINGS_SIZE } else { 0 }
    }

    pub fn info(&mut self) -> ExtFlashInfo {
        self.probe();
        ExtFlashInfo {
            present: self.present,
            jedec_id: self.jedec_id,
            status1: self.read_status1(),
        }
    }

    pub fn read_settings(&mut self, offset: u32, data: &mut [u8]) {
        if data.is_empty() {
            return;
        }

        if !self.present {
            self.probe();
        }

        if !self.present || offset >= SETTINGS_SIZE {
            data.fill(0xff);
            return;
        }

        let readable = min((SETTINGS_SIZE - offset) as usize, data.len());
        self.read(SETTINGS_OFFSET + offset, &mut data[..readable]);

        if readable < data.len() {
            data[readable..].fill(0xff);
        }
    }

    pub fn write_settings(&mut self, offset: u32, data: &[u8]) {
        if data.is_empty() || offset >= SETTINGS_SIZE {
            return;
        }

        if !self.present {
            self.probe();
        }
        if !self.present {
            return;
        }

        let mut rel_off = offset;
        let mut src = data;
        let mut writable = min((SETTINGS_SIZE - offset) as usize, src.len());
        let mut sector_buf = [0xff; SECTOR_SIZE];

        while writable != 0 {
            let abs_off = SETTINGS_OFFSET + rel_off;
            let sector_base = abs_off & !((SECTOR_SIZE as u32) - 1);
            let in_sector = (abs_off - sector_base) as usize;
            let chunk = min(writable, SECTOR_SIZE - in_sector);

            self.read(sector_base, &mut sector_buf);

            if sector_buf[in_sector..in_sector + chunk] != src[..chunk] {
                sector_buf[in_sector..in_sector + chunk].copy_from_slice(&src[..chunk]);
                self.erase_sector(sector_base);

                for page_off in (0..SECTOR_SIZE).step_by(PAGE_SIZE) {
                    if !sector_buf[page_off..page_off + PAGE_SIZE]
                        .iter()
                        .all(|byte| *byte == 0xff)
                    {
                        let page = sector_buf[page_off..page_off + PAGE_SIZE]
                            .try_into()
                            .unwrap();
                        self.write_page(sector_base + page_off as u32, page);
                    }
                }
            }

            rel_off += chunk as u32;
            src = &src[chunk..];
            writable -= chunk;
        }
    }

    fn select(&mut self) {
        self.cs.set_low();
    }

    fn deselect(&mut self) {
        self.cs.set_high();
    }

    fn xfer(&mut self, byte: u8) -> u8 {
        let mut data = [byte];
        self.spi.blocking_transfer_in_place(&mut data).unwrap();
        data[0]
    }

    fn wait_busy(&mut self) {
        for _ in 0..100_000 {
            self.select();
            self.xfer(CMD_READ_STATUS1);
            let status = self.xfer(0xff);
            self.deselect();

            if status & STATUS_WIP == 0 {
                return;
            }
        }
    }

    fn write_enable(&mut self) {
        self.select();
        self.xfer(CMD_WRITE_ENABLE);
        self.deselect();
    }

    fn probe(&mut self) {
        self.jedec_id = self.read_jedec_id();
        self.present = self.jedec_id[0] == EXPECTED_JEDEC_MANUFACTURER
            && self.jedec_id[2] == EXPECTED_JEDEC_CAPACITY;
    }

    fn read_jedec_id(&mut self) -> [u8; 3] {
        self.select();
        self.xfer(CMD_READ_JEDEC_ID);
        let id = [self.xfer(0), self.xfer(0), self.xfer(0)];
        self.deselect();
        id
    }

    fn read_status1(&mut self) -> u8 {
        self.select();
        self.xfer(CMD_READ_STATUS1);
        let status = self.xfer(0);
        self.deselect();
        status
    }

    fn erase_sector(&mut self, offset: u32) {
        self.wait_busy();
        self.write_enable();

        self.select();
        self.xfer(CMD_SECTOR_ERASE);
        self.write_addr(offset);
        self.deselect();

        self.wait_busy();
    }

    fn write_page(&mut self, offset: u32, data: &[u8; PAGE_SIZE]) {
        self.wait_busy();
        self.write_enable();

        self.select();
        self.xfer(CMD_PAGE_PROGRAM);
        self.write_addr(offset);
        for byte in data {
            self.xfer(*byte);
        }
        self.deselect();

        self.wait_busy();
    }

    fn read(&mut self, offset: u32, data: &mut [u8]) {
        self.wait_busy();

        self.select();
        self.xfer(CMD_READ_DATA);
        self.write_addr(offset);
        for byte in data {
            *byte = self.xfer(0xff);
        }
        self.deselect();
    }

    fn write_addr(&mut self, offset: u32) {
        self.xfer((offset >> 16) as u8);
        self.xfer((offset >> 8) as u8);
        self.xfer(offset as u8);
    }
}
