.text
.globl _abi_probe
.p2align 2
_abi_probe:
stp x29, x30, [sp, #-176]!
mov x29, sp
stp x19, x20, [sp, #16]
stp x21, x22, [sp, #32]
stp x23, x24, [sp, #48]
stp x25, x26, [sp, #64]
stp x27, x28, [sp, #80]
stp d8, d9, [sp, #96]
stp d10, d11, [sp, #112]
stp d12, d13, [sp, #128]
stp d14, d15, [sp, #144]
str x0, [sp, #160]
str x1, [sp, #168]
mov x16, x0
mov x0, x1
mov x19, #19
mov x20, #20
mov x21, #21
mov x22, #22
mov x23, #23
mov x24, #24
mov x25, #25
mov x26, #26
mov x27, #27
mov x28, #28
mov x10, #2008
fmov d8, x10
mov x10, #2009
fmov d9, x10
mov x10, #2010
fmov d10, x10
mov x10, #2011
fmov d11, x10
mov x10, #2012
fmov d12, x10
mov x10, #2013
fmov d13, x10
mov x10, #2014
fmov d14, x10
mov x10, #2015
fmov d15, x10
blr x16
cbnz x0, Lbad
mov x10, sp
cmp x29, x10
b.ne Lbad
and x10, x10, #15
cbnz x10, Lbad
cmp x19, #19
b.ne Lbad
cmp x20, #20
b.ne Lbad
cmp x21, #21
b.ne Lbad
cmp x22, #22
b.ne Lbad
cmp x23, #23
b.ne Lbad
cmp x24, #24
b.ne Lbad
cmp x25, #25
b.ne Lbad
cmp x26, #26
b.ne Lbad
cmp x27, #27
b.ne Lbad
cmp x28, #28
b.ne Lbad
fmov x10, d8
cmp x10, #2008
b.ne Lbad
fmov x10, d9
cmp x10, #2009
b.ne Lbad
fmov x10, d10
cmp x10, #2010
b.ne Lbad
fmov x10, d11
cmp x10, #2011
b.ne Lbad
fmov x10, d12
cmp x10, #2012
b.ne Lbad
fmov x10, d13
cmp x10, #2013
b.ne Lbad
fmov x10, d14
cmp x10, #2014
b.ne Lbad
fmov x10, d15
cmp x10, #2015
b.ne Lbad
mov x0, #1
b Ldone
Lbad:
mov x0, #0
Ldone:
ldp d8, d9, [sp, #96]
ldp d10, d11, [sp, #112]
ldp d12, d13, [sp, #128]
ldp d14, d15, [sp, #144]
ldp x19, x20, [sp, #16]
ldp x21, x22, [sp, #32]
ldp x23, x24, [sp, #48]
ldp x25, x26, [sp, #64]
ldp x27, x28, [sp, #80]
ldp x29, x30, [sp], #176
ret
