#ifndef LED_H
#define LED_H

#include <stdint.h>

void set_led(uint8_t led, uint32_t color);
void palette_led(uint8_t led, uint8_t velocity);
void novation_led(uint8_t led, uint8_t velocity);
void rgb_led(uint8_t led, uint8_t r, uint8_t g, uint8_t b);

void clear_led();

#endif