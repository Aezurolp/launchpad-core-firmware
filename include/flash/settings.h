#ifndef SETTINGS_H
#define SETTINGS_H

#include <stdint.h>

extern uint8_t settings_brightness; // 0-7
extern uint8_t settings_velocity_curve; // 0-2
extern uint8_t settings_velocity_enabled; // 0-1
extern uint8_t settings_aftertouch_curve; // 0-2
extern uint8_t settings_aftertouch_mode; // 0-2
extern uint8_t settings_palette; // 0-1

extern uint8_t settings_custom_palette[3][3][128]; // RGB 0-63

#endif