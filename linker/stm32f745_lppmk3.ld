/* stm32f745_lppmk3.ld - CFW safe RAM placement */

MEMORY
{
  FLASH   (rx)  : ORIGIN = 0x08010000, LENGTH = 0x00070000
  CFW_RAM (rwx) : ORIGIN = 0x2000F000, LENGTH = 0x00002000
  STATE   (rwx) : ORIGIN = 0x20030000, LENGTH = 0x00001000
}

SECTIONS
{
  .blob 0x08010000 : ALIGN(4)
  {
    KEEP(*(.blob*))
  } > FLASH

  . = 0x08076000;
  .text : ALIGN(4)
  {
    KEEP(*(.text.CFW_Reset))
    KEEP(*(.cfw_keep))

    *(.text*) *(.text.*)
    *(.rodata*) *(.rodata.*)

    *(.glue_7*) *(.glue_7t*)
    *(.ARM.extab* .gnu.linkonce.armextab.*)
  } > FLASH

  .ARM.exidx : ALIGN(4)
  {
    __exidx_start = .;
    *(.ARM.exidx* .gnu.linkonce.armexidx.*)
    __exidx_end = .;
  } > FLASH

  . = ALIGN(16);
  .cfw_rodata : ALIGN(16)
  {
    KEEP(*(.cfw_rodata*))
  } > FLASH

  . = ORIGIN(CFW_RAM);

  .data : ALIGN(4)
  {
    _sdata = .;
    *(.data*) *(.data.*)
    . = ALIGN(4);
    _edata = .;
  } > CFW_RAM AT > FLASH

  _sidata = LOADADDR(.data);

  .bss (NOLOAD) : ALIGN(4)
  {
    _sbss = .;
    *(.bss*) *(.bss.*) *(COMMON)
    . = ALIGN(4);
    _ebss = .;
  } > CFW_RAM

  . = ALIGN(16);
  .cfw_bss (NOLOAD) : ALIGN(16)
  {
    _scfw_bss = .;
    *(.cfw_bss*)
    . = ALIGN(16);
    _ecfw_bss = .;
  } > CFW_RAM

  . = ALIGN(16);
  .cfw_state (NOLOAD) : ALIGN(16)
  {
    *(.cfw_state*)
  } > STATE

  .padding :
  {
    . = ORIGIN(FLASH) + LENGTH(FLASH);
  } > FLASH

  _estack = ORIGIN(CFW_RAM) + LENGTH(CFW_RAM);
}
