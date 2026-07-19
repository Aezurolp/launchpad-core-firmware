// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

use core::cmp::min;
use embedded_storage::{ReadStorage, nor_flash::NorFlash};
use esp_storage::FlashStorage;

const SETTINGS_OFFSET: u32 = 0x7b_0000;
const SETTINGS_SIZE: u32 = firmware_core::sys::settings::SETTINGS_FLASH_SIZE as u32;
const SECTOR_SIZE: usize = 4096;

pub struct SettingsFlash {
    flash: FlashStorage<'static>,
}

impl SettingsFlash {
    pub fn new(flash: esp_hal::peripherals::FLASH<'static>) -> Self {
        Self {
            flash: FlashStorage::new(flash),
        }
    }

    pub const fn settings_size(&self) -> u32 {
        SETTINGS_SIZE
    }

    pub fn read(&mut self, offset: u32, data: &mut [u8]) {
        if offset >= SETTINGS_SIZE {
            data.fill(0xff);
            return;
        }
        let readable = min(data.len(), (SETTINGS_SIZE - offset) as usize);
        if ReadStorage::read(
            &mut self.flash,
            SETTINGS_OFFSET + offset,
            &mut data[..readable],
        )
        .is_err()
        {
            data[..readable].fill(0xff);
        }
        data[readable..].fill(0xff);
    }

    pub fn write(&mut self, offset: u32, mut data: &[u8]) {
        if offset >= SETTINGS_SIZE || data.is_empty() {
            return;
        }
        let mut relative = offset;
        let mut remaining = min(data.len(), (SETTINGS_SIZE - offset) as usize);
        let mut sector = [0xff; SECTOR_SIZE];

        while remaining != 0 {
            let absolute = SETTINGS_OFFSET + relative;
            let sector_base = absolute & !((SECTOR_SIZE as u32) - 1);
            let in_sector = (absolute - sector_base) as usize;
            let count = min(remaining, SECTOR_SIZE - in_sector);
            if ReadStorage::read(&mut self.flash, sector_base, &mut sector).is_err() {
                return;
            }
            if sector[in_sector..in_sector + count] != data[..count] {
                sector[in_sector..in_sector + count].copy_from_slice(&data[..count]);
                if self
                    .flash
                    .erase(sector_base, sector_base + SECTOR_SIZE as u32)
                    .is_err()
                {
                    return;
                }
                if self.flash.write(sector_base, &sector).is_err() {
                    return;
                }
            }
            relative += count as u32;
            remaining -= count;
            data = &data[count..];
        }
    }
}
