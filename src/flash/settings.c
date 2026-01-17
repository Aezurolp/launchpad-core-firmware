#include <flash/settings.h>

uint8_t __attribute__((section(".cfw_bss"))) settings_brightness = 7; // 0-7
uint8_t __attribute__((section(".cfw_bss"))) settings_velocity_curve = 0; // 0-2
uint8_t __attribute__((section(".cfw_bss"))) settings_velocity_enabled = 0; // 0-1
uint8_t __attribute__((section(".cfw_bss"))) settings_aftertouch_curve = 1; // 0-2
uint8_t __attribute__((section(".cfw_bss"))) settings_aftertouch_mode = 0; // 0-2
uint8_t __attribute__((section(".cfw_bss"))) settings_palette = 0; // 0-6

uint8_t __attribute__((section(".cfw_bss"))) settings_custom_palette[3][3][128] = {0};