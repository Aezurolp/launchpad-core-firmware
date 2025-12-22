#include <flash/flash.h>
#include <driver/driver.h>
#include <flash/settings.h>


#define CONF_ID_BRIGHTNESS 1

void flash_read() {
    // confsys_init(&cfg);
    
    settings_brightness = 7;
    settings_velocity_curve = 0;
    settings_velocity_enabled = 0;
    settings_aftertouch_curve = 0;
    settings_aftertouch_mode = 0;
    settings_palette = 0;
    
    driver_read_flash(0, &settings_brightness, 1);
    driver_read_flash(1, &settings_velocity_curve, 1);
    driver_read_flash(2, &settings_velocity_enabled, 1);
    driver_read_flash(3, &settings_aftertouch_curve, 1);
    driver_read_flash(4, &settings_aftertouch_mode, 1);
    driver_read_flash(5, &settings_palette, 1);
    // driver_read_flash(6, &settings_custom_palette, 3 * 3 * 128);
    
    driver_set_brightness(settings_brightness);
    driver_set_velocity_curve(settings_velocity_curve);
    driver_set_velocity_enabled(settings_velocity_enabled);
    driver_set_aftertouch_curve(settings_aftertouch_curve);
    driver_set_aftertouch_mode(settings_aftertouch_mode);
}

void flash_write() {
    driver_write_flash(0, &settings_brightness, 1);
    driver_write_flash(1, &settings_velocity_curve, 1);
    driver_write_flash(2, &settings_velocity_enabled, 1);
    driver_write_flash(3, &settings_aftertouch_curve, 1);
    driver_write_flash(4, &settings_aftertouch_mode, 1);
    driver_write_flash(5, &settings_palette, 1);
    // driver_write_flash(6, &settings_custom_palette, 3 * 3 * 128);
}