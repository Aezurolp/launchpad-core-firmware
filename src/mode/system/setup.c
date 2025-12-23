#include "mode/system/setup.h"
#include "mode/mode.h"
#include "driver/driver.h"
#include "led/led.h"
#include "flash/settings.h"
#include "flash/flash.h"

#if defined(LPX) || defined(LPMINI)
__attribute__((section(".cfw_injection_misc_1")))
#endif
static const uint32_t headline_leds[4][28][2] = {
    {
        { 81, 0xff0000 }, { 82, 0xff0000 }, { 71, 0xff0000 }, { 61, 0xff0000 }, { 51, 0xff0000 }, { 52, 0xff0000 }, { 83, 0xff8888 }, { 84, 0xff8888 }, { 85, 0xff8888 }, { 73, 0xff8888 }, { 63, 0xff8888 }, { 64, 0xff8888 }, { 53, 0xff8888 }, { 86, 0xff0000 }, { 88, 0xff0000 }, { 76, 0xff0000 }, { 78, 0xff0000 }, { 66, 0xff0000 }, { 67, 0xff0000 }, { 68, 0xff0000 }, { 56, 0xff0000 }, { 57, 0x100000 }, { 58, 0xff0000 },
    },
    {
        { 81, 0x2020ff }, { 71, 0x2020ff }, { 61, 0x2020ff }, { 51, 0x2020ff }, { 52, 0x2020ff }, { 83, 0xccccff }, { 84, 0xccccff }, { 85, 0xccccff }, { 73, 0xccccff }, { 74, 0xccccff }, { 63, 0xccccff }, { 53, 0xccccff }, { 54, 0xccccff }, { 55, 0xccccff }, { 86, 0x2020ff }, { 87, 0x2020ff }, { 76, 0x2020ff }, { 78, 0x2020ff }, { 66, 0x2020ff }, { 68, 0x2020ff }, { 56, 0x2020ff }, { 57, 0x2020ff },
    },
    {
        { 81, 0xff4000 }, { 83, 0xff4000 }, { 71, 0xff4000 }, { 73, 0xff4000 }, { 61, 0xff4000 }, { 63, 0xff4000 }, { 52, 0xff4000 }, { 84, 0xfffbcc }, { 85, 0xfffbcc }, { 86, 0xfffbcc }, { 74, 0xfffbcc }, { 75, 0xfffbcc }, { 64, 0xfffbcc }, { 54, 0xfffbcc }, { 55, 0xfffbcc }, { 56, 0xfffbcc }, { 87, 0xff4000 }, { 77, 0xff4000 }, { 67, 0xff4000 }, { 57, 0xff4000 }, { 58, 0xff4000 },
    },
    {
        { 82, 0x4000ff }, { 71, 0x4000ff }, { 73, 0x4000ff }, { 61, 0x4000ff }, { 62, 0x4000ff }, { 63, 0x4000ff }, { 51, 0x4000ff }, { 53, 0x4000ff }, { 84, 0x9088ff }, { 85, 0x9088ff }, { 74, 0x9088ff }, { 64, 0x9088ff }, { 65, 0x9088ff }, { 54, 0x9088ff }, { 86, 0x4000ff }, { 87, 0x4000ff }, { 88, 0x4000ff }, { 77, 0x4000ff }, { 67, 0x4000ff }, { 57, 0x4000ff },
    },
};

#define MODES 2

static const uint8_t selectable_modes[MODES][2] = {
    { 11, MODE_PERFORMANCE },
    { 12, MODE_PROGRAMMER },
    // { 13, MODE_LIVE },
    // { 14, MODE_CHORD },
};

#if defined(LPX) || defined(LPPMK3) || defined(LPPRO)
#define PAGES 4
#else
#define PAGES 2
#endif


uint8_t page = 0;

// Rainbow animation state for the custom palette indicator (LED 25)
static uint8_t palette_rainbow_hue = 0;
static uint8_t palette_rainbow_tick = 0; // rate divider to control speed

// Simple color wheel: maps 0..255 to 0xRRGGBB
static inline uint32_t wheel(uint8_t pos) {
    uint8_t r, g, b;
    if (pos < 85) {
        r = (uint8_t)(255 - pos * 3);
        g = (uint8_t)(pos * 3);
        b = 0;
    } else if (pos < 170) {
        pos = (uint8_t)(pos - 85);
        r = 0;
        g = (uint8_t)(255 - pos * 3);
        b = (uint8_t)(pos * 3);
    } else {
        pos = (uint8_t)(pos - 170);
        r = (uint8_t)(pos * 3);
        g = 0;
        b = (uint8_t)(255 - pos * 3);
    }
    return (uint32_t)((r << 16) | (g << 8) | b);
}

void setup_init() {
    if (page > PAGES) page = 0;

    for (uint8_t i = 0; i < 28; ++i) {
        set_led(headline_leds[page][i][0], headline_leds[page][i][1]);
    }

    for (uint8_t i = 0; i < PAGES; ++i) {
        set_led(89 - (i * 10), 0x101014);
    }

    uint8_t dark_r = (headline_leds[page][0][1] >> 16) & 0xff;
    uint8_t dark_g = (headline_leds[page][0][1] >> 8) & 0xff;
    uint8_t dark_b = (headline_leds[page][0][1]) & 0xff;

    uint32_t page_highlight = (uint32_t)(((dark_r / 3) << 16) | ((dark_g / 3) << 8) | (dark_b / 3));

    set_led(89 - (page * 10), page_highlight);

    if (page == 0) { // <- CFW Page
        for (uint8_t i = 0; i < sizeof(selectable_modes) / sizeof(selectable_modes[0]); ++i) { // Display selectable mode
            set_led(selectable_modes[i][0], (mode == selectable_modes[i][1]) ? modes[selectable_modes[i][1]].color : modes[selectable_modes[i][1]].color_dimmed);
        }

        // Custom palette indicator (LED 25): off for system palettes, rainbow for custom
        if (settings_palette < 4) {
            set_led(25, 0x000000);
        } else {
            set_led(25, wheel(palette_rainbow_hue));
        }

        for (uint8_t i = 0; i < 3; i++) { // Custom Palettes
            set_led(26 + i, (settings_palette == 4 + i)? 0x70EEFF : 0x106080);
        }

        for (uint8_t i = 0; i < 4; i++) { // System Palettes
            set_led(15 + i, (settings_palette == i)? 0x6060FF : 0x101090);
        }

        set_led(18, settings_palette == 3 ? 0xFFCC80 : 0x906010);
    } else if (page == 1) {
        uint8_t brightness = driver_get_brightness();

        for (uint8_t i = 0; i < 8; ++i) {
            set_led(31 + i, 0x101010);
        }

        set_led(31 + brightness, 0xccccff);
    } else if (page == 2) {
        uint8_t vel_curve = driver_get_velocity_curve();
        uint8_t vel_enabled = driver_get_velocity_enabled();

        set_led(31, vel_enabled ? 0x10ff10 : 0xff1010);

        if (vel_enabled) {
            for (uint8_t i = 0; i < 3; ++i) {
                set_led(21 + i, 0x101010);
            }

            set_led(21 + vel_curve, 0xff4000);
        }
    } else if (page == 3) {
        uint8_t at_curve = driver_get_aftertouch_curve();
        uint8_t at_mode = driver_get_aftertouch_mode();

        set_led(31, at_mode == 0 ? 0xff1010 : 0x400101);
        set_led(32, at_mode == 1 ? 0x10ff10 : 0x014001);
        set_led(33, at_mode == 2 ? 0x10ff10 : 0x014001);

        if (at_mode != 0) {
            for (uint8_t i = 0; i < 3; ++i) {
                set_led(21 + i, 0x101010);
            }

            set_led(21 + at_curve, 0x4000ff);
        }
    }
}

void setup_timer_event() {
    // Animate LED 25 only on CFW page when a custom palette is selected
    if (page == 0 && settings_palette >= 4) {
        // Slow down animation by updating every few ticks
        if (++palette_rainbow_tick >= 6) { // adjust for desired speed
            palette_rainbow_tick = 0;
            palette_rainbow_hue++;
            set_led(25, wheel(palette_rainbow_hue));
        }
    }
}

void setup_surface_event(uint8_t type, uint8_t index, uint8_t value) {
    if (type != 1) return;

    if (index == 95 || (index == 0 && value != 0)) {
        mode_switch(mode);
        flash_write();
        return;
    }

    if (index % 10 == 9) { // Page buttons
        uint8_t new_page = 8 - (index / 10);
        if (new_page < PAGES && new_page != page) {
            page = new_page;
            
            mode_refresh();
        }
        return;
    }

    if (page == 0) { // <- CFW Page
        for (uint8_t i = 0; i < sizeof(selectable_modes) / sizeof(selectable_modes[0]); ++i) {
            if (index == selectable_modes[i][0]) {
                mode = selectable_modes[i][1];
                setup_init();
                return;
            }
        }

        if (index >= 26 && index <= 28) { // Custom Palettes
            settings_palette = index - 26 + 4;
            setup_init();
            return;
        } else if (index >= 15 && index <= 18) { // System Palettes
            settings_palette = index - 15;
            setup_init();
            return;
        }
    } else if (page == 1) { // <- LED Page
        if (index >= 31 && index <= 38) {
            uint8_t new_brightness = index - 31;
            driver_set_brightness(new_brightness);
            settings_brightness = new_brightness;

            setup_init();
        }
    } else if (page == 2) { // <- Velocity Page
        if (index == 31) {
            uint8_t enabled = driver_get_velocity_enabled();
            driver_set_velocity_enabled(!enabled);
            settings_velocity_enabled = !enabled;

            mode_refresh();
        } else if (index >= 21 && index <= 23) {
            uint8_t enabled = driver_get_velocity_enabled();
            if (!enabled) return;

            uint8_t new_curve = index - 21;
            driver_set_velocity_curve(new_curve);
            settings_velocity_curve = new_curve;

            setup_init();
        }
    } else if (page == 3) { // <- Aftertouch Page
        if (index >= 31 && index <= 33) {
            uint8_t new_mode = index - 31;
            driver_set_aftertouch_mode(new_mode);
            settings_aftertouch_mode = new_mode;

            mode_refresh();
        } else if (index >= 21 && index <= 23) {
            uint8_t new_curve = index - 21;
            driver_set_aftertouch_curve(new_curve);
            settings_aftertouch_curve = new_curve;

            setup_init();
        }
    }
}

void setup_midi_event(uint8_t port, uint8_t status, uint8_t d1, uint8_t d2) { }

void setup_aftertouch_event(uint8_t index, uint8_t value) { }