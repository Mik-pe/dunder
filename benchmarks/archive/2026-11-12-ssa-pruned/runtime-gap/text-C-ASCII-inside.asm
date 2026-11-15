// selected feasible static instruction path; no execution/frequency/timing claim
// ASCII byte>32, already inside word, nonzero byte, checked sum does not overflow
00000001000004f4	ldrb	w19, [x7]
00000001000004f8	cbz	w19, 0x1000005c8
00000001000004fc	add	x7, x7, #0x1
0000000100000500	tbz	w19, #0x7, 0x10000058c
000000010000058c	adds	x4, x4, w19, uxtw
0000000100000590	b.vs	0x10000064c
0000000100000594	cmp	w19, #0x20
0000000100000598	b.hi	0x1000004ec
00000001000004ec	cbz	w6, 0x1000005bc
00000001000004f0	mov	w6, #0x1
