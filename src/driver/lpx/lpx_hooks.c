#include "driver/driver.h"

#include <stdint.h>
#include <stddef.h>
#include "app.h"

#define FW_BASE        0x0800C000u
#define SET_LED_ADDR   (FW_BASE + 0x0000E32u)
#define SCAN_ADDR      (FW_BASE + 0x000E87Cu)
#define SEND_SHORT_ADDR   (0x0800E3DEu)
#define TX_PUT_ADDR       (0x0800E452u)

#define PORT0_CTX_PP      ((void**)0x200005E0u)
#define PORT1_CTX_PP      ((void**)0x200005E4u)

#define OFF_IN_SYSEX  0x0Cu
#define OFF_BUF_PTR   0x10u
#define OFF_BUF_CAP   0x50u
#define OFF_RX_LEN    0x90u
#define OFF_MIDI_CB   0xD0u
#define OFF_SYSEX_CB  0x110u
#define OFF_COOKIE    0x150u

#define CB_COUNT_ADDR   ((volatile uint8_t *)0x200004A8u)
#define CB_PTRS32       ((volatile uint32_t*)0x20007834u)
#define CB_TYPE_BYTES   ((volatile uint8_t *)0x20007838u)

typedef void (*set_led_fn)(uint32_t index, uint32_t rgb);
typedef uint32_t (*scan_buttons_fn)();
typedef void (*midi_cb_t)(uint8_t port, uint8_t st, uint8_t d1, uint8_t d2);
typedef void (*sysex_cb_t)(void* cookie, const uint8_t* buf, uint32_t len);
typedef int32_t (*send_short_fn)(void* portCtx, uint8_t status, uint8_t d1, uint8_t d2, uint32_t dummy);
typedef uint32_t(*tx_put_fn)    (void* portCtx, uint8_t byte,   uint32_t prev, uint32_t acc);
typedef void (*pad_cb_t)(uint32_t idx, uint32_t val);

static pad_cb_t g_orig[0x20];
static uint8_t  g_hooked = 0;
static uint8_t  g_type_hooked[3] = {0,0,0};

#define SET_LED   ((set_led_fn)(SET_LED_ADDR | 1u))
#define ORIG_SCAN ((scan_buttons_fn)(SCAN_ADDR | 1u))

__attribute__((section(".cfw_keep")))
static void CFW_Midi_CB(uint8_t port, uint8_t st, uint8_t d1, uint8_t d2){
    app_midi_event(port, st, d1, d2);
}

__attribute__((section(".cfw_keep")))
static void CFW_SysEx_CB(void* cookie, const uint8_t* buf, uint32_t len){
    uint8_t port = (uint8_t)(uintptr_t)cookie;
    app_sysex_event(port, (uint8_t*)buf, (uint16_t)len);
}

__attribute__((used, noinline, section(".cfw_keep")))
void* CFW_RegPort_Replacement(
    void*     ctx,
    int       port,
    void*     rx_buf,
    uint32_t  rx_size,
    midi_cb_t midi_cb,
    sysex_cb_t sysex_cb,
    void*     cookie
){
    uint8_t* base = (uint8_t*)ctx;
    uint8_t* slot = base + ((port & 0xF) << 2);

    *(volatile void**   )(slot + OFF_BUF_PTR ) = rx_buf;
    *(volatile uint32_t*)(slot + OFF_BUF_CAP ) = rx_size;
    *(volatile uint32_t*)(slot + OFF_RX_LEN  ) = 0;
    *(volatile uint32_t*)(slot + OFF_IN_SYSEX) = 0;

    *(volatile uint32_t*)(slot + OFF_MIDI_CB ) = ((uint32_t)&CFW_Midi_CB ) | 1u;
    *(volatile uint32_t*)(slot + OFF_SYSEX_CB) = ((uint32_t)&CFW_SysEx_CB) | 1u;
    *(volatile void**   )(slot + OFF_COOKIE  ) = (void*)(uintptr_t)(port & 0xFF);

    return ctx;
}

static void Wrap_Press(uint32_t idx, uint32_t val){
    uint8_t x = idx % 10;
    uint8_t y = 9 - (idx / 10);

    uint8_t yx = y * 10 + x;

    app_surface_event(1, yx, (y == 9 || x == 9) ? 127 : (uint8_t)val);
}

static void Wrap_After(uint32_t idx, uint32_t val) {
    uint8_t yx = (9 - idx / 10) * 10 + idx % 10;

    app_aftertouch_event(yx, (uint8_t)val);
}

static void Wrap_Release(uint32_t idx, uint32_t val){
    uint8_t yx = (9 - idx / 10) * 10 + idx % 10;

    app_surface_event(0, yx, 0);
}

static void Drop_Handler(uint32_t idx, uint32_t val) { }

static inline pad_cb_t desired_for_type(uint8_t t){
    switch (t) {
        case 1: 
            return (pad_cb_t)((uintptr_t)Wrap_After   | 1u);
        case 0: 
            return (pad_cb_t)((uintptr_t)Wrap_Press   | 1u);
        case 2: 
            return (pad_cb_t)((uintptr_t)Wrap_Release | 1u);

        default: 
            return NULL;
    }
}
static inline int is_ours_or_drop(pad_cb_t p){
    uintptr_t a = ((uintptr_t)p) & ~1u;

    return a == (((uintptr_t)Wrap_After)   & ~1u) ||
           a == (((uintptr_t)Wrap_Press)   & ~1u) ||
           a == (((uintptr_t)Wrap_Release) & ~1u) ||
           a == (((uintptr_t)Drop_Handler) & ~1u);
}

void init_buttons() {
    uint8_t n = *CB_COUNT_ADDR;

    if (n > 0x20) n = 0x20;

    for (uint8_t i = 0; i < n; ++i){
        uint8_t t = CB_TYPE_BYTES[i<<3];

        if (t > 2) {
            continue;
        }

        pad_cb_t *slot = (pad_cb_t*)&CB_PTRS32[i<<1];

        if (is_ours_or_drop(*slot)) {
            continue;
        }

        if (!g_type_hooked[t]) {
            g_orig[i] = *slot;
            *slot = desired_for_type(t);
            g_type_hooked[t] = 1;
        }
    }

    for (uint8_t i = 0; i < n; ++i){
        uint8_t t = CB_TYPE_BYTES[i<<3];

        if (t > 2) {
            continue;
        }

        pad_cb_t *slot = (pad_cb_t*)&CB_PTRS32[i<<1];

        if (is_ours_or_drop(*slot)) {
            continue;
        }

        if (g_type_hooked[t]) {
            *slot = (pad_cb_t)((uintptr_t)Drop_Handler | 1u);
        }
    }

    g_hooked = n;
}

static uint8_t initialized = 0;

__attribute__((section(".cfw_keep")))
void CFW_AppTick() {
    if (!initialized) {
        initialized = 1;
        init_buttons();

        app_init();
    }

    app_timer_event();
}

void driver_set_led(uint8_t led, uint32_t color) {
    if (led >= 100) return;
    uint8_t yx = (uint8_t)((9 - (led / 10)) * 10 + (led % 10));
    SET_LED(yx, color);
}

static inline void* get_port_ctx(uint8_t port){
    if (port == 0) return *PORT0_CTX_PP;
    if (port == 1) return *PORT1_CTX_PP;
    return *PORT0_CTX_PP;
}

void driver_send_midi(uint8_t port, const uint8_t* data, uint16_t len){
    if (!data || !len) return;

    void* ctx = get_port_ctx(port);
    if (!ctx) return;

    send_short_fn SEND_SHORT = (send_short_fn)(SEND_SHORT_ADDR | 1u);
    tx_put_fn     TX_PUT     = (tx_put_fn    )(TX_PUT_ADDR    | 1u);

    if (data[0] >= 0xF8) {
        (void)SEND_SHORT(ctx, data[0], 0, 0, 0);
        return;
    }

    if (data[0] == 0xF0) {
        uint32_t prev = 0, acc = 0;
        for (uint16_t i = 0; i < len; ++i){
            acc = TX_PUT(ctx, data[i], prev, acc);
            prev = data[i];
            if (data[i] == 0xF7) break;
        }
        return;
    }

    uint8_t st = data[0];
    uint8_t d1 = (len > 1) ? data[1] : 0;
    uint8_t d2 = (len > 2) ? data[2] : 0;

    SEND_SHORT(ctx, st, d1, d2, 0);
}

__attribute__((section(".cfw_keep"), used))
void OFW_FUN_STUB(void) {}

__attribute__((section(".cfw_keep"), used))
int32_t OFW_FLASH_WORKER(void) {
    return 0;
}