#include <led/led.h>
#include <driver/driver.h>
#include <flash/settings.h>
#include <utils/palette.h>

#if defined(LPX)
#define NOVATION_RGB_PAL_ADDR   0x8018DB4u
#define NOVATION_RGB_PAL_U32    ((const uint32_t* const)(NOVATION_RGB_PAL_ADDR))
#elif defined(LPPMK3)
#define NOVATION_RGB_PAL_ADDR   0x8067300u
#define NOVATION_RGB_PAL_U32    ((const uint32_t* const)(NOVATION_RGB_PAL_ADDR))
#elif defined(LPMINI)
#define NOVATION_RGB_PAL_ADDR   0x801895Cu
#define NOVATION_RGB_PAL_U32    ((const uint32_t* const)(NOVATION_RGB_PAL_ADDR))
#endif

static inline uint8_t scale6to8(uint8_t v) {
    // Map 0..63 -> 0..255 with rounding
    return (uint8_t)(((uint32_t)v * 255u + 31u) / 63u);
}

static inline uint8_t scale8to6(uint8_t v) {
    // Map 0..255 -> 0..63 with rounding
    return (uint8_t)(((uint32_t)v * 63u + 127u) / 255u);
}

void set_led(uint8_t led, uint32_t color) {
    #if defined(LPPMK2) || defined(LPPRO)
        uint8_t r = (color >> 16) & 0xFF;
        uint8_t g = (color >> 8)  & 0xFF;
        uint8_t b = (color >> 0)  & 0xFF;

        r = scale8to6(r);
        g = scale8to6(g);
        b = scale8to6(b);

        driver_set_led_rgb(led, r, g, b);
    #else
        driver_set_led(led, color);
    #endif
}

void rgb_led(uint8_t led, uint8_t r, uint8_t g, uint8_t b) {
    #if defined(LPX) || defined(LPMINI) || defined(LPPMK3)
        driver_set_led(led, (uint32_t)((r << 16) | (g << 8) | b));
    #elif defined(LPPRO)
        driver_set_led_rgb(led, r, g, b);
    #endif
}

void palette_led(uint8_t led, uint8_t velocity) {
    #if defined(LPX) || defined(LPMINI) || defined(LPPMK3)
        if (settings_palette == 5) {
            uint8_t r = scale6to8(temporary_palette[0][velocity]);
            uint8_t g = scale6to8(temporary_palette[1][velocity]);
            uint8_t b = scale6to8(temporary_palette[2][velocity]);

            driver_set_led(led, (uint32_t)((r << 16) | (g << 8) | b));
            return;
        } else if (settings_palette >= 3) {
            // Custom palette
            uint8_t r = scale6to8(settings_custom_palette[settings_palette - 3][0][velocity]);
            uint8_t g = scale6to8(settings_custom_palette[settings_palette - 3][1][velocity]);
            uint8_t b = scale6to8(settings_custom_palette[settings_palette - 3][2][velocity]);

            driver_set_led(led, (uint32_t)((r << 16) | (g << 8) | b));
            return;
        } else if (settings_palette <= 2) {
            if (settings_palette == 0) {
                driver_set_led(led, NOVATION_RGB_PAL_U32[velocity]);
                return;
            }

            uint8_t r = scale6to8(native_palettes[settings_palette][0][velocity]);
            uint8_t g = scale6to8(native_palettes[settings_palette][1][velocity]);
            uint8_t b = scale6to8(native_palettes[settings_palette][2][velocity]);

            driver_set_led(led, (uint32_t)((r << 16) | (g << 8) | b));
            return;
        }
    #elif defined(LPPRO)
        if (settings_palette == 5) {
            uint8_t r = temporary_palette[0][velocity];
            uint8_t g = temporary_palette[1][velocity];
            uint8_t b = temporary_palette[2][velocity];

            driver_set_led_rgb(led, r, g, b);
            return;
        } else if (settings_palette >= 3) {
            // Custom palette
            uint8_t r = settings_custom_palette[settings_palette - 3][0][velocity];
            uint8_t g = settings_custom_palette[settings_palette - 3][1][velocity];
            uint8_t b = settings_custom_palette[settings_palette - 3][2][velocity];

            driver_set_led_rgb(led, r, g, b);
            return;
        } else if (settings_palette <= 2) {
            uint8_t r = native_palettes[settings_palette][0][velocity];
            uint8_t g = native_palettes[settings_palette][1][velocity];
            uint8_t b = native_palettes[settings_palette][2][velocity];

            driver_set_led_rgb(led, r, g, b);
            return;
        }
    #endif
}

void clear_led() {
    #if defined (LPPMK3)
    for (int i = 0; i < 109; i++) {
    #else
    for (int i = 0; i < 100; i++) {
    #endif
        set_led(i, 0);
    }
}