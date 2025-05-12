.global vec_context_switch
vec_context_switch:
    
    mov x28, lr
    
    bl vec_context_save    

    mov x0, x29 // info
    mrs x1, ESR_EL1 // esr
    mov x2, SP // tf

    bl handle_exception

    mov     x0, sp          // old_sp
    bl      switch_stack
    mov     sp, x0

    mov lr, x28
    b vec_context_restore // teleport

.global vec_context_save
vec_context_save:

    stp x26, x27, [SP, #-16]!
    stp x24, x25, [SP, #-16]!
    stp x22, x23, [SP, #-16]!
    stp x20, x21, [SP, #-16]!
    stp x18, x19, [SP, #-16]!
    stp x16, x17, [SP, #-16]!
    stp x14, x15, [SP, #-16]!
    stp x12, x13, [SP, #-16]!
    stp x10, x11, [SP, #-16]!
    stp x8, x9, [SP, #-16]!
    stp x6, x7, [SP, #-16]!
    stp x4, x5, [SP, #-16]!
    stp x2, x3, [SP, #-16]!
    stp x0, x1, [SP, #-16]!

    stp q30, q31, [SP, #-32]!
    stp q28, q29, [SP, #-32]!
    stp q26, q27, [SP, #-32]!
    stp q24, q25, [SP, #-32]!
    stp q22, q23, [SP, #-32]!
    stp q20, q21, [SP, #-32]!
    stp q18, q19, [SP, #-32]!
    stp q16, q17, [SP, #-32]!
    stp q14, q15, [SP, #-32]!
    stp q12, q13, [SP, #-32]!
    stp q10, q11, [SP, #-32]!
    stp q8, q9, [SP, #-32]!
    stp q6, q7, [SP, #-32]!
    stp q4, q5, [SP, #-32]!
    stp q2, q3, [SP, #-32]!
    stp q0, q1, [SP, #-32]!

    mrs x0, TTBR0_EL1
    mrs x1, TTBR1_EL1
    stp x0, x1, [SP, #-16]!

    mrs x0, SP_EL0
    mrs x1, TPIDR_EL0
    stp x0, x1, [SP, #-16]!

    mrs x0, ELR_EL1
    mrs x1, SPSR_EL1
    stp x0, x1, [SP, #-16]!

    ret


.global vec_context_restore
vec_context_restore:
    ldp x0, x1, [SP], #16
    msr ELR_EL1, x0
    msr SPSR_EL1, x1

    ldp x0, x1, [SP], #16
    msr SP_EL0, x0
    msr TPIDR_EL0, x1


    ldp x0, x1, [SP], #16
    msr TTBR0_EL1, x0
    msr TTBR1_EL1, x1

    ic   iallu
    dsb     ishst
    tlbi    vmalle1
    dsb     ish
    isb


    ldp q0, q1, [SP], #32
    ldp q2, q3, [SP], #32
    ldp q4, q5, [SP], #32
    ldp q6, q7, [SP], #32
    ldp q8, q9, [SP], #32
    ldp q10, q11, [SP], #32
    ldp q12, q13, [SP], #32
    ldp q14, q15, [SP], #32
    ldp q16, q17, [SP], #32
    ldp q18, q19, [SP], #32
    ldp q20, q21, [SP], #32
    ldp q22, q23, [SP], #32
    ldp q24, q25, [SP], #32
    ldp q26, q27, [SP], #32
    ldp q28, q29, [SP], #32
    ldp q30, q31, [SP], #32

    ldp x0, x1, [SP], #16
    ldp x2, x3, [SP], #16
    ldp x4, x5, [SP], #16
    ldp x6, x7, [SP], #16
    ldp x8, x9, [SP], #16
    ldp x10, x11, [SP], #16
    ldp x12, x13, [SP], #16
    ldp x14, x15, [SP], #16
    ldp x16, x17, [SP], #16
    ldp x18, x19, [SP], #16
    ldp x20, x21, [SP], #16
    ldp x22, x23, [SP], #16
    ldp x24, x25, [SP], #16
    ldp x26, x27, [SP], #16

    ret


// pair instructions ensure stack alignment of 16
.macro HANDLER source, kind
    .align 7
    stp     lr, xzr, [SP, #-16]!
    stp     x28, x29, [SP, #-16]!
    
    mov     x29, \source
    movk    x29, \kind, LSL #16
    bl      vec_context_switch
    
    ldp     x28, x29, [SP], #16
    ldp     lr, xzr, [SP], #16
    eret
.endm

// custom handler for nested FIQ in EL1 - don't switch stacks
.macro FIQ_NESTED_HANDLER
    .align 7
    stp     lr, xzr, [SP, #-16]!
    stp     x28, x29, [SP, #-16]!

    mov     x29, #1 // sub in for HANDLER 1, 2
    movk    x29, #2, LSL #16

    bl      vec_context_save

    mov     x0, x29            // Info
    mrs     x1, ESR_EL1        // esr
    mov     x2, SP             // TrapFrame pointer

    bl      handle_exception

    bl      vec_context_restore

    ldp     x28, x29, [SP], #16
    ldp     lr, xzr, [SP], #16
    eret
.endm


// ===================
// Vector table layout
// ===================
.align 11
.global vectors
vectors:
    HANDLER 0, 0          // CurrentSP_EL0, Synchronous
    HANDLER 0, 1          // CurrentSP_EL0, IRQ
    HANDLER 0, 2          // CurrentSP_EL0, FIQ
    HANDLER 0, 3          // CurrentSP_EL0, SError

    HANDLER 1, 0          // CurrentSP_ELx, Synchronous
    HANDLER 1, 1          // CurrentSP_ELx, IRQ
    FIQ_NESTED_HANDLER
    HANDLER 1, 3          // CurrentSP_ELx, SError

    HANDLER 2, 0          // Lower AArch64, Synchronous
    HANDLER 2, 1          // Lower AArch64, IRQ
    HANDLER 2, 2          // Lower AArch64, FIQ
    HANDLER 2, 3          // Lower AArch64, SError

    HANDLER 3, 0          // Lower AArch32, Synchronous (usually unused)
    HANDLER 3, 1          // Lower AArch32, IRQ
    HANDLER 3, 2          // Lower AArch32, FIQ
    HANDLER 3, 3          // Lower AArch32, SError