// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025-2026 Anthony Hofmeister

use core::marker::PhantomData;

use crate::app::palette_editor::PaletteEditorApp;
use crate::app::performance::PerformanceApp;
use crate::app::programmer::ProgrammerApp;
use crate::app::setup::SetupApp;
use crate::app::{AftertouchEvent, App, AppId, MidiEvent, SurfaceEvent};
use crate::sys::led;
use crate::sys::midi::MidiPort;
#[cfg(not(feature = "no-setup-btn"))]
use crate::sys::settings;
use crate::sys::sysex::{DefaultSysExHandler, SysExHandler, modes};

#[cfg(feature = "no-setup-btn")]
const SETUP_HOLD_BUTTON_INDEX: u8 = 95;
#[cfg(feature = "no-setup-btn")]
const SETUP_HOLD_TICKS: u16 = 500;
#[cfg(not(feature = "no-setup-btn"))]
const SETUP_BUTTON_INDEX: u8 = 0;

pub struct AppHost<BootApp: App, LiveApp: App, Sysex: SysExHandler = DefaultSysExHandler> {
    pub current: AppId,
    previous_app: AppId,
    boot: BootApp,
    live: LiveApp,
    setup: SetupApp,
    performance: PerformanceApp,
    programmer: ProgrammerApp,
    palette_editor: PaletteEditorApp,
    #[cfg(feature = "no-setup-btn")]
    setup_hold_ticks: u16,
    #[cfg(feature = "no-setup-btn")]
    setup_hold_active: bool,
    sysex: PhantomData<Sysex>,
}

impl<BootApp: App, LiveApp: App, Sysex: SysExHandler> AppHost<BootApp, LiveApp, Sysex> {
    pub const fn new(current: AppId, boot: BootApp, live: LiveApp) -> Self {
        Self {
            current,
            boot,
            live,
            setup: SetupApp::new(),
            performance: PerformanceApp::new(),
            programmer: ProgrammerApp::new(),
            palette_editor: PaletteEditorApp::new(),
            previous_app: current,
            #[cfg(feature = "no-setup-btn")]
            setup_hold_ticks: 0,
            #[cfg(feature = "no-setup-btn")]
            setup_hold_active: false,
            sysex: PhantomData,
        }
    }

    fn active_app_mut(&mut self) -> &mut dyn App {
        match self.current {
            AppId::Boot => &mut self.boot,
            AppId::Setup => &mut self.setup,
            AppId::Performance => &mut self.performance,
            AppId::Live => &mut self.live,
            AppId::Programmer => &mut self.programmer,
            AppId::PaletteEditor => &mut self.palette_editor,
        }
    }

    pub fn init(&mut self) {
        self.active_app_mut().on_enter();
        self.apply_requested_app_switch();
    }

    pub fn switch(&mut self, app: AppId) {
        if app == self.current {
            return;
        }

        if app == AppId::Setup && self.current != AppId::PaletteEditor {
            self.previous_app = self.current;
        }

        self.active_app_mut().on_exit();

        self.current = app;

        led::clear();

        if app == AppId::Setup {
            self.setup.set_current_mode(self.previous_app);
        }

        self.active_app_mut().on_enter();
    }

    pub fn route_surface_event(&mut self, event: SurfaceEvent) {
        if (event.index != 0) {
            self.active_app_mut().on_surface(event);
        }

        #[cfg(feature = "no-setup-btn")]
        if self.handle_setup_hold_button(&event) {
            return;
        }

        #[cfg(not(feature = "no-setup-btn"))]
        if self.handle_setup_button(&event) {
            return;
        }

        self.apply_requested_app_switch();
    }

    pub fn route_midi_event(&mut self, event: MidiEvent) {
        if event.port == MidiPort::Daw {
            if self.current == AppId::Live {
                self.live.on_midi(event);
                self.apply_requested_app_switch();
            }
            return;
        }

        if Self::handle_led_tempo_event(&event) {
            return;
        }

        self.active_app_mut().on_midi(event);
        self.apply_requested_app_switch();
    }

    pub fn route_aftertouch_event(&mut self, event: AftertouchEvent) {
        self.active_app_mut().on_aftertouch(event);
        self.apply_requested_app_switch();
    }

    pub fn receive_sysex(&mut self, port: MidiPort, data: &[u8]) {
        if port == MidiPort::Daw {
            return;
        }

        if let Some(app) = modes::switch_target(data) {
            self.switch(app);
            return;
        }

        Sysex::execute(self.current, port, data);
        if let Some(app) = Sysex::take_requested_app_switch() {
            self.switch(app);
        }
    }

    pub fn route_tick_event(&mut self) {
        #[cfg(feature = "no-setup-btn")]
        self.tick_setup_hold_button();

        led::tick();
        self.active_app_mut().on_tick();
        self.apply_requested_app_switch();
    }

    fn apply_requested_app_switch(&mut self) {
        if let Some(app) = self.active_app_mut().take_requested_app_switch() {
            self.switch(app);
        }
    }

    fn handle_led_tempo_event(event: &MidiEvent) -> bool {
        match event.status {
            0xfa => {
                led::tempo_start();
                true
            }
            0xf8 => {
                led::tempo_midi_clock();
                true
            }
            0xfc => {
                led::tempo_stop();
                true
            }
            _ => false,
        }
    }

    fn exit_setup(&mut self) {
        let app = self.setup.finish_setup().unwrap_or(self.previous_app);
        self.switch(app);
    }

    #[cfg(feature = "no-setup-btn")]
    fn handle_setup_hold_button(&mut self, event: &SurfaceEvent) -> bool {
        if event.index != SETUP_HOLD_BUTTON_INDEX {
            return false;
        }

        if self.current == AppId::PaletteEditor {
            return false;
        }

        if self.current == AppId::Boot {
            return true;
        }

        if self.current == AppId::Setup {
            if event.pressed {
                self.exit_setup();
            }

            return true;
        }

        if event.pressed {
            self.setup_hold_ticks = 0;
            self.setup_hold_active = true;
        } else {
            self.setup_hold_active = false;
            self.setup_hold_ticks = 0;
        }

        true
    }

    #[cfg(not(feature = "no-setup-btn"))]
    fn handle_setup_button(&mut self, event: &SurfaceEvent) -> bool {
        if event.index != SETUP_BUTTON_INDEX {
            return false;
        }

        if self.current == AppId::Boot {
            return true;
        }

        if event.pressed {
            if self.current == AppId::Setup {
                self.exit_setup();
            } else if self.current == AppId::PaletteEditor {
                settings::save();
                self.switch(AppId::Setup);
            } else {
                self.switch(AppId::Setup);
            }
        }

        true
    }

    #[cfg(feature = "no-setup-btn")]
    fn tick_setup_hold_button(&mut self) {
        if !self.setup_hold_active {
            return;
        }

        self.setup_hold_ticks = self.setup_hold_ticks.saturating_add(1);

        if self.setup_hold_ticks >= SETUP_HOLD_TICKS {
            self.setup_hold_active = false;
            self.setup_hold_ticks = 0;

            self.route_surface_event(
                SurfaceEvent {
                    pressed: false,
                    index: 95,
                    value: 0
                }
            );

            self.switch(AppId::Setup);
        }
    }
}
