#include <stdint.h>

#define LPX_BR_GET_RAW_ADDR   0x0800D882u
#define LPX_BR_SET_RAW_ADDR   0x0800D878u
#define LPX_BR_APPLY_ADDR     0x0800CD8Cu

typedef uint32_t (*br_get_raw_fn)(void);
typedef int32_t  (*br_set_raw_fn)(uint8_t v);
typedef uint32_t (*br_apply_fn)  (uint8_t v);

static inline uint8_t lvl_from_raw(uint32_t raw) {
    return (uint8_t)((raw & 0xFFu) >> 5);
}

static inline uint8_t raw_from_lvl(uint8_t lvl) {
    if (lvl > 7) lvl = 7;
    uint8_t raw = (uint8_t)((lvl << 5) + 14u);
    raw &= 0xFEu;
    return raw;
}

uint8_t driver_get_brightness(void) {
    br_get_raw_fn GET = (br_get_raw_fn)(LPX_BR_GET_RAW_ADDR | 1u);
    uint32_t raw = GET() & 0xFFu;
    return lvl_from_raw(raw);
}

void driver_set_brightness(uint8_t level) {
    br_set_raw_fn SET   = (br_set_raw_fn)(LPX_BR_SET_RAW_ADDR | 1u);
    br_apply_fn   APPLY = (br_apply_fn)  (LPX_BR_APPLY_ADDR   | 1u);

    uint8_t raw = raw_from_lvl(level);

    *(volatile uint8_t*)0x200009e9 = raw;
    (void)APPLY(raw);
}
