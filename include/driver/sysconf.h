#ifndef SYSCONF_H
#define SYSCONF_H

#include <stdint.h>
#include <stdbool.h>

#define CFW_VERSION {0, 1, 0}

struct SystemConf {
    char * name;
    bool flash_supported;
};

extern const struct SystemConf system_conf;

#endif