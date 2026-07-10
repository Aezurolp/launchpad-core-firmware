// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

pub mod fastled;
pub mod led_control;
pub mod modes;
pub mod palette;
pub mod version_inquiry;

use crate::app::AppId;
use crate::sys::midi::MidiPort;

pub trait SysExHandler {
    fn execute(app: AppId, port: MidiPort, data: &[u8]) -> bool;

    fn take_requested_app_switch() -> Option<AppId> {
        None
    }
}

pub struct DefaultSysExHandler;

impl SysExHandler for DefaultSysExHandler {
    fn execute(app: AppId, port: MidiPort, data: &[u8]) -> bool {
        if app == AppId::Performance && fastled::Handler::execute(app, port, data) {
            return true;
        }

        if version_inquiry::Handler::execute(app, port, data) {
            return true;
        }

        if palette::Handler::execute(app, port, data) {
            return true;
        }

        false
    }
}
