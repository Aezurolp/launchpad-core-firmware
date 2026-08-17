// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

use core::cmp::min;
use core::convert::Infallible;

use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::mode::Blocking;
use embassy_stm32::peripherals;
use embassy_stm32::spi::mode::Master;
use embassy_stm32::spi::{Config as SpiConfig, Spi};
use embassy_stm32::time::Hertz;
use embassy_stm32::Peri;
use embedded_hal::digital::v2::OutputPin;
use embedded_storage::nor_flash::{
    check_erase, check_read, check_write, ErrorType, NorFlash, NorFlashErrorKind, ReadNorFlash,
};
use spi_memory::series25::Flash;
use spi_memory::{BlockDevice, Read};

const PAGE_SIZE: usize = 256;
const SECTOR_SIZE: usize = 4096;
const TOTAL_SIZE: u32 = 16 * 1024 * 1024;
const SETTINGS_SIZE: u32 = 8 * 1024;
const SETTINGS_OFFSET: u32 = TOTAL_SIZE - SETTINGS_SIZE;
const EXPECTED_JEDEC_MANUFACTURER: u8 = 0xc2;
const EXPECTED_JEDEC_CAPACITY: u8 = 0x18;
// The MCU runs at 216 MHz. Keep CS high for about 1 us between commands;
// this comfortably exceeds the flash chip's minimum CS deselect time.
const CS_DESELECT_DELAY_CYCLES: u32 = 216;

struct FlashCs<'d>(Output<'d>);

impl OutputPin for FlashCs<'_> {
    type Error = Infallible;

    fn set_low(&mut self) -> Result<(), Self::Error> {
        self.0.set_low();
        Ok(())
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        self.0.set_high();
        cortex_m::asm::delay(CS_DESELECT_DELAY_CYCLES);
        Ok(())
    }
}

pub struct ExtFlash<'d> {
    flash: Option<Flash<Spi<'d, Blocking, Master>, FlashCs<'d>>>,
    sector_buf: [u8; SECTOR_SIZE],
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

        let spi = Spi::new_blocking(spi1, pa5, pb5, pb4, spi_cfg);
        let cs = FlashCs(Output::new(pa15, Level::High, Speed::VeryHigh));
        let mut this = Self {
            flash: Flash::init(spi, cs).ok(),
            sector_buf: [0xff; SECTOR_SIZE],
            jedec_id: [0; 3],
            present: false,
        };
        this.probe();
        this
    }

    pub fn info(&mut self) -> ExtFlashInfo {
        self.probe();
        ExtFlashInfo {
            present: self.present,
            jedec_id: self.jedec_id,
            status1: self.status1(),
        }
    }

    fn probe(&mut self) {
        let Some(flash) = self.flash.as_mut() else {
            self.jedec_id = [0; 3];
            self.present = false;
            return;
        };

        let Ok(id) = flash.read_jedec_id() else {
            self.jedec_id = [0; 3];
            self.present = false;
            return;
        };
        let device_id = id.device_id();
        self.jedec_id = [id.mfr_code(), device_id[0], device_id[1]];
        self.present = self.jedec_id[0] == EXPECTED_JEDEC_MANUFACTURER
            && self.jedec_id[2] == EXPECTED_JEDEC_CAPACITY;
    }

    fn status1(&mut self) -> u8 {
        self.flash
            .as_mut()
            .and_then(|flash| flash.read_status().ok())
            .map(|status| status.bits())
            .unwrap_or(0xff)
    }
}

impl ErrorType for ExtFlash<'_> {
    type Error = NorFlashErrorKind;
}

impl ReadNorFlash for ExtFlash<'_> {
    const READ_SIZE: usize = 1;

    fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        check_read(self, offset, bytes.len())?;
        let flash = self.flash.as_mut().ok_or(NorFlashErrorKind::Other)?;
        flash
            .read(SETTINGS_OFFSET + offset, bytes)
            .map_err(|_| NorFlashErrorKind::Other)
    }

    fn capacity(&self) -> usize {
        if self.present {
            SETTINGS_SIZE as usize
        } else {
            0
        }
    }
}

impl NorFlash for ExtFlash<'_> {
    const WRITE_SIZE: usize = 1;
    const ERASE_SIZE: usize = SECTOR_SIZE;

    fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        check_write(self, offset, bytes.len())?;

        let mut rel_off = offset;
        let mut src = bytes;
        let mut writable = min((SETTINGS_SIZE - offset) as usize, src.len());
        let (flash, sector_buf) = (&mut self.flash, &mut self.sector_buf);
        let flash = flash.as_mut().ok_or(NorFlashErrorKind::Other)?;

        while writable != 0 {
            let abs_off = SETTINGS_OFFSET + rel_off;
            let sector_base = abs_off & !((SECTOR_SIZE as u32) - 1);
            let in_sector = (abs_off - sector_base) as usize;
            let chunk = min(writable, SECTOR_SIZE - in_sector);

            flash
                .read(sector_base, sector_buf)
                .map_err(|_| NorFlashErrorKind::Other)?;

            if sector_buf[in_sector..in_sector + chunk] != src[..chunk] {
                sector_buf[in_sector..in_sector + chunk].copy_from_slice(&src[..chunk]);
                flash
                    .erase_sectors(sector_base, 1)
                    .map_err(|_| NorFlashErrorKind::Other)?;

                for page_off in (0..SECTOR_SIZE).step_by(PAGE_SIZE) {
                    if !sector_buf[page_off..page_off + PAGE_SIZE]
                        .iter()
                        .all(|byte| *byte == 0xff)
                    {
                        flash
                            .write_bytes(
                                sector_base + page_off as u32,
                                &mut sector_buf[page_off..page_off + PAGE_SIZE],
                            )
                            .map_err(|_| NorFlashErrorKind::Other)?;
                    }
                }
            }

            rel_off += chunk as u32;
            src = &src[chunk..];
            writable -= chunk;
        }

        Ok(())
    }

    fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
        check_erase(self, from, to)?;
        let flash = self.flash.as_mut().ok_or(NorFlashErrorKind::Other)?;

        for sector_addr in (from..to).step_by(SECTOR_SIZE) {
            flash
                .erase_sectors(SETTINGS_OFFSET + sector_addr, 1)
                .map_err(|_| NorFlashErrorKind::Other)?;
        }

        Ok(())
    }
}
