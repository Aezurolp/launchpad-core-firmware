#include "driver/driver.h"
#include "flash/settings.h"
#include "lpp_app_defs.h"
#include "lpp_app.h"

double apply_brightness(u8 c) {
    if (c == 0) return 0;

    u32 x = (u32)settings_brightness;
    return ((c - 1) * (63 * (x + 5) * (x + 5) - 144) / (62 * 144)) + 1;
}

void driver_set_led(uint8_t led, uint32_t color) {}

void driver_set_led_rgb(uint8_t led, uint8_t red, uint8_t green, uint8_t blue) {
    hal_plot_led(led == 99? TYPESETUP : TYPEPAD, led, apply_brightness(red), apply_brightness(green), apply_brightness(blue));
}

uint8_t driver_get_brightness(void) {
    return settings_brightness;
}

void driver_set_brightness(uint8_t level) {
    settings_brightness = level;
}