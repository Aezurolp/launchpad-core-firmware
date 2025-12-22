#pragma once
#include <stdint.h>

void lppmk3_leds_init(void);

void lppmk3_leds_set_internal(uint8_t idx, uint32_t rgb24);

void lppmk3_leds_set_xy(uint8_t xy, uint32_t rgb24);

void lppmk3_leds_fill(uint32_t rgb24);

void lppmk3_leds_flush(void);
