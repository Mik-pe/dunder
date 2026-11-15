	.build_version macos, 27, 2	sdk_version 27, 2
	.section	__TEXT,__text,regular,pure_instructions
	.globl	_main                           ; -- Begin function main
	.p2align	2
_main:                                  ; @main
	.cfi_startproc
; %bb.0:
	sub	sp, sp, #64
	stp	x22, x21, [sp, #16]             ; 16-byte Folded Spill
	stp	x20, x19, [sp, #32]             ; 16-byte Folded Spill
	stp	x29, x30, [sp, #48]             ; 16-byte Folded Spill
	add	x29, sp, #48
	.cfi_def_cfa w29, 16
	.cfi_offset w30, -8
	.cfi_offset w29, -16
	.cfi_offset w19, -24
	.cfi_offset w20, -32
	.cfi_offset w21, -40
	.cfi_offset w22, -48
	cmp	w0, #3
	b.ne	LBB0_20
; %bb.1:
	ldr	x0, [x1, #8]
	mov	x19, x1
	bl	_atoi
	mov	x20, x0
	ldr	x0, [x19, #16]
	mov	x1, #0                          ; =0x0
	mov	w2, #10                         ; =0xa
	bl	_strtoll
	mov	x19, x0
	mov	w0, #2048                       ; =0x800
	bl	_malloc
	cbz	x0, LBB0_25
; %bb.2:
	cmp	w20, #1
	b.lt	LBB0_21
; %bb.3:
	mov	x8, #0                          ; =0x0
	mov	x21, #0                         ; =0x0
	add	x9, x0, #8
	mov	x10, #195                       ; =0xc3
	movk	x10, #22733, lsl #16
	movk	x10, #8898, lsl #32
	movk	x10, #33255, lsl #48
	mov	w11, #1009                      ; =0x3f1
	mov	w12, w20
LBB0_4:                                 ; =>This Loop Header: Depth=1
                                        ;     Child Loop BB0_5 Depth 2
                                        ;     Child Loop BB0_11 Depth 2
                                        ;       Child Loop BB0_12 Depth 3
                                        ;     Child Loop BB0_16 Depth 2
	mov	x13, x0
	mov	w14, #4335                      ; =0x10ef
LBB0_5:                                 ;   Parent Loop BB0_4 Depth=1
                                        ; =>  This Inner Loop Header: Depth=2
	adds	x15, x14, x19
	b.vs	LBB0_23
; %bb.6:                                ;   in Loop: Header=BB0_5 Depth=2
	adds	x15, x15, x8
	b.vs	LBB0_23
; %bb.7:                                ;   in Loop: Header=BB0_5 Depth=2
	smulh	x16, x15, x10
	add	x16, x16, x15
	asr	x17, x16, #9
	add	x16, x17, x16, lsr #63
	msub	x15, x16, x11, x15
	str	x15, [x13], #8
	sub	x14, x14, #17
	cmn	x14, #17
	b.ne	LBB0_5
; %bb.8:                                ;   in Loop: Header=BB0_4 Depth=1
	mov	x13, x9
	mov	w14, #2                         ; =0x2
	mov	w15, #1                         ; =0x1
	b	LBB0_11
LBB0_9:                                 ;   in Loop: Header=BB0_11 Depth=2
	sub	x17, x17, #1
LBB0_10:                                ;   in Loop: Header=BB0_11 Depth=2
	sbfiz	x17, x17, #3, #32
	str	x16, [x0, x17]
	add	x15, x15, #1
	add	x14, x14, #1
	add	x13, x13, #8
	cmp	x15, #256
	b.eq	LBB0_15
LBB0_11:                                ;   Parent Loop BB0_4 Depth=1
                                        ; =>  This Loop Header: Depth=2
                                        ;       Child Loop BB0_12 Depth 3
	ldr	x16, [x0, x15, lsl #3]
	mov	x1, x13
	mov	x17, x14
LBB0_12:                                ;   Parent Loop BB0_4 Depth=1
                                        ;     Parent Loop BB0_11 Depth=2
                                        ; =>    This Inner Loop Header: Depth=3
	ldur	x2, [x1, #-8]
	cmp	x2, x16
	b.le	LBB0_9
; %bb.13:                               ;   in Loop: Header=BB0_12 Depth=3
	str	x2, [x1], #-8
	sub	x17, x17, #1
	cmp	x17, #1
	b.gt	LBB0_12
; %bb.14:                               ;   in Loop: Header=BB0_11 Depth=2
	mov	x17, #0                         ; =0x0
	b	LBB0_10
LBB0_15:                                ;   in Loop: Header=BB0_4 Depth=1
	mov	x13, x0
	mov	w14, #1                         ; =0x1
LBB0_16:                                ;   Parent Loop BB0_4 Depth=1
                                        ; =>  This Inner Loop Header: Depth=2
	ldr	x16, [x13], #8
	mul	x15, x16, x14
	smulh	x16, x16, x14
	cmp	x16, x15, asr #63
	b.ne	LBB0_24
; %bb.17:                               ;   in Loop: Header=BB0_16 Depth=2
	adds	x21, x21, x15
	b.vs	LBB0_23
; %bb.18:                               ;   in Loop: Header=BB0_16 Depth=2
	add	x14, x14, #1
	cmp	x14, #257
	b.ne	LBB0_16
; %bb.19:                               ;   in Loop: Header=BB0_4 Depth=1
	add	x8, x8, #1
	cmp	x8, x12
	b.ne	LBB0_4
	b	LBB0_22
LBB0_20:
	mov	w0, #2                          ; =0x2
	ldp	x29, x30, [sp, #48]             ; 16-byte Folded Reload
	ldp	x20, x19, [sp, #32]             ; 16-byte Folded Reload
	ldp	x22, x21, [sp, #16]             ; 16-byte Folded Reload
	add	sp, sp, #64
	ret
LBB0_21:
	mov	x21, #0                         ; =0x0
LBB0_22:
	bl	_free
	str	x21, [sp]
Lloh0:
	adrp	x0, l_.str@PAGE
Lloh1:
	add	x0, x0, l_.str@PAGEOFF
	bl	_printf
	mov	w0, #0                          ; =0x0
	ldp	x29, x30, [sp, #48]             ; 16-byte Folded Reload
	ldp	x20, x19, [sp, #32]             ; 16-byte Folded Reload
	ldp	x22, x21, [sp, #16]             ; 16-byte Folded Reload
	add	sp, sp, #64
	ret
LBB0_23:
	brk	#0x5500
LBB0_24:
	brk	#0x550c
LBB0_25:
	bl	_main.cold.1
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
	mov	w0, #109                        ; =0x6d
	bl	_exit
	.cfi_endproc
                                        ; -- End function
	.section	__TEXT,__cstring,cstring_literals
l_.str:                                 ; @.str
	.asciz	"%lld\n"

.subsections_via_symbols
