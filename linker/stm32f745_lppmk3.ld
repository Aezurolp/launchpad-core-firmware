MEMORY
{
  FLASH (rx) : ORIGIN = 0x08010000, LENGTH = 458752
  RAM (rwx)  : ORIGIN = 0x20048000, LENGTH = 32K
  STATE (rwx) : ORIGIN = 0x20030000, LENGTH = 4K
}

SECTIONS
{
  .blob 0x08010000 : ALIGN(4)
  {
    KEEP(*(.blob*))
  } > FLASH

  . = 0x08076000;
  .text : {
    KEEP(*(.text.CFW_Reset))
    KEEP(*(.cfw_keep))
    *(.text*) *(.rodata*)
  } > FLASH

  . = ALIGN(16);
  .cfw_rodata : ALIGN(16) {
    KEEP(*(.cfw_rodata*))
  } > FLASH

  .data : { *(.data*) } > RAM AT > FLASH

  .padding : {
    . = ORIGIN(FLASH) + LENGTH(FLASH);
  } > FLASH

  .bss  : { *(.bss*)  *(COMMON) } > RAM

  . = ALIGN(16);
  .cfw_bss (NOLOAD) : ALIGN(16) {
    *(.cfw_bss*)
  } > RAM

  . = ALIGN(16);
  .cfw_state (NOLOAD) : ALIGN(16) {
    *(.cfw_state*)
  } > STATE

  _sidata = LOADADDR(.data);
  _sdata  = ADDR(.data); _edata = _sdata + SIZEOF(.data);
  _sbss   = ADDR(.bss);  _ebss  = _sbss  + SIZEOF(.bss);
  _scfw_bss = ADDR(.cfw_bss); _ecfw_bss = _scfw_bss + SIZEOF(.cfw_bss);
  _estack = ORIGIN(RAM) + LENGTH(RAM);
}
