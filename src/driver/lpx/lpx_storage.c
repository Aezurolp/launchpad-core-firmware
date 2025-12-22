#include <stdint.h>
#include <string.h>
#include <stdbool.h>
#include <driver/driver.h>

#define THUMB(a) ((a) | 1u)
#define FN(addr, type) ((type)(uintptr_t)THUMB(addr))

#define LPX_GET_DEV_ADDR      0x0800D786u
#define LPX_TICK_ADDR         0x0800D64Cu
#define LPX_GET_SIZE_ADDR     0x0800D510u

#define LPX_ERASE_ADDR        0x0800D52Au
#define LPX_WRITE256_ADDR     0x0800D59Cu
#define LPX_READ_ADDR         0x0800D5EEu

typedef void*    (*get_dev_fn)(void);
typedef int32_t  (*tick_fn)(void* dev);
typedef uint32_t (*get_size_fn)(void* dev);
typedef int32_t  (*erase_fn)(void* dev, uint32_t addr, uint32_t len, uint32_t cookie);
typedef int32_t  (*write256_fn)(void* dev, uint32_t addr, const uint8_t* src256);
typedef int32_t  (*read_fn)(void* dev, uint32_t addr, uint8_t* dst, uint32_t len);

#define FLASH_PAGE   0x100u
#define FLASH_SECTOR 0x1000u

static uint8_t s_page[FLASH_PAGE];
static uint8_t s_sector[FLASH_SECTOR];

static inline void* devptr(void) {
    return FN(LPX_GET_DEV_ADDR, get_dev_fn)();
}

static inline uint32_t flash_size(void* dev) {
    return FN(LPX_GET_SIZE_ADDR, get_size_fn)(dev);
}

static int wait_idle(void* dev) {
    tick_fn tick = FN(LPX_TICK_ADDR, tick_fn);

    for (uint32_t i = 0; i < 100000000u; i++) {
        if (tick(dev) == 0) return 0;
        __asm volatile("nop");
    }
    return -1;
}

static int start_read_aligned(void* dev, uint32_t addr_aligned, uint8_t* dst, uint32_t len) {
    read_fn r = FN(LPX_READ_ADDR, read_fn);
    wait_idle(dev);
    r(dev, addr_aligned, dst, len);
    return wait_idle(dev);
}

static int start_write_page(void* dev, uint32_t addr_aligned, const uint8_t page[FLASH_PAGE]) {
    write256_fn w = FN(LPX_WRITE256_ADDR, write256_fn);
    wait_idle(dev);
    w(dev, addr_aligned, page);
    return wait_idle(dev);
}

static int start_erase_4k(void* dev, uint32_t addr_4k) {
    erase_fn e = FN(LPX_ERASE_ADDR, erase_fn);
    wait_idle(dev);
    e(dev, addr_4k, FLASH_SECTOR, 0);
    return wait_idle(dev);
}

uint32_t driver_get_flash_size(void) {
    void* dev = devptr();
    return dev ? flash_size(dev) : 0;
}

void driver_read_flash(uint32_t offset, uint8_t* data, uint32_t len) {
    void* dev = devptr();
    if (!dev || !data || !len) return;

    uint32_t size = flash_size(dev);
    if (offset >= size) return;
    if (offset + len > size) len = size - offset;

    while (len) {
        uint32_t page_base = offset & ~(FLASH_PAGE - 1u);
        uint32_t in_page   = offset - page_base;
        uint32_t take      = FLASH_PAGE - in_page;
        if (take > len) take = len;

        if (start_read_aligned(dev, page_base, s_page, FLASH_PAGE) != 0) return;
        memcpy(data, &s_page[in_page], take);

        offset += take;
        data   += take;
        len    -= take;
    }
}

void driver_write_flash(uint32_t offset, const uint8_t* data, uint32_t len) {
    void* dev = devptr();
    if (!dev || !data || !len) return;

    uint32_t size = flash_size(dev);
    if (offset >= size) return;
    if (offset + len > size) len = size - offset;

    while (len) {
        uint32_t sector_base = offset & ~(FLASH_SECTOR - 1u);
        uint32_t in_sector   = offset - sector_base;
        uint32_t take        = FLASH_SECTOR - in_sector;
        if (take > len) take = len;

        for (uint32_t p = 0; p < FLASH_SECTOR; p += FLASH_PAGE) {
            if (start_read_aligned(dev, sector_base + p, &s_sector[p], FLASH_PAGE) != 0) return;
        }

        bool any_change = false;
        bool erase_needed = false;
        uint16_t dirty_pages = 0;

        for (uint32_t i = 0; i < take; i++) {
            uint32_t idx = in_sector + i;
            uint8_t oldv = s_sector[idx];
            uint8_t newv = data[i];
            if (oldv == newv) continue;

            any_change = true;
            dirty_pages |= (uint16_t)(1u << (idx >> 8));
            if (((uint8_t)(~oldv) & newv) != 0) erase_needed = true;
            s_sector[idx] = newv;
        }

        if (!any_change) {
            offset += take; data += take; len -= take;
            continue;
        }

        if (erase_needed) {
            if (start_erase_4k(dev, sector_base) != 0) return;
            for (uint32_t p = 0; p < FLASH_SECTOR; p += FLASH_PAGE) {
                if (start_write_page(dev, sector_base + p, &s_sector[p]) != 0) return;
            }
        } else {
            for (uint32_t page = 0; page < (FLASH_SECTOR / FLASH_PAGE); page++) {
                if ((dirty_pages & (1u << page)) == 0) continue;
                uint32_t p_off = page * FLASH_PAGE;
                if (start_write_page(dev, sector_base + p_off, &s_sector[p_off]) != 0) return;
            }
        }

        offset += take;
        data   += take;
        len    -= take;
    }
}
