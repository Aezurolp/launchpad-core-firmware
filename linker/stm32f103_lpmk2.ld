MEMORY {
  FLASH (rx)  : ORIGIN = 0x08003400, LENGTH = 72K
  RAM   (rwx) : ORIGIN = 0x20000000, LENGTH = 20K
}

SECTIONS {
  .blob 0x08003400 : ALIGN(4)
  {
    KEEP(*(.blob*))
  } > FLASH

  . = ALIGN(128);
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
  .bss  : { *(.bss*)  *(COMMON) } > RAM
  
  . = ALIGN(16);
  .cfw_bss : ALIGN(16) {
    *(.cfw_bss*)
  } > RAM

  _sidata = LOADADDR(.data);
  _sdata  = ADDR(.data); _edata = _sdata + SIZEOF(.data);
  _sbss   = ADDR(.bss);  _ebss  = _sbss  + SIZEOF(.bss);
  _scfw_bss = ADDR(.cfw_bss); _ecfw_bss = _scfw_bss + SIZEOF(.cfw_bss);
  _estack = ORIGIN(RAM) + LENGTH(RAM);
}