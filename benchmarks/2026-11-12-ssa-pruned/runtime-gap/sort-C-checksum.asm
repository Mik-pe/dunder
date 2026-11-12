// selected feasible static instruction path; no execution/frequency/timing claim
// checksum loop has another element and all checked arithmetic succeeds
000000010000057c	ldr	x16, [x13], #0x8
0000000100000580	mul	x15, x16, x14
0000000100000584	smulh	x16, x16, x14
0000000100000588	cmp	x16, x15, asr #63
000000010000058c	b.ne	0x100000600
0000000100000590	adds	x21, x21, x15
0000000100000594	b.vs	0x1000005fc
0000000100000598	add	x14, x14, #0x1
000000010000059c	cmp	x14, #0x101
00000001000005a0	b.ne	0x10000057c
