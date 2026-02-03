CC=arm-none-eabi-gcc
CFLAGS=-mcpu=cortex-m4 -mthumb -mfpu=fpv4-sp-d16 -mfloat-abi=softfp -Os \
	-ffunction-sections -fdata-sections -fno-builtin

DEVICE ?= lpx

ifeq ($(DEVICE),lpx)
	CFLAGS+=-DLPX -Isrc/driver/lpx/
	LDFLAGS=-Wl,--gc-sections -T linker/stm32f401_lpx.ld
	DRIVER_SRC=src/driver/lpx/lpx_hooks.c \
			src/driver/lpx/lpx_leds.c \
			src/driver/lpx/lpx_boot.c \
			src/driver/lpx/lpx_storage.c \
			src/driver/lpx/lpx_buttons.c \
			src/driver/lpx/sysconf.c
	BLOB_SPLIT_DIR=build/lpx/blob_split
	BLOB_PART1_OBJ=$(BLOB_SPLIT_DIR)/blob_part1.o
	BLOB_PART2_OBJ=$(BLOB_SPLIT_DIR)/blob_part2.o
	BLOB_OBJ=$(BLOB_PART1_OBJ) $(BLOB_PART2_OBJ)
	VERSION=351
	SYSEX_TYPE=/x
	PATCHES_FILE=patches/lpx.json
	BUILD_METHOD=standard
  
  ORIG_FW_SYX=original/launchpadx-firmware-$(VERSION).syx
  ORIG_FW_BIN=original/launchpadx-firmware-$(VERSION).bin
  LPX_BASE=0x0800c000
  LPX_FREE_START=0x08019004
  LPX_FREE_END=0x0801a94c
  OFF_FREE_START=$(shell printf %d $$(( $(LPX_FREE_START) - $(LPX_BASE) )))
  OFF_AFTER_FREE_END=$(shell printf %d $$(( $(LPX_FREE_END) - $(LPX_BASE) + 1 )))
else ifeq ($(DEVICE),mini)
	CFLAGS+=-DLPMINI -fno-common
	LDFLAGS=-Wl,--gc-sections -Wl,-Map=build/mini/fw.map -T linker/stm32f401_lpmini.ld
	DRIVER_SRC=src/driver/mini/mini_hooks.c \
			  src/driver/mini/mini_boot.c \
			  src/driver/mini/mini_velocity_stubs.c \
			  src/driver/mini/mini_storage.c \
			  src/driver/mini/mini_leds.c \
			  src/driver/mini/sysconf.c
	BLOB_SPLIT_DIR=build/mini/blob_split
	BLOB_PART1_OBJ=$(BLOB_SPLIT_DIR)/blob_part1.o
	BLOB_PART2_OBJ=$(BLOB_SPLIT_DIR)/blob_part2.o
	BLOB_OBJ=$(BLOB_PART1_OBJ) $(BLOB_PART2_OBJ)
	VERSION=407
	SYSEX_TYPE=/minimk3
	PATCHES_FILE=patches/lpmini.json
	BUILD_METHOD=standard
  
  ORIG_FW_SYX=original/launchpadminimk3-firmware-$(VERSION).syx
  ORIG_FW_BIN=original/launchpadminimk3-firmware-$(VERSION).bin
  MINI_BASE=0x0800c000
  MINI_FREE_START=0x08018ac2
  MINI_FREE_END=0x0801a7ef
  OFF_FREE_START=$(shell printf %d $$(( $(MINI_FREE_START) - $(MINI_BASE) )))
  OFF_AFTER_FREE_END=$(shell printf %d $$(( $(MINI_FREE_END) - $(MINI_BASE) + 1 )))
else ifeq ($(DEVICE),lppmk3)
	CFLAGS+=-DLPPMK3
	LDFLAGS=-Wl,--gc-sections -T linker/stm32f745_lppmk3.ld
	DRIVER_SRC=src/driver/lppmk3/lppmk3_hooks.c \
			src/driver/lppmk3/lppmk3_boot.c \
			src/driver/lppmk3/lppmk3_leds.c \
			src/driver/lppmk3/lppmk3_storage.c \
			src/driver/lppmk3/lppmk3_driver.c \
			src/driver/lppmk3/lppmk3_threading.c \
			src/driver/lppmk3/sysconf.c
	BLOB_OBJ=libs/lppmk3_blob.o
	VERSION=385
	SYSEX_TYPE=/lppmk3
	PATCHES_FILE=patches/lppmk3.json
	BUILD_METHOD=standard
	ORIG_FW_SYX=original/launchpadpromk3-firmware-$(VERSION).syx
	ORIG_FW_BIN=original/launchpadpromk3-firmware-$(VERSION).bin
	PAD_REF=$(ORIG_FW_BIN)
	PAD_TO_SIZE=$(if $(wildcard $(PAD_REF)),$(shell stat -f%z $(PAD_REF)))
  
  LPPMK3_BASE=0x08010000
  LPPMK3_CFW_START=0x08076000
  DEFLATE_SIZE=$(shell printf %d $$(( $(LPPMK3_CFW_START) - $(LPPMK3_BASE) )))
else ifeq ($(DEVICE),mk2)
	CFLAGS:=$(filter-out -mcpu=cortex-m4 -mfpu=fpv4-sp-d16 -mfloat-abi=softfp,$(CFLAGS))
	CFLAGS+=-DLPMK2 -mcpu=cortex-m3
	LDFLAGS=-nostdlib -Wl,--gc-sections -T linker/stm32f103_lpmk2.ld
	DRIVER_SRC=src/driver/mk2/mk2_hooks.c \
			src/driver/mk2/mk2_boot.c
	BLOB_OBJ=libs/lpmk2_blob.o
	VERSION=171
	SYSEX_TYPE=/mk2
	ORIG_FW_SYX=original/launchpadmk2-firmware-$(VERSION).syx
	ORIG_FW_BIN=original/launchpadmk2-firmware-$(VERSION).bin
	PATCHES_FILE=patches/mk2.json
	BUILD_METHOD=standard
else ifeq ($(DEVICE),lpp)
	CFLAGS=-O2 -Wall -Iinclude \
		-D_STM32F103RBT6_ -D_STM3x_ -D_STM32x_ -mthumb -mcpu=cortex-m3 -fcommon -fsigned-char \
		-DSTM32F10X_MD -DUSE_STDPERIPH_DRIVER -DHSE_VALUE=6000000UL -DCMSIS -DUSE_GLOBAL_CONFIG \
		-ffunction-sections -fdata-sections -std=c99 -mlittle-endian -DPRO -DLPPRO
	LDFLAGS=-T linker/stm32_lpp.ld -u _start -u _Minimum_Stack_Size -mcpu=cortex-m3 -mthumb \
		-specs=nano.specs -specs=nosys.specs -nostdlib -Wl,-static -N -nostartfiles -Wl,--gc-sections
	DRIVER_SRC=src/driver/lpp/lpp_app.c \
			src/driver/lpp/lpp_boot.c \
			src/driver/lpp/lpp_leds.c \
			src/driver/lpp/lpp_storage.c \
			src/driver/lpp/lpp_driver.c \
			src/driver/lpp/sysconf.c
	LIB_OBJ=libs/lpp.a
	BLOB_OBJ=
	VERSION=000
	SYSEX_TYPE=/lpp
	PATCHES_FILE=
	BUILD_METHOD=standalone
endif

SRC=$(DRIVER_SRC) \
	src/app.c \
	src/led/led.c \
	src/utils/conversion.c \
	src/utils/palette.c \
	src/utils/sysex.c \
	src/flash/settings.c \
	src/flash/flash.c \
	src/mode/mode.c \
	src/mode/system/setup.c \
	src/mode/user/performance.c \
	src/mode/user/programmer.c \

INCLUDES=-Iinclude

BUILD_DIR=build/$(DEVICE)
SCRIPTS_DIR=tools

OBJS=$(patsubst %.c,$(BUILD_DIR)/%.o,$(SRC)) $(BLOB_OBJ) $(LIB_OBJ)

libs/%.o:
	@# Prebuilt object retained: $@

libs/%.a:
	@# Prebuilt library retained: $@

ifeq ($(DEVICE),lpx)
LANDFILL_BIN=original/launchpadx-firmware-351-LANDFILL.bin
else ifeq ($(DEVICE),mini)
LANDFILL_BIN=original/launchpadminimk3-firmware-407-LANDFILL.bin
else ifeq ($(DEVICE),lppmk3)
LANDFILL_BIN=patches/LPPMK3-385-LANDFILL.bin
else ifeq ($(DEVICE),mk2)
LANDFILL_BIN=patches/LPMK2-171-LANDFILL.bin
endif

.PHONY: all lpx mini lppmk3 lpp mk2 clean help

all: lpx mini lppmk3 lpp mk2

lpx:
	$(MAKE) DEVICE=lpx build/lpx/fw.elf build/lpx/fw.bin build/lpx/fw.patched.bin build/lpx/fw.patched.syx build/lpx/fw.patched.bin.bipa

mini:
	$(MAKE) DEVICE=mini build/mini/fw.elf build/mini/fw.bin build/mini/fw.patched.bin build/mini/fw.patched.syx build/mini/fw.patched.bin.bipa

lppmk3:
	$(MAKE) DEVICE=lppmk3 build/lppmk3/fw.elf build/lppmk3/fw.bin build/lppmk3/fw.patched.bin build/lppmk3/fw.patched.syx build/lppmk3/fw.patched.bin.bipa

mk2:
	$(MAKE) DEVICE=mk2 build/mk2/fw.elf build/mk2/fw.bin build/mk2/fw.patched.bin build/mk2/fw.patched.syx

lpp:
	$(MAKE) DEVICE=lpp build/lpp/fw.elf build/lpp/fw.bin build/lpp/fw.patched.bin build/lpp/fw.patched.syx

$(LANDFILL_BIN): $(PATCHES_FILE) $(SCRIPTS_DIR)/landfill.py
	@echo "Generating landfill binary from $(PATCHES_FILE)..."
	python3 $(SCRIPTS_DIR)/landfill.py $(PATCHES_FILE)

ifeq ($(DEVICE),lpx)
$(BLOB_PART1_OBJ) $(BLOB_PART2_OBJ): $(LANDFILL_BIN)
	@echo "Splitting landfill blob for LPX..."
	@mkdir -p $(BLOB_SPLIT_DIR)
	@echo " - part1: 0..$$(($(OFF_FREE_START)-1)) bytes"
	dd if=$(LANDFILL_BIN) of=$(BLOB_SPLIT_DIR)/blob_part1.bin bs=1 count=$(OFF_FREE_START)
	@echo " - part2: from byte $(OFF_AFTER_FREE_END) to EOF"
	dd if=$(LANDFILL_BIN) of=$(BLOB_SPLIT_DIR)/blob_part2.bin bs=1 skip=$(OFF_AFTER_FREE_END)
	@echo "Creating part1 object"
	arm-none-eabi-objcopy -I binary -O elf32-littlearm -B arm \
		--rename-section .data=.blob_part1 \
		$(BLOB_SPLIT_DIR)/blob_part1.bin $(BLOB_PART1_OBJ)
	@echo "Creating part2 object"
	arm-none-eabi-objcopy -I binary -O elf32-littlearm -B arm \
		--rename-section .data=.blob_part2 \
		$(BLOB_SPLIT_DIR)/blob_part2.bin $(BLOB_PART2_OBJ)
else ifeq ($(DEVICE),mini)
$(BLOB_PART1_OBJ) $(BLOB_PART2_OBJ): $(LANDFILL_BIN)
	@echo "Splitting landfill blob for Mini..."
	@mkdir -p $(BLOB_SPLIT_DIR)
	@echo " - part1: 0..$$(($(OFF_FREE_START)-1)) bytes"
	dd if=$(LANDFILL_BIN) of=$(BLOB_SPLIT_DIR)/blob_part1.bin bs=1 count=$(OFF_FREE_START)
	@echo " - part2: from byte $(OFF_AFTER_FREE_END) to EOF"
	dd if=$(LANDFILL_BIN) of=$(BLOB_SPLIT_DIR)/blob_part2.bin bs=1 skip=$(OFF_AFTER_FREE_END)
	@echo "Creating part1 object"
	arm-none-eabi-objcopy -I binary -O elf32-littlearm -B arm \
		--rename-section .data=.blob_part1 \
		$(BLOB_SPLIT_DIR)/blob_part1.bin $(BLOB_PART1_OBJ)
	@echo "Creating part2 object"
	arm-none-eabi-objcopy -I binary -O elf32-littlearm -B arm \
		--rename-section .data=.blob_part2 \
		$(BLOB_SPLIT_DIR)/blob_part2.bin $(BLOB_PART2_OBJ)
else ifeq ($(DEVICE),lppmk3)
libs/lppmk3_blob.o: $(ORIG_FW_BIN)
	@echo "Creating LPPMK3 blob object from deflated firmware..."
	@echo " - trimming at CFW start: $(LPPMK3_CFW_START) (size $(DEFLATE_SIZE) bytes)"
	dd if=$< of=original/LPPMK3-$(VERSION)-DEFLATED.bin bs=1 count=$(DEFLATE_SIZE)
	@mkdir -p $(dir $(BLOB_OBJ))
	arm-none-eabi-objcopy -I binary -O elf32-littlearm -B arm \
		--rename-section .data=.blob \
		original/LPPMK3-$(VERSION)-DEFLATED.bin $(BLOB_OBJ)
else ifeq ($(DEVICE),mk2)
$(BLOB_OBJ): $(ORIG_FW_BIN)
	@echo "Creating MK2 blob object from firmware..."
	@mkdir -p $(dir $(BLOB_OBJ))
	arm-none-eabi-objcopy -I binary -O elf32-littlearm -B arm \
		--rename-section .data=.blob \
		$(ORIG_FW_BIN) $(BLOB_OBJ)
endif
	@echo "Cleaning up temporary landfill binary..."
	# @rm -f $(LANDFILL_BIN)

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

$(BUILD_DIR)/%.o: %.c | $(BUILD_DIR)
	@mkdir -p $(dir $@)
	$(CC) $(CFLAGS) $(INCLUDES) -c $< -o $@

$(BUILD_DIR)/fw.elf: $(OBJS) | $(BUILD_DIR)
	$(CC) $(CFLAGS) $(OBJS) -o $@ $(LDFLAGS)

ifeq ($(DEVICE),lppmk3)
$(BUILD_DIR)/fw.bin: $(BUILD_DIR)/fw.elf
	@echo "Generating full LPPMK3 FLASH image via objcopy..."
	arm-none-eabi-objcopy -O binary $< $@
else
$(BUILD_DIR)/fw.bin: $(BUILD_DIR)/fw.elf
	arm-none-eabi-objcopy -O binary $< $@
endif

ifeq ($(BUILD_METHOD),standard)
$(BUILD_DIR)/fw.patched.bin: $(BUILD_DIR)/fw.elf $(BUILD_DIR)/fw.bin $(SCRIPTS_DIR)/patcher.py $(PATCHES_FILE)
	python3 $(SCRIPTS_DIR)/patcher.py $(PATCHES_FILE) $(BUILD_DIR)/fw.elf $(BUILD_DIR)/fw.bin $@
	@if [ -n "$(PAD_TO_SIZE)" ]; then \
		arm-none-eabi-objcopy -I binary -O binary --gap-fill 0xFF --pad-to $(PAD_TO_SIZE) $@ $@.padded && mv $@.padded $@; \
	fi
endif

ifeq ($(BUILD_METHOD),padded)
$(BUILD_DIR)/fw.patched.bin: $(BUILD_DIR)/fw.elf $(BUILD_DIR)/fw.bin $(SCRIPTS_DIR)/patcher.py $(PATCHES_FILE)
	python3 $(SCRIPTS_DIR)/patcher.py $(PATCHES_FILE) $(BUILD_DIR)/fw.elf $(BUILD_DIR)/fw.bin $@
endif

ifeq ($(BUILD_METHOD),standalone)
$(BUILD_DIR)/fw.patched.bin: $(BUILD_DIR)/fw.bin
	@echo "Using standalone method - no patching needed, binary already contains payload"
	cp $(BUILD_DIR)/fw.bin $@
endif

$(BUILD_DIR)/fw.patched.syx: $(BUILD_DIR)/fw.patched.bin $(SCRIPTS_DIR)/syxtool.py
	@python3 $(SCRIPTS_DIR)/syxtool.py --to-syx $(SYSEX_TYPE) $(VERSION) $(BUILD_DIR)/fw.patched.bin $(BUILD_DIR)/cfw.syx
	@cp $(BUILD_DIR)/cfw.syx build/$(DEVICE)-cfw.syx
	@echo "Created build/$(DEVICE)-cfw.syx"

ifeq ($(DEVICE),lpx)
$(BUILD_DIR)/fw.patched.bin.bipa: $(SCRIPTS_DIR)/bipa.py $(ORIG_FW_BIN) $(BUILD_DIR)/fw.patched.bin
	@echo "Creating BIPA patch for LPX (source=$(ORIG_FW_BIN))"
	python3 $(SCRIPTS_DIR)/bipa.py create --source $(ORIG_FW_BIN) --target $(BUILD_DIR)/fw.patched.bin
	@cp $@ build/$(DEVICE)-cfw.bipa
	@echo "Created build/$(DEVICE)-cfw.bipa"
else ifeq ($(DEVICE),mini)
$(BUILD_DIR)/fw.patched.bin.bipa: $(SCRIPTS_DIR)/bipa.py $(ORIG_FW_BIN) $(BUILD_DIR)/fw.patched.bin
	@echo "Creating BIPA patch for Mini (source=$(ORIG_FW_BIN))"
	python3 $(SCRIPTS_DIR)/bipa.py create --source $(ORIG_FW_BIN) --target $(BUILD_DIR)/fw.patched.bin
	@cp $@ build/$(DEVICE)-cfw.bipa
	@echo "Created build/$(DEVICE)-cfw.bipa"
else ifeq ($(DEVICE),lppmk3)
$(BUILD_DIR)/fw.patched.bin.bipa: $(SCRIPTS_DIR)/bipa.py $(ORIG_FW_BIN) $(BUILD_DIR)/fw.patched.bin
	@echo "Creating BIPA patch for LPPMK3 (source=$(ORIG_FW_BIN))"
	python3 $(SCRIPTS_DIR)/bipa.py create --source $(ORIG_FW_BIN) --target $(BUILD_DIR)/fw.patched.bin
	@cp $@ build/$(DEVICE)-cfw.bipa
	@echo "Created build/$(DEVICE)-cfw.bipa"
endif

original/%.bin: original/%.syx $(SCRIPTS_DIR)/syxtool.py
	@echo "Converting $< to $@ via syxtool..."
	python3 $(SCRIPTS_DIR)/syxtool.py --to-bin $< $@

clean:
	rm -rf build/
	rm -f *-LANDFILL.bin

help:
	@echo "Build targets:"
	@echo "  all           Build all devices (default: DEVICE=$(DEVICE))"
	@echo "  lpx           Build LPX firmware and syx"
	@echo "  mini          Build LP Mini MK3 firmware and syx"
	@echo "  lppmk3        Build Launchpad Pro MK3 firmware and syx"
	@echo "  mk2           Build Launchpad MK2 firmware and syx"
	@echo "  lpp           Build Launchpad Pro (standalone)"
	@echo "Variables:"
	@echo "  DEVICE=<lpx|mini|lppmk3|mk2|lpp>  Select device"
	@echo "  CC, CFLAGS, LDFLAGS can be overridden as usual"