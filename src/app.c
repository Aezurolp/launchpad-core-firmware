#include "app.h"
#include "led/led.h"
#include "mode/mode.h"
#include "flash/flash.h"
#include "utils/sysex.h"

#if defined(LPX) || defined(LPMINI)
uint8_t session_button_down = 0;
uint16_t session_button_tick = 0;
#endif

#if defined(LPPRO)
void alt_app_init() {
#else
void app_init() {
#endif
    #if defined(LPX) || defined(LPMINI)
    session_button_down = 0;
    session_button_tick = 0;
    #endif

    flash_read();

    mode_switch(MODE_BOOT);
}

#if defined(LPPRO)
void alt_app_timer_event() {
#else
void app_timer_event() {
#endif
    #if defined(LPX) || defined(LPMINI)
    if (current_mode != MODE_SETUP) {
        if (session_button_tick >= 750) {
            modes[current_mode].surface_event(0, 95, 0);
            session_button_down = 0;
            session_button_tick = 0;

            mode_switch(MODE_SETUP);
        } else if (session_button_down) {
            session_button_tick++;
        }
    }
    #endif

    modes[current_mode].timer_event();
}

#if defined(LPPRO)
void alt_app_surface_event(uint8_t type, uint8_t index, uint8_t value) {
#else
void app_surface_event(uint8_t type, uint8_t index, uint8_t value) {
#endif
    #if defined(LPX) || defined(LPMINI) 
    if (current_mode != MODE_SETUP && current_mode != MODE_BOOT) {
        if (type && index == 95) {
            session_button_down = 1;
        } else if (type == 0 && index == 95) {
            session_button_down = 0;
            session_button_tick = 0;
        }
    }
    #endif

    #if defined(LPPRO) || defined(LPPMK3)
    if (current_mode != MODE_SETUP && current_mode != MODE_BOOT) {
        if (type == 1 && index == 0 && value != 0) {
            mode_switch(MODE_SETUP);
            return;
        }
    }
    #endif

    modes[current_mode].surface_event(type, index, value);
}


#if defined(LPPRO)
void alt_app_midi_event(uint8_t port, uint8_t status, uint8_t d1, uint8_t d2) {
#else
void app_midi_event(uint8_t port, uint8_t status, uint8_t d1, uint8_t d2) {
#endif
    modes[current_mode].midi_event(port, status, d1, d2);
}


#if defined(LPPRO)
void alt_app_aftertouch_event(uint8_t index, uint8_t value) {
#else
void app_aftertouch_event(uint8_t index, uint8_t value) {
#endif
    modes[current_mode].aftertouch_event(index, value);
}

#if defined(LPPRO)
void alt_app_sysex_event(uint8_t port, uint8_t* buf, uint16_t len) {
#else
void app_sysex_event(uint8_t port, uint8_t* buf, uint16_t len) {
#endif
    handle_sysex(buf, len);
}