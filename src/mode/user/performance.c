#include <mode/user/performance.h>
#include <mode/mode.h>
#include <led/led.h>
#include <driver/driver.h>
#include <utils/conversion.h>
#include <flash/settings.h>

uint8_t velocity_curve_high(uint8_t v);
uint8_t velocity_curve_low(uint8_t v);

void performance_init() { }

void performance_timer_event() { }

void performance_surface_event(uint8_t type, uint8_t index, uint8_t value) {
    uint8_t vel = 0;
    if (settings_velocity_curve == 0) {
        vel = velocity_curve_low(value);
    } else if (settings_velocity_curve == 2) {
        vel = velocity_curve_high(value);
    } else {
        vel = value;
    }

    driver_send_midi(1, (uint8_t[]){(uint8_t)(0x90), xy_dr[index], type ? (settings_velocity_enabled ? vel : 127) : 0}, 3);
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

uint8_t velocity_curve_low(uint8_t v)
{
    if (v == 0) return 0;
    if (v > 127) v = 127;

    uint16_t x = v;
    uint16_t y = (uint16_t)((x * x + 63u) / 127u);
    if (y > 127) y = 127;
    return (uint8_t)y;
}

uint8_t velocity_curve_high(uint8_t v)
{
    if (v == 0) return 0;
    if (v > 127) v = 127;

    uint16_t n = (uint16_t)v * 127u;

    uint16_t res = 0;
    uint16_t bit = 1u << 14;

    while (bit > n) bit >>= 2;
    while (bit != 0) {
        if (n >= res + bit) {
            n -= (uint16_t)(res + bit);
            res = (uint16_t)((res >> 1) + bit);
        } else {
            res >>= 1;
        }
        bit >>= 2;
    }

    if (res > 127) res = 127;
    return (uint8_t)res;
}