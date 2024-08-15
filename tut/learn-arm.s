@ How would you write memcpy in ARMv8 assembly? (asm-memcpy)
asm-memcpy:
    subs    x2, x2, #8        // Subtract 8 from the count (set flags)
1:
    ldr     x3, [x0], #8      // Load an 8-byte word from source, post-increment source pointer
    str     x3, [x1], #8      // Store the 8-byte word to destination, post-increment destination pointer
    subs    x2, x2, #8        // Subtract 8 from the count (set flags)
    b.gt    1b                // If x2 > 0, loop
    ret                       // Return from function

@ How would you write 0xABCDE to ELR_EL1? (asm-movk)
asm-movk:
    movz    x0, #0xABCD, lsl #4   // Load lower 16 bits (0xABCD) and shift left by 4 bits
    movk    x0, #0xE, lsl #0      // Insert the lower 4 bits (0xE) into the lower 4 bits of x0
    msr     ELR_EL1, x0           // Move the value in x0 to ELR_EL1
