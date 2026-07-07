// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

use core::cmp::min;
use core::ptr;

const SETTINGS_START: u32 = 0x0801_e000;
const SETTINGS_SIZE: u32 = 6 * 1024;
const PAGE_SIZE: u32 = 1024;

const FLASH_BASE: usize = 0x4002_2000;
const FLASH_KEYR: *mut u32 = (FLASH_BASE + 0x04) as *mut u32;
const FLASH_SR: *mut u32 = (FLASH_BASE + 0x0c) as *mut u32;
const FLASH_CR: *mut u32 = (FLASH_BASE + 0x10) as *mut u32;
const FLASH_AR: *mut u32 = (FLASH_BASE + 0x14) as *mut u32;

const FLASH_KEY1: u32 = 0x4567_0123;
const FLASH_KEY2: u32 = 0xcdef_89ab;
const FLASH_SR_BSY: u32 = 1 << 0;
const FLASH_SR_EOP: u32 = 1 << 5;
const FLASH_CR_PG: u32 = 1 << 0;
const FLASH_CR_PER: u32 = 1 << 1;
const FLASH_CR_STRT: u32 = 1 << 6;
const FLASH_CR_LOCK: u32 = 1 << 7;

pub struct Flash;

impl Flash {
    pub const fn new() -> Self {
        Self
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
        let src = (SETTINGS_START + offset) as *const u8;

        unsafe {
            ptr::copy_nonoverlapping(src, data.as_mut_ptr(), readable);
        }

        if readable < data.len() {
            data[readable..].fill(0xff);
        }
    }

    pub fn write_settings(&mut self, offset: u32, data: &[u8]) {
        if data.is_empty() || offset >= SETTINGS_SIZE {
            return;
        }

        let writable = min((SETTINGS_SIZE - offset) as usize, data.len());
        let mut page = [0xff; PAGE_SIZE as usize];
        let mut rel_off = offset;
        let mut src = &data[..writable];

        cortex_m::interrupt::free(|_| {
            unlock();

            while !src.is_empty() {
                let page_base = rel_off & !(PAGE_SIZE - 1);
                let in_page = (rel_off - page_base) as usize;
                let chunk = min(src.len(), PAGE_SIZE as usize - in_page);
                self.read_settings(page_base, &mut page);

                if page[in_page..in_page + chunk] != src[..chunk] {
                    page[in_page..in_page + chunk].copy_from_slice(&src[..chunk]);
                    erase_page(SETTINGS_START + page_base);
                    program_page(SETTINGS_START + page_base, &page);
                }

                rel_off += chunk as u32;
                src = &src[chunk..];
            }

            lock();
        });
    }
}

fn unlock() {
    if reg_read(FLASH_CR) & FLASH_CR_LOCK == 0 {
        return;
    }

    reg_write(FLASH_KEYR, FLASH_KEY1);
    reg_write(FLASH_KEYR, FLASH_KEY2);
}

fn lock() {
    reg_write(FLASH_CR, reg_read(FLASH_CR) | FLASH_CR_LOCK);
}

fn wait_ready() {
    while reg_read(FLASH_SR) & FLASH_SR_BSY != 0 {}
    reg_write(FLASH_SR, FLASH_SR_EOP);
}

fn erase_page(address: u32) {
    wait_ready();
    reg_write(FLASH_CR, FLASH_CR_PER);
    reg_write(FLASH_AR, address);
    reg_write(FLASH_CR, FLASH_CR_PER | FLASH_CR_STRT);
    wait_ready();
    reg_write(FLASH_CR, 0);
}

fn program_page(address: u32, data: &[u8; PAGE_SIZE as usize]) {
    wait_ready();
    reg_write(FLASH_CR, FLASH_CR_PG);

    for (index, bytes) in data.chunks_exact(2).enumerate() {
        let halfword = u16::from_le_bytes([bytes[0], bytes[1]]);
        if halfword != 0xffff {
            unsafe {
                ptr::write_volatile((address as usize + index * 2) as *mut u16, halfword);
            }
            wait_ready();
        }
    }

    reg_write(FLASH_CR, 0);
}

fn reg_read(reg: *mut u32) -> u32 {
    unsafe { ptr::read_volatile(reg) }
}

fn reg_write(reg: *mut u32, value: u32) {
    unsafe {
        ptr::write_volatile(reg, value);
    }
}
