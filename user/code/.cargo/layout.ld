SECTIONS {
  . = 0xffffffffc0000000;

  /* start of the binary */
  __text_beg = .;

  .text : {
        *(.text._start)
        *(.text .text.* .gnu.linkonce.t*)
  }

  .rodata : {
    *(.rodata .rodata.* .gnu.linkonce.r*)
  }

  .data : {
    *(.data .data.* .gnu.linkonce.d*)
  }

    /* load bss so we can directly map virtual memory */
  .bss : {
    . = ALIGN(32);
    __bss_beg = .;
    *(.bss .bss.*)
    *(COMMON)
    . = ALIGN(8);
    __bss_end = .;
  }

  /* end of the binary */
  __text_end = ALIGN(8);

  /* number of bytes in BSS section and complete binary */
  __bss_len = (__bss_end - __bss_beg);
  __text_len = (__text_end - __text_beg);

  /DISCARD/ : { *(.comment) *(.gnu*) *(.note*) *(.eh_frame*) }
}
