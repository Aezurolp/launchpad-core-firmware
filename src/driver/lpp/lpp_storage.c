#include "lpp_app.h"
#include "lpp_app_defs.h"
#include <stdint.h>

uint32_t driver_get_flash_size() {
    return USER_AREA_SIZE;
}

void driver_write_flash(uint32_t offset, const uint8_t* data, uint32_t len) {
    hal_write_flash(offset, data, len);
}

void driver_read_flash(uint32_t offset, uint8_t* data, uint32_t len) {
    hal_read_flash(offset, data, len);
}