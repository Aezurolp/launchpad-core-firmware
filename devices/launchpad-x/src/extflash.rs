// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister
// Copyright (C) 2026 ZephyrCodesStuff

use core::cmp::min;

use embassy_stm32::Peri;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::mode::Blocking;
use embassy_stm32::peripherals;
use embassy_stm32::spi::mode::Master;
use embassy_stm32::spi::{Config as SpiConfig, Spi};
use embassy_stm32::time::Hertz;

use embedded_storage::nor_flash::{ErrorType, NorFlash, NorFlashErrorKind, ReadNorFlash};
use spi_memory::series25::Flash;
use spi_memory::{BlockDevice, Read};

const PAGE_SIZE: usize = 256;
const SECTOR_SIZE: usize = 4096;
const TOTAL_SIZE: u32 = 1024 * 1024;
const SETTINGS_SIZE: u32 = 8 * 1024;
const SETTINGS_OFFSET: u32 = TOTAL_SIZE - SETTINGS_SIZE;

pub struct ExtFlash<'d> {
    flash: Option<Flash<Spi<'d, Blocking, Master>, Output<'d>>>,
}

impl<'d> ExtFlash<'d> {
    pub fn new(
        spi2: Peri<'d, peripherals::SPI2>,
        pb13: Peri<'d, peripherals::PB13>,
        pb15: Peri<'d, peripherals::PB15>,
        pb14: Peri<'d, peripherals::PB14>,
        pb12: Peri<'d, peripherals::PB12>,
    ) -> Self {
        let mut spi_cfg = SpiConfig::default();
        spi_cfg.frequency = Hertz(10_500_000);

        let spi = Spi::new_blocking(spi2, pb13, pb15, pb14, spi_cfg);
        let cs = Output::new(pb12, Level::High, Speed::VeryHigh);

        Self {
            flash: Flash::init(spi, cs).ok(),
        }
    }

    pub const fn settings_size(&self) -> u32 {
        SETTINGS_SIZE
    }

    pub fn read_settings(&mut self, offset: u32, data: &mut [u8]) {
        if data.is_empty() {
            return;
        }

        if offset >= SETTINGS_SIZE {
            data.fill(0xff);
            return;
        }

        let readable = min((SETTINGS_SIZE - offset) as usize, data.len());
        if let Some(flash) = &mut self.flash {
            let _ = flash.read(SETTINGS_OFFSET + offset, &mut data[..readable]);
        } else {
            data[..readable].fill(0xff);
        }

        if readable < data.len() {
            data[readable..].fill(0xff);
        }
    }

    pub fn write_settings(&mut self, offset: u32, data: &[u8]) {
        let flash = match &mut self.flash {
            Some(f) => f,
            None => return,
        };

        if data.is_empty() || offset >= SETTINGS_SIZE {
            return;
        }

        let mut rel_off = offset;
        let mut src = data;
        let mut writable = min((SETTINGS_SIZE - offset) as usize, src.len());
        static mut SECTOR_BUF: [u8; SECTOR_SIZE] = [0xff; SECTOR_SIZE];
        let mut sector_buf = unsafe { &mut SECTOR_BUF[..] };

        while writable != 0 {
            let abs_off = SETTINGS_OFFSET + rel_off;
            let sector_base = abs_off & !((SECTOR_SIZE as u32) - 1);
            let in_sector = (abs_off - sector_base) as usize;
            let chunk = min(writable, SECTOR_SIZE - in_sector);

            let _ = flash.read(sector_base, &mut sector_buf);

            if sector_buf[in_sector..in_sector + chunk] != src[..chunk] {
                sector_buf[in_sector..in_sector + chunk].copy_from_slice(&src[..chunk]);
                let _ = flash.erase_sectors(sector_base, 1);

                for page_off in (0..SECTOR_SIZE).step_by(PAGE_SIZE) {
                    if !sector_buf[page_off..page_off + PAGE_SIZE]
                        .iter()
                        .all(|byte| *byte == 0xff)
                    {
                        let _ = flash.write_bytes(
                            sector_base + page_off as u32,
                            &mut sector_buf[page_off..page_off + PAGE_SIZE],
                        );
                    }
                }
            }

            rel_off += chunk as u32;
            src = &src[chunk..];
            writable -= chunk;
        }
    }
}

impl ErrorType for ExtFlash<'_> {
    type Error = NorFlashErrorKind;
}

impl ReadNorFlash for ExtFlash<'_> {
    const READ_SIZE: usize = 1;

    fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        self.read_settings(offset, bytes);
        Ok(())
    }

    fn capacity(&self) -> usize {
        SETTINGS_SIZE as usize
    }
}

impl NorFlash for ExtFlash<'_> {
    const WRITE_SIZE: usize = 1;
    const ERASE_SIZE: usize = SECTOR_SIZE;

    fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        self.write_settings(offset, bytes);
        Ok(())
    }

    fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
        if from % (SECTOR_SIZE as u32) != 0 || to % (SECTOR_SIZE as u32) != 0 || from >= to {
            return Err(NorFlashErrorKind::NotAligned);
        }
        let mut sector_addr = from;
        while sector_addr < to {
            if let Some(flash) = &mut self.flash {
                let _ = flash.erase_sectors(SETTINGS_OFFSET + sector_addr, 1);
            }
            sector_addr += SECTOR_SIZE as u32;
        }
        Ok(())
    }
}
