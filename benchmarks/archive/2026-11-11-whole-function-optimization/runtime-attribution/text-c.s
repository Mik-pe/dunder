	.build_version macos, 27, 2	sdk_version 27, 2
	.section	__TEXT,__text,regular,pure_instructions
	.globl	_main                           ; -- Begin function main
	.p2align	2
_main:                                  ; @main
	.cfi_startproc
; %bb.0:
	sub	sp, sp, #80
	stp	x24, x23, [sp, #16]             ; 16-byte Folded Spill
	stp	x22, x21, [sp, #32]             ; 16-byte Folded Spill
	stp	x20, x19, [sp, #48]             ; 16-byte Folded Spill
	stp	x29, x30, [sp, #64]             ; 16-byte Folded Spill
	add	x29, sp, #64
	.cfi_def_cfa w29, 16
	.cfi_offset w30, -8
	.cfi_offset w29, -16
	.cfi_offset w19, -24
	.cfi_offset w20, -32
	.cfi_offset w21, -40
	.cfi_offset w22, -48
	.cfi_offset w23, -56
	.cfi_offset w24, -64
	cmp	w0, #3
	b.ne	LBB0_25
; %bb.1:
	mov	x19, x1
	ldr	x0, [x1, #8]
	bl	_atoi
	cmp	w0, #1
	b.lt	LBB0_26
; %bb.2:
	ldr	x9, [x19, #16]
	ldrb	w10, [x9]
	cbz	w10, LBB0_26
; %bb.3:
	mov	w11, #0                         ; =0x0
	mov	x8, #0                          ; =0x0
	mov	w12, #16960                     ; =0x4240
	movk	w12, #15, lsl #16
	mov	w13, #1                         ; =0x1
	mov	x14, #1536                      ; =0x600
	movk	x14, #1, lsl #32
	mov	w15, #65536                     ; =0x10000
	mov	w16, #2048                      ; =0x800
	mov	w17, #2                         ; =0x2
	mov	w1, #128                        ; =0x80
	mov	w2, #63                         ; =0x3f
	mov	w3, #55296                      ; =0xd800
LBB0_4:                                 ; =>This Loop Header: Depth=1
                                        ;     Child Loop BB0_7 Depth 2
                                        ;       Child Loop BB0_10 Depth 3
	mov	x5, #0                          ; =0x0
	mov	w6, #0                          ; =0x0
	mov	x4, #0                          ; =0x0
	mov	x19, x10
	mov	x7, x9
	b	LBB0_7
LBB0_5:                                 ;   in Loop: Header=BB0_7 Depth=2
	cbz	w6, LBB0_20
LBB0_6:                                 ;   in Loop: Header=BB0_7 Depth=2
	mov	w6, #1                          ; =0x1
	ldrb	w19, [x7]
	cbz	w19, LBB0_21
LBB0_7:                                 ;   Parent Loop BB0_4 Depth=1
                                        ; =>  This Loop Header: Depth=2
                                        ;       Child Loop BB0_10 Depth 3
	add	x7, x7, #1
	tbz	w19, #7, LBB0_16
; %bb.8:                                ;   in Loop: Header=BB0_7 Depth=2
	sub	w20, w19, #245
	cmn	w20, #52
	b.ls	LBB0_31
; %bb.9:                                ;   in Loop: Header=BB0_7 Depth=2
	and	w20, w19, #0xf0
	cmp	w20, #224
	csel	w20, w16, w15, eq
	cmp	w19, #240
	cinc	w21, w17, hs
	cmp	w19, #223
	csel	w20, w20, w1, hi
	csinc	w21, w21, wzr, hi
	sub	w22, w21, #1
	lsr	w21, w2, w21
	and	w19, w21, w19
	mov	w21, #1                         ; =0x1
LBB0_10:                                ;   Parent Loop BB0_4 Depth=1
                                        ;     Parent Loop BB0_7 Depth=2
                                        ; =>    This Inner Loop Header: Depth=3
	cbz	w21, LBB0_13
; %bb.11:                               ;   in Loop: Header=BB0_10 Depth=3
	mov	x21, x22
	ldrb	w22, [x7], #1
	and	w23, w22, #0xc0
	cmp	w23, #128
	b.ne	LBB0_28
; %bb.12:                               ;   in Loop: Header=BB0_10 Depth=3
	bfi	w22, w19, #6, #26
	mov	x19, x22
	subs	w22, w21, #1
	b.vc	LBB0_10
	b	LBB0_29
LBB0_13:                                ;   in Loop: Header=BB0_7 Depth=2
	cmp	w19, w20
	b.lo	LBB0_32
; %bb.14:                               ;   in Loop: Header=BB0_7 Depth=2
	lsr	w20, w19, #16
	cmp	w20, #16
	b.hi	LBB0_32
; %bb.15:                               ;   in Loop: Header=BB0_7 Depth=2
	and	w20, w19, #0x1ff800
	cmp	w20, w3
	b.eq	LBB0_32
LBB0_16:                                ;   in Loop: Header=BB0_7 Depth=2
	adds	x4, x4, w19, uxtw
	b.vs	LBB0_30
; %bb.17:                               ;   in Loop: Header=BB0_7 Depth=2
	cmp	w19, #32
	b.hi	LBB0_5
; %bb.18:                               ;   in Loop: Header=BB0_7 Depth=2
	mov	w19, w19
	lsl	x19, x13, x19
	tst	x19, x14
	b.eq	LBB0_5
; %bb.19:                               ;   in Loop: Header=BB0_7 Depth=2
	mov	w6, #0                          ; =0x0
	ldrb	w19, [x7]
	cbnz	w19, LBB0_7
	b	LBB0_21
LBB0_20:                                ;   in Loop: Header=BB0_7 Depth=2
	adds	x5, x5, #1
	b.vc	LBB0_6
	b	LBB0_30
LBB0_21:                                ;   in Loop: Header=BB0_4 Depth=1
	mul	x6, x5, x12
	smulh	x5, x5, x12
	cmp	x5, x6, asr #63
	b.ne	LBB0_33
; %bb.22:                               ;   in Loop: Header=BB0_4 Depth=1
	adds	x4, x6, x4
	b.vs	LBB0_30
; %bb.23:                               ;   in Loop: Header=BB0_4 Depth=1
	adds	x8, x8, x4
	b.vs	LBB0_30
; %bb.24:                               ;   in Loop: Header=BB0_4 Depth=1
	add	w11, w11, #1
	cmp	w11, w0
	b.ne	LBB0_4
	b	LBB0_27
LBB0_25:
	mov	w0, #2                          ; =0x2
	ldp	x29, x30, [sp, #64]             ; 16-byte Folded Reload
	ldp	x20, x19, [sp, #48]             ; 16-byte Folded Reload
	ldp	x22, x21, [sp, #32]             ; 16-byte Folded Reload
	ldp	x24, x23, [sp, #16]             ; 16-byte Folded Reload
	add	sp, sp, #80
	ret
LBB0_26:
	mov	x8, #0                          ; =0x0
LBB0_27:
	str	x8, [sp]
Lloh0:
	adrp	x0, l_.str@PAGE
Lloh1:
	add	x0, x0, l_.str@PAGEOFF
	bl	_printf
	mov	w0, #0                          ; =0x0
	ldp	x29, x30, [sp, #64]             ; 16-byte Folded Reload
	ldp	x20, x19, [sp, #48]             ; 16-byte Folded Reload
	ldp	x22, x21, [sp, #32]             ; 16-byte Folded Reload
	ldp	x24, x23, [sp, #16]             ; 16-byte Folded Reload
	add	sp, sp, #80
	ret
LBB0_28:
	bl	_main.cold.1
LBB0_29:
	brk	#0x5515
LBB0_30:
	brk	#0x5500
LBB0_31:
	bl	_main.cold.3
LBB0_32:
	bl	_main.cold.2
LBB0_33:
	brk	#0x550c
	.loh AdrpAdd	Lloh0, Lloh1
	.cfi_endproc
                                        ; -- End function
	.p2align	2                               ; -- Begin function main.cold.1
_main.cold.1:                           ; @main.cold.1
	.cfi_startproc
; %bb.0:
	stp	x29, x30, [sp, #-16]!           ; 16-byte Folded Spill
	mov	x29, sp
	.cfi_def_cfa w29, 16
	.cfi_offset w30, -8
	.cfi_offset w29, -16
	bl	_OUTLINED_FUNCTION_0
	.cfi_endproc
                                        ; -- End function
	.p2align	2                               ; -- Begin function main.cold.2
_main.cold.2:                           ; @main.cold.2
	.cfi_startproc
; %bb.0:
	stp	x29, x30, [sp, #-16]!           ; 16-byte Folded Spill
	mov	x29, sp
	.cfi_def_cfa w29, 16
	.cfi_offset w30, -8
	.cfi_offset w29, -16
	bl	_OUTLINED_FUNCTION_0
	.cfi_endproc
                                        ; -- End function
	.p2align	2                               ; -- Begin function main.cold.3
_main.cold.3:                           ; @main.cold.3
	.cfi_startproc
; %bb.0:
	stp	x29, x30, [sp, #-16]!           ; 16-byte Folded Spill
	mov	x29, sp
	.cfi_def_cfa w29, 16
	.cfi_offset w30, -8
	.cfi_offset w29, -16
	bl	_OUTLINED_FUNCTION_0
	.cfi_endproc
                                        ; -- End function
	.p2align	2                               ; -- Begin function OUTLINED_FUNCTION_0
_OUTLINED_FUNCTION_0:                   ; @OUTLINED_FUNCTION_0 Thunk
	.cfi_startproc
; %bb.0:
	mov	w0, #106                        ; =0x6a
	b	_exit
	.cfi_endproc
                                        ; -- End function
	.section	__TEXT,__cstring,cstring_literals
l_.str:                                 ; @.str
	.asciz	"%lld\n"

.subsections_via_symbols
