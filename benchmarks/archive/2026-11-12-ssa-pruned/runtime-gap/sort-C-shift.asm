// selected feasible static instruction path; no execution/frequency/timing claim
// successful insertion shift with next j>0
0000000100000550	ldur	x2, [x1, #-0x8]
0000000100000554	cmp	x2, x16
0000000100000558	b.le	0x100000524
000000010000055c	str	x2, [x1], #-0x8
0000000100000560	sub	x17, x17, #0x1
0000000100000564	cmp	x17, #0x1
0000000100000568	b.gt	0x100000550
