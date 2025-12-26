#include <stdint.h>
#include <stddef.h>
#include <string.h>

#include "driver/driver.h"

#define THUMB(addr) ((addr) | 1u)
#define FN(addr, type) ((type)(uintptr_t)THUMB(addr))

#define NAFFS_OPEN_ADDR      0x0801B4A4u
#define NAFFS_CLOSE_ADDR     0x0801B6E4u
#define NAFFS_WRITE_ADDR     0x0801B7F8u
#define NAFFS_READ_ADDR      0x0801BA80u
#define NAFFS_DELETE_ADDR    0x0801BBB4u
#define NAFFS_GET_SIZE_ADDR  0x0801BE50u
#define NAFFS_SEEK_ADDR      0x0801BFECu

typedef int32_t (*naffs_open_fn)(uint32_t dev_id, uint32_t file_id, uint32_t mode);
typedef int32_t (*naffs_close_fn)(int32_t handle);
typedef int32_t (*naffs_write_fn)(int32_t handle, const void* src, uint32_t len);
typedef int32_t (*naffs_read_fn)(int32_t handle, void* dst, uint32_t len, int32_t* inout_len);
typedef int32_t (*naffs_delete_fn)(uint32_t dev_id, uint32_t file_id);
typedef int32_t (*naffs_get_size_fn)(uint32_t dev_id, uint32_t file_id, int32_t* out_size);
typedef int32_t (*naffs_seek_fn)(int32_t handle, uint32_t offset);

#define NAFFS_OPEN      FN(NAFFS_OPEN_ADDR, naffs_open_fn)
#define NAFFS_CLOSE     FN(NAFFS_CLOSE_ADDR, naffs_close_fn)
#define NAFFS_WRITE     FN(NAFFS_WRITE_ADDR, naffs_write_fn)
#define NAFFS_READ      FN(NAFFS_READ_ADDR, naffs_read_fn)
#define NAFFS_DELETE    FN(NAFFS_DELETE_ADDR, naffs_delete_fn)
#define NAFFS_GET_SIZE  FN(NAFFS_GET_SIZE_ADDR, naffs_get_size_fn)
#define NAFFS_SEEK      FN(NAFFS_SEEK_ADDR, naffs_seek_fn)

#ifndef CFW_NAFFS_DEV
#define CFW_NAFFS_DEV   0xA0u
#endif
#ifndef CFW_NAFFS_FILE
#define CFW_NAFFS_FILE  0xE7u
#endif
#ifndef CFW_NAFFS_TMP_FILE
#define CFW_NAFFS_TMP_FILE 0xE8u
#endif

#ifndef CFW_FLASH_CAPACITY
#define CFW_FLASH_CAPACITY 0x1000u
#endif

#define NAFFS_CHUNK 0x20u

static uint32_t u32_min(uint32_t a, uint32_t b) { return a < b ? a : b; }
static uint32_t u32_max(uint32_t a, uint32_t b) { return a > b ? a : b; }
static uint32_t align_up(uint32_t x, uint32_t a) { return (x + (a - 1u)) & ~(a - 1u); }

#define NAFFS_THREAD_ID_ADDR   0x2004FB1Cu
#define NAFFS_MAILBOX_ID_ADDR  0x2004FB20u

static inline int naffs_ready(void) {
    return (*(void* volatile*)NAFFS_THREAD_ID_ADDR != 0) &&
           (*(void* volatile*)NAFFS_MAILBOX_ID_ADDR != 0);
}

static int naffs_wait_ready(uint32_t spins) {
    while (spins--) {
        if (naffs_ready()) return 1;
        __asm volatile("nop");
    }
    return 0;
}

static uint32_t clamp_len(uint32_t off, uint32_t len) {
    if (off >= CFW_FLASH_CAPACITY) return 0;
    uint32_t max = CFW_FLASH_CAPACITY - off;
    return (len > max) ? max : len;
}

uint32_t driver_get_flash_size(void) {
    return CFW_FLASH_CAPACITY;
}

void driver_read_flash(uint32_t offset, uint8_t* data, uint32_t len) {
    if (!data || len == 0) return;

    len = clamp_len(offset, len);
    if (len == 0) return;

    memset(data, 0xFF, len);

    if (!naffs_wait_ready(800000u)) {
        return;
    }

    int32_t size_i = 0;
    if (NAFFS_GET_SIZE(CFW_NAFFS_DEV, CFW_NAFFS_FILE, &size_i) != 0 || size_i <= 0) return;
    uint32_t size = (uint32_t)size_i;

    if (offset >= size) return;
    uint32_t readable = u32_min(len, size - offset);

    int32_t h = NAFFS_OPEN(CFW_NAFFS_DEV, CFW_NAFFS_FILE, 0);
    if (h < 0) return;

    if (NAFFS_SEEK(h, offset) != 0) {
        NAFFS_CLOSE(h);
        return;
    }

    int32_t io = (int32_t)readable;
    if (NAFFS_READ(h, data, readable, &io) != 0) {
        NAFFS_CLOSE(h);
        return;
    }

    NAFFS_CLOSE(h);
}

static int delete_if_exists(uint32_t dev, uint32_t file) {
    int32_t s = 0;
    if (NAFFS_GET_SIZE(dev, file, &s) == 0 && s > 0) {
        (void)NAFFS_DELETE(dev, file);
    }
    return 0;
}

static int build_tmp_with_patch(uint32_t offset, const uint8_t* patch, uint32_t patch_len, uint32_t total_size, uint32_t old_size) {
    (void)NAFFS_DELETE(CFW_NAFFS_DEV, CFW_NAFFS_TMP_FILE);

    int32_t tmp = NAFFS_OPEN(CFW_NAFFS_DEV, CFW_NAFFS_TMP_FILE, 2);
    if (tmp < 0) return -1;

    int32_t src = -1;
    if (old_size != 0) {
        src = NAFFS_OPEN(CFW_NAFFS_DEV, CFW_NAFFS_FILE, 0);
        if (src >= 0) (void)NAFFS_SEEK(src, 0);
    }

    uint8_t buf[NAFFS_CHUNK];

    for (uint32_t pos = 0; pos < total_size; pos += NAFFS_CHUNK) {
        uint32_t chunk = NAFFS_CHUNK;

        memset(buf, 0xFF, chunk);

        if (src >= 0 && pos < old_size) {
            int32_t io = (int32_t)chunk;
            if (NAFFS_READ(src, buf, chunk, &io) != 0) {
                NAFFS_CLOSE(src);
                NAFFS_CLOSE(tmp);
                return -2;
            }
        }

        uint32_t patch_start = u32_max(pos, offset);
        uint32_t patch_end   = u32_min(pos + chunk, offset + patch_len);
        if (patch_start < patch_end) {
            uint32_t dst_off = patch_start - pos;
            uint32_t src_off = patch_start - offset;
            uint32_t n = patch_end - patch_start;
            memcpy(&buf[dst_off], &patch[src_off], n);
        }

        if (NAFFS_WRITE(tmp, buf, chunk) != 0) {
            if (src >= 0) NAFFS_CLOSE(src);
            NAFFS_CLOSE(tmp);
            return -3;
        }
    }

    if (src >= 0) NAFFS_CLOSE(src);
    NAFFS_CLOSE(tmp);
    return 0;
}

static int commit_tmp_to_target(uint32_t total_size) {
    int32_t tmp_r = NAFFS_OPEN(CFW_NAFFS_DEV, CFW_NAFFS_TMP_FILE, 0);
    if (tmp_r < 0) return -1;
    (void)NAFFS_SEEK(tmp_r, 0);

    delete_if_exists(CFW_NAFFS_DEV, CFW_NAFFS_FILE);

    int32_t dst = NAFFS_OPEN(CFW_NAFFS_DEV, CFW_NAFFS_FILE, 2);
    if (dst < 0) {
        NAFFS_CLOSE(tmp_r);
        return -2;
    }

    uint8_t buf[NAFFS_CHUNK];
    for (uint32_t pos = 0; pos < total_size; pos += NAFFS_CHUNK) {
        memset(buf, 0xFF, NAFFS_CHUNK);

        int32_t io = (int32_t)NAFFS_CHUNK;
        if (NAFFS_READ(tmp_r, buf, NAFFS_CHUNK, &io) != 0) {
            NAFFS_CLOSE(tmp_r);
            NAFFS_CLOSE(dst);
            return -3;
        }

        if (NAFFS_WRITE(dst, buf, NAFFS_CHUNK) != 0) {
            NAFFS_CLOSE(tmp_r);
            NAFFS_CLOSE(dst);
            return -4;
        }
    }

    NAFFS_CLOSE(tmp_r);
    NAFFS_CLOSE(dst);
    (void)NAFFS_DELETE(CFW_NAFFS_DEV, CFW_NAFFS_TMP_FILE);
    return 0;
}

void driver_write_flash(uint32_t offset, const uint8_t* data, uint32_t len) {
    if (!data || len == 0) return;

    len = clamp_len(offset, len);
    if (len == 0) return;

    int32_t old_i = 0;
    uint32_t old_size = 0;
    if (NAFFS_GET_SIZE(CFW_NAFFS_DEV, CFW_NAFFS_FILE, &old_i) == 0 && old_i > 0) {
        old_size = (uint32_t)old_i;
    }

    uint32_t total_size = align_up(CFW_FLASH_CAPACITY, NAFFS_CHUNK);

    if (build_tmp_with_patch(offset, data, len, total_size, old_size) != 0) return;

    (void)commit_tmp_to_target(total_size);
}
