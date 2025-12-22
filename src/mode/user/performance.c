#include <mode/user/performance.h>
#include <mode/mode.h>
#include <led/led.h>
#include <driver/driver.h>
#include <utils/conversion.h>
#include <flash/settings.h>

void performance_init() { }

void performance_timer_event() { }

void performance_surface_event(uint8_t type, uint8_t index, uint8_t value) {
    driver_send_midi(1, (uint8_t[]){(uint8_t)(0x90), xy_dr[index], type ? (settings_velocity_enabled ? value : 127) : 0}, 3);
}

void performance_midi_event(uint8_t port, uint8_t status, uint8_t d1, uint8_t d2) {
    if (port != 1) return;

    #if defined(LPX) || defined(LPMINI) || defined(LPPMK3)
    if (status == 0x90) {
        palette_led(dr_xy[d1], d2);
    }

    if (status == 176) {
        if (d1 >= 90 && d1 <= 99) {
            palette_led(d1, d2);
        }
    }

    if (status == 0x80) {
        palette_led(dr_xy[d1], 0);
    }
    #else
    if (status >= 144 && status <= 159) {
        if (d2 != 0) {
            palette_led(dr_xy[d1], d2);
        } else {
            palette_led(dr_xy[d1], 0);
        }
    } else if (status >= 128 && status <= 143) {
        palette_led(dr_xy[d1], 0);
    }
    #endif
}

void performance_aftertouch_event(uint8_t index, uint8_t value) {
    if (settings_aftertouch_mode == 1) {
        driver_send_midi(1, (uint8_t[]){(uint8_t)(160), xy_dr[index], value}, 3);
    } else if (settings_aftertouch_mode == 2) {
        driver_send_midi(1, (uint8_t[]){(uint8_t)(208), xy_dr[index], value}, 3);
    }
}
