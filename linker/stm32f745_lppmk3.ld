MEMORY
{
  FLASH (rx) : ORIGIN = 0x08010000, LENGTH = 458752
  RAM (rwx)  : ORIGIN = 0x2003C000, LENGTH = 32K
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

  . = ORIGIN(RAM);

  .data : ALIGN(4) {
    _sdata = .;
    *(.data*)
    _edata = .;
  } > RAM AT > FLASH

  _sidata = LOADADDR(.data);

  .bss (NOLOAD) : ALIGN(4) {
    _sbss = .;
    *(.bss*) *(COMMON)
    _ebss = .;
  } > RAM

  . = ALIGN(16);
  .cfw_bss (NOLOAD) : ALIGN(16) {
    _scfw_bss = .;
    *(.cfw_bss*)
    _ecfw_bss = .;
  } > RAM

  . = ALIGN(16);
  .cfw_state (NOLOAD) : ALIGN(16) {
    *(.cfw_state*)
  } > STATE

  .padding : {
    . = ORIGIN(FLASH) + LENGTH(FLASH);
  } > FLASH

  _estack = ORIGIN(RAM) + LENGTH(RAM);
}
