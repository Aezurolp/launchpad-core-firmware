#include "driver/driver.h"
#include "flash/settings.h"
#include "lpp_app.h"

void driver_set_velocity_curve(uint8_t curve) {
    settings_velocity_curve = curve;
}

void driver_set_velocity_enabled(uint8_t enabled) {
    settings_velocity_enabled = enabled;
}

uint8_t driver_get_velocity_curve(void) {
    return settings_velocity_curve;
}

uint8_t driver_get_velocity_enabled(void) {
    return settings_velocity_enabled;
}

void driver_set_aftertouch_curve(uint8_t curve) {
    settings_aftertouch_curve = curve;
}

void driver_set_aftertouch_mode(uint8_t mode) {
    settings_aftertouch_mode = mode;
}

uint8_t driver_get_aftertouch_curve(void) {
    return settings_aftertouch_curve;
}

uint8_t driver_get_aftertouch_mode(void) {
    return settings_aftertouch_mode;
}

void driver_send_midi(uint8_t port, const uint8_t* data, uint16_t len) {
    if (len > 3) {
        hal_send_sysex(port, data, len);
    } else {
        hal_send_midi(port, data[0], data[1], data[2]);
    }
}