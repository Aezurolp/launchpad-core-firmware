#include "lppmk3_leds.h"
#include "flash/settings.h"
#include <string.h>
#include <stdio.h>
#include <stdbool.h>

#define OFW_SET_LED_ADDR    0x080212a4u
#define OFW_PUSH_LEDS_ADDR  0x080259b4u

typedef int32_t (*ofw_set_led_fn)(uint32_t index, uint32_t rgb);
typedef int32_t (*ofw_push_leds_fn)(void);

#define OFW_SET_LED   ((ofw_set_led_fn)(OFW_SET_LED_ADDR | 1u))
#define OFW_PUSH_LEDS ((ofw_push_leds_fn)(OFW_PUSH_LEDS_ADDR | 1u))

static const uint8_t XY_P3[109] = { 41,  24,  25,  26,  27,  28,  29,  30,  31, 0, 15,  99, 100, 101, 102, 103, 104, 105, 106,  23, 14,  91,  92,  93,  94,  95,  96,  97,  98,  22, 13,  83,  84,  85,  86,  87,  88,  89,  90,  21, 12,  75,  76,  77,  78,  79,  80,  81,  82,  20, 11,  67,  68,  69,  70,  71,  72,  73,  74,  19, 10,  59,  60,  61,  62,  63,  64,  65,  66,  18, 9,  51,  52,  53,  54,  55,  56,  57,  58,  17, 8,  43,  44,  45,  46,  47,  48,  49,  50,  16, 40,   0,   1,   2,   3,   4,   5,   6,   7,  42, 0,  32,  33,  34,  35,  36,  37,  38,  39};

#define LED_INTERNAL_MAX 109u

__attribute__((section(".cfw_state"), aligned(4)))
static uint32_t g_fb[LED_INTERNAL_MAX];

__attribute__((section(".cfw_state"), aligned(4)))
static uint32_t g_hw[LED_INTERNAL_MAX];

__attribute__((section(".cfw_state")))
static uint8_t g_valid[LED_INTERNAL_MAX];

__attribute__((section(".cfw_state")))
static uint8_t g_dirty_any;

__attribute__((section(".cfw_state")))
static uint32_t g_last_flush_ms;
__attribute__((section(".cfw_state")))
static uint32_t g_rr_start;

static inline uint32_t rgb24(uint32_t c) { return (c & 0x00FFFFFFu); }

static inline uint8_t br_lvl_to_scale(uint8_t lvl)
{
    static const uint8_t map[8] = { 36u, 49u, 62u, 75u, 88u, 101u, 114u, 127u };
    return map[(uint8_t)(lvl & 7u)];
}

static inline uint32_t apply_brightness24(uint32_t c24, uint8_t lvl)
{
    uint8_t scale = br_lvl_to_scale(lvl);
    c24 &= 0x00FFFFFFu;
    if (scale >= 127u) {
        return c24;
    }
    uint32_t r = (c24 >> 16) & 0xFFu;
    uint32_t g = (c24 >> 8)  & 0xFFu;
    uint32_t b = (c24 >> 0)  & 0xFFu;

    r = (r * (uint32_t)scale) >> 7;
    g = (g * (uint32_t)scale) >> 7;
    b = (b * (uint32_t)scale) >> 7;

    return (uint32_t)((r << 16) | (g << 8) | b);
}

static void build_valid_table(void)
{
    memset(g_valid, 0, sizeof(g_valid));
    for (uint32_t xy = 0; xy < 109u; ++xy) {
        uint8_t idx = XY_P3[xy];
        if (idx < LED_INTERNAL_MAX) {
            g_valid[idx] = 1u;
        }
    }
}

void lppmk3_leds_init(void)
{
    build_valid_table();

    memset(g_fb, 0, sizeof(g_fb));
    
    for (uint32_t i = 0; i < LED_INTERNAL_MAX; ++i) g_hw[i] = 0xFFFFFFFFu;

    g_dirty_any = 1u;
    g_last_flush_ms = 0u;
    g_rr_start = 0u;
}

void lppmk3_leds_set_internal(uint8_t idx, uint32_t rgb24_in)
{
    if (idx >= LED_INTERNAL_MAX) return;
    if (!g_valid[idx]) return;

    uint32_t c = rgb24(rgb24_in);
    if (g_fb[idx] == c) return;

    g_fb[idx] = c;
    g_dirty_any = 1u;
}

void lppmk3_leds_set_xy(uint8_t xy, uint32_t rgb24_in)
{
    if (xy >= 109u) return;

    lppmk3_leds_set_internal(XY_P3[xy], rgb24_in);
}

void lppmk3_leds_fill(uint32_t rgb24_in)
{
    uint32_t c = rgb24(rgb24_in);
    for (uint32_t i = 0; i < LED_INTERNAL_MAX; ++i) {
        if (!g_valid[i]) continue;
        g_fb[i] = c;
    }
    g_dirty_any = 1u;
}

void lppmk3_leds_flush(void)
{
    if (!g_dirty_any) return;

    const uint32_t MAX_UPDATES_PER_FLUSH = 108u;
    uint32_t pushed = 0u;
    uint32_t i = g_rr_start;
    uint32_t scanned = 0u;
    bool pending_any = false;
    uint8_t br_lvl = settings_brightness; // 0..7

    while (scanned < LED_INTERNAL_MAX) {
        if (g_valid[i]) {
            uint32_t c = g_fb[i];
            uint32_t cs = apply_brightness24(c, br_lvl);
            if (g_hw[i] != cs) {
                pending_any = true;
                int32_t r = OFW_SET_LED(i, cs);
                if (r == 0) {
                    g_hw[i] = cs;
                    ++pushed;
                    if (pushed >= MAX_UPDATES_PER_FLUSH) {
                        ++i; if (i >= LED_INTERNAL_MAX) i = 0u;
                        break;
                    }
                } else {
                    break;
                }
            }
        }
        ++i; if (i >= LED_INTERNAL_MAX) i = 0u;
        ++scanned;
    }

    g_rr_start = i;

    if (pushed) {
        OFW_PUSH_LEDS();
    }

    if (!pending_any && scanned >= LED_INTERNAL_MAX && pushed == 0u) {
        g_dirty_any = 0u;
    } else {
        g_dirty_any = 1u;
    }
}

uint8_t driver_get_brightness(void)
{
    return (uint8_t)(settings_brightness & 7u);
}

void driver_set_brightness(uint8_t level)
{
    if (level > 7u) level = 7u;
    settings_brightness = level;

    for (uint32_t i = 0; i < LED_INTERNAL_MAX; ++i) {
        g_hw[i] = 0xFFFFFFFFu;
    }
    g_dirty_any = 1u;
}