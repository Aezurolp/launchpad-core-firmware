#ifndef DRIVER_H
#define DRIVER_H

#include <stdint.h>

void driver_set_led(uint8_t led, uint32_t color);
void driver_set_led_rgb(uint8_t led, uint8_t red, uint8_t green, uint8_t blue);

uint8_t driver_get_brightness(void);
void driver_set_brightness(uint8_t level);

void driver_set_velocity_curve(uint8_t curve);
void driver_set_velocity_enabled(uint8_t enabled);
uint8_t driver_get_velocity_curve(void);
uint8_t driver_get_velocity_enabled(void);

void driver_set_aftertouch_curve(uint8_t curve);
void driver_set_aftertouch_mode(uint8_t mode); // 0 == disabled, 1 == poly, 2 == channel
uint8_t driver_get_aftertouch_curve(void);
uint8_t driver_get_aftertouch_mode(void);

void driver_send_midi(uint8_t port, const uint8_t* data, uint16_t len);

uint32_t driver_get_flash_size();
void driver_write_flash(uint32_t offset, const uint8_t* data, uint32_t len);
void driver_read_flash(uint32_t offset, uint8_t* data, uint32_t len);

#ifdef LPMK2
// LPMK2-specific functions
void lpmk2_set_led(uint8_t index, uint8_t red, uint8_t green, uint8_t blue);
void lpmk2_clear_all_leds();
void lpmk2_led_init();
uint32_t lpmk2_scan_buttons();
void lpmk2_button_init();
void lpmk2_send_midi(uint8_t port, uint8_t cmd, uint8_t data1, uint8_t data2);
void lpmk2_send_sysex(uint8_t port, const uint8_t* data, uint32_t length);
#endif

#endif