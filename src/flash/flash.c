#include <flash/flash.h>
#include <driver/driver.h>
#include <flash/settings.h>

static uint8_t flash[1024] = {0};

void flash_read() {
    settings_brightness = 7;
    settings_velocity_curve = 0;
    settings_velocity_enabled = 0;
    settings_aftertouch_curve = 0;
    settings_aftertouch_mode = 0;
    settings_palette = 0;
    
    driver_read_flash(0, &flash[0], 1024);
    settings_brightness = flash[0];
    settings_velocity_enabled = flash[1];
    settings_velocity_curve = flash[2];
    settings_aftertouch_mode = flash[3];
    settings_aftertouch_curve = flash[4];
    settings_palette = flash[5];
    
    if (settings_brightness > 7) settings_brightness = 7;
    if (settings_velocity_curve > 1) settings_velocity_curve = 0;
    if (settings_velocity_enabled > 2) settings_velocity_enabled = 0;
    if (settings_aftertouch_curve > 2) settings_aftertouch_curve = 0;
    if (settings_aftertouch_mode > 2) settings_aftertouch_mode = 0;
    if (settings_palette > 6) settings_palette = 0;

    driver_set_brightness(settings_brightness);
    driver_set_velocity_curve(settings_velocity_curve);
    driver_set_velocity_enabled(settings_velocity_enabled);
    driver_set_aftertouch_curve(settings_aftertouch_curve);
    driver_set_aftertouch_mode(settings_aftertouch_mode);
}

void flash_write() {
    flash[0] = settings_brightness;
    flash[1] = settings_velocity_enabled;
    flash[2] = settings_velocity_curve;
    flash[3] = settings_aftertouch_mode;
    flash[4] = settings_aftertouch_curve;
    flash[5] = settings_palette;

    driver_write_flash(0, &flash[0], 1024);
}