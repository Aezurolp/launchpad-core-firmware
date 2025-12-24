#include "driver/driver.h"
#include "flash/settings.h"
#include <stdint.h>

void driver_set_led_rgb(uint8_t led, uint8_t red, uint8_t green, uint8_t blue) { }

uint8_t driver_get_brightness(void) {
    return settings_brightness;
}

void driver_set_brightness(uint8_t level) {
    settings_brightness = level;
}

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