#include <stdint.h>

#define MINI_BR_GET_RAW_ADDR   0x0800DF32u
#define MINI_BR_SET_RAW_ADDR   0x0800DF28u
#define MINI_BR_APPLY_ADDR     0x0800D440u

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
    br_get_raw_fn GET = (br_get_raw_fn)(MINI_BR_GET_RAW_ADDR | 1u);
    uint32_t raw = GET() & 0xFFu;
    return lvl_from_raw(raw);
}

void driver_set_brightness(uint8_t level) {
    br_set_raw_fn SET   = (br_set_raw_fn)(MINI_BR_SET_RAW_ADDR | 1u);
    br_apply_fn   APPLY = (br_apply_fn)  (MINI_BR_APPLY_ADDR   | 1u);

    uint8_t raw = raw_from_lvl(level);
    (void)SET(raw);
    (void)APPLY(raw);
}
