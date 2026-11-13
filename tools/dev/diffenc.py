#!/usr/bin/env python3
"""Differentiell kodarverifiering: frökodare vs clang-assemblerad referens.
Referensen är endast ett verifieringsverktyg, aldrig del av Dunders byggväg."""
import subprocess, sys, os, re, tempfile

DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, DIR)
import seedasm

CASES = [
    ('mov x0, 42', 'mov x0, #42'),
    ('mov x0, 65535', 'mov x0, #65535'),
    ('mov x0, -1', 'mov x0, #-1'),
    ('mov x0, -65536', 'mov x0, #-65536'),
    ('mov x3, x7', 'mov x3, x7'),
    ('mov w2, w9', 'mov w2, w9'),
    ('mov w2, 300', 'mov w2, #300'),
    ('mov x1, xzr', 'mov x1, xzr'),
    ('movz x5, 0x1234, lsl 16', 'movz x5, #0x1234, lsl #16'),
    ('movn x5, 0x1234', 'movn x5, #0x1234'),
    ('movk x5, 0x99, lsl 32', 'movk x5, #0x99, lsl #32'),
    ('add x0, x1, x2', 'add x0, x1, x2'),
    ('add x0, x1, 4095', 'add x0, x1, #4095'),
    ('add w0, w1, w2', 'add w0, w1, w2'),
    ('sub x3, x4, x5', 'sub x3, x4, x5'),
    ('sub x3, x4, 16', 'sub x3, x4, #16'),
    ('sub x3, x4, -16', 'sub x3, x4, #-16'),
    ('adds x0, x1, x2', 'adds x0, x1, x2'),
    ('adds x0, x1, 5', 'adds x0, x1, #5'),
    ('subs x0, x1, x2', 'subs x0, x1, x2'),
    ('cmp x0, x1', 'cmp x0, x1'),
    ('cmp x0, 100', 'cmp x0, #100'),
    ('cmp w0, w1', 'cmp w0, w1'),
    ('cmn x0, x1', 'cmn x0, x1'),
    ('cmn x0, 7', 'cmn x0, #7'),
    ('mul x0, x1, x2', 'mul x0, x1, x2'),
    ('mul w0, w1, w2', 'mul w0, w1, w2'),
    ('smulh x0, x1, x2', 'smulh x0, x1, x2'),
    ('umulh x0, x1, x2', 'umulh x0, x1, x2'),
    ('udiv x0, x1, x2', 'udiv x0, x1, x2'),
    ('sdiv x0, x1, x2', 'sdiv x0, x1, x2'),
    ('and x0, x1, x2', 'and x0, x1, x2'),
    ('orr x0, x1, x2', 'orr x0, x1, x2'),
    ('eor x0, x1, x2', 'eor x0, x1, x2'),
    ('and w0, w1, w2', 'and w0, w1, w2'),
    ('lsl x0, x1, x2', 'lsl x0, x1, x2'),
    ('lsr x0, x1, x2', 'lsr x0, x1, x2'),
    ('asr x0, x1, x2', 'asr x0, x1, x2'),
    ('lsl w0, w1, w2', 'lsl w0, w1, w2'),
    ('lsl x0, x1, 5', 'lsl x0, x1, #5'),
    ('lsr x0, x1, 5', 'lsr x0, x1, #5'),
    ('asr x0, x1, 5', 'asr x0, x1, #5'),
    ('lsl w0, w1, 5', 'lsl w0, w1, #5'),
    ('lsr w0, w1, 5', 'lsr w0, w1, #5'),
    ('asr w0, w1, 5', 'asr w0, w1, #5'),
    ('lsl x0, x1, 0', 'lsl x0, x1, #0'),
    ('ror x0, x1, 12', 'ror x0, x1, #12'),
    ('cset x0, eq', 'cset x0, eq'),
    ('cset x0, lt', 'cset x0, lt'),
    ('cset w0, ne', 'cset w0, ne'),
    ('csel x0, x1, x2, ge', 'csel x0, x1, x2, ge'),
    ('csel w0, w1, w2, hi', 'csel w0, w1, w2, hi'),
    ('sxtw x0, w1', 'sxtw x0, w1'),
    ('uxtw x0, w1', 'uxtw x0, w1'),
    ('rev x0, x1', 'rev x0, x1'),
    ('rev w0, w1', 'rev w0, w1'),
    ('ldr x0, [x1, 8]', 'ldr x0, [x1, #8]'),
    ('ldr x0, [x1]', 'ldr x0, [x1, #0]'),
    ('str x0, [x1, 32760]', 'str x0, [x1, #32760]'),
    ('ldr w0, [x1, 4]', 'ldr w0, [x1, #4]'),
    ('str w0, [x1, 8]', 'str w0, [x1, #8]'),
    ('ldrb w0, [x1, 3]', 'ldrb w0, [x1, #3]'),
    ('strb w0, [x1, 255]', 'strb w0, [x1, #255]'),
    ('ldrh w0, [x1, 2]', 'ldrh w0, [x1, #2]'),
    ('strh w0, [x1, 6]', 'strh w0, [x1, #6]'),
    ('ldrsb x0, [x1, 3]', 'ldrsb x0, [x1, #3]'),
    ('ldrsb w0, [x1, 3]', 'ldrsb w0, [x1, #3]'),
    ('ldrsw x0, [x1, 4]', 'ldrsw x0, [x1, #4]'),
    ('ldr x0, [x1, x2]', 'ldr x0, [x1, x2]'),
    ('ldr x0, [x1, x2, lsl 3]', 'ldr x0, [x1, x2, lsl #3]'),
    ('str x0, [x1, x2]', 'str x0, [x1, x2]'),
    ('ldr w0, [x1, x2]', 'ldr w0, [x1, x2]'),
    ('ldrb w0, [x1, x2]', 'ldrb w0, [x1, x2]'),
    ('strb w0, [x1, x2]', 'strb w0, [x1, x2]'),
    ('stp x19, x20, [sp, -16]', 'stp x19, x20, [sp, #-16]'),
    ('stp x19, x20, [sp, 16]', 'stp x19, x20, [sp, #16]'),
    ('ldp x19, x20, [sp, 48]', 'ldp x19, x20, [sp, #48]'),
    ('stp w0, w1, [x2, -8]', 'stp w0, w1, [x2, #-8]'),
    ('ldp w0, w1, [x2, 8]', 'ldp w0, w1, [x2, #8]'),
    ('stp x29, x30, [sp, -16]!', 'stp x29, x30, [sp, #-16]!'),
    ('ldp x29, x30, [sp, 16]!', 'ldp x29, x30, [sp, #16]!'),
    ('stp x29, x30, [sp], 16', 'stp x29, x30, [sp], #16'),
    ('ldp x29, x30, [sp], 16', 'ldp x29, x30, [sp], #16'),
    ('ldp w29, w30, [sp], 8', 'ldp w29, w30, [sp], #8'),
    ('nop', 'nop'),
    ('brk 1', 'brk #1'),
    ('ret', 'ret'),
    ('ret x5', 'ret x5'),
    ('br x5', 'br x5'),
    ('blr x5', 'blr x5'),
    ('ldr x0, [sp, 16]', 'ldr x0, [sp, #16]'),
    ('str x0, [sp, 32]', 'str x0, [sp, #32]'),
]

BRANCHES = [
    ('b.eq tgt', 'b.eq tgt'),
    ('b.ne tgt', 'b.ne tgt'),
    ('b.lt tgt', 'b.lt tgt'),
    ('b.ge tgt', 'b.ge tgt'),
    ('b.hi tgt', 'b.hi tgt'),
    ('b.ls tgt', 'b.ls tgt'),
    ('b.hs tgt', 'b.hs tgt'),
    ('b.lo tgt', 'b.lo tgt'),
    ('cbz x0, tgt', 'cbz x0, tgt'),
    ('cbnz x0, tgt', 'cbnz x0, tgt'),
    ('cbz w3, tgt', 'cbz w3, tgt'),
    ('cbnz w5, tgt', 'cbnz w5, tgt'),
    ('tbz x0, 3, tgt', 'tbz x0, 3, tgt'),
    ('tbnz x0, 31, tgt', 'tbnz x0, 31, tgt'),
    ('tbz w2, 0, tgt', 'tbz w2, 0, tgt'),
    ('tbnz w7, 12, tgt', 'tbnz w7, 12, tgt'),
    ('b tgt', 'b tgt'),
    ('bl tgt', 'bl tgt'),
]

MULTI = [
    (['movz x0, #0x5678', 'movk x0, #0x1234, lsl #16'],
     'mov x0, #0x12345678'),
    (['movz x0, #0xdef0', 'movk x0, #0x9abc, lsl #16',
      'movk x0, #0x5678, lsl #32', 'movk x0, #0x1234, lsl #48'],
     'mov x0, #0x123456789abcdef0'),
    (['movz w0, #0x5678', 'movk w0, #0x1234, lsl #16'],
     'mov w0, #0x12345678'),
]


def main():
    tmp = tempfile.mkdtemp()
    clang_src = ['.text,0x80\n' if False else '.text\n']
    seed_src = ['.text', '.entry _start']
    ln = 1
    lines_map = []
    for i, (clang_syntax, seed_syntax) in enumerate(CASES):
        clang_src.append(f'99{i}: {clang_syntax}\n')
        seed_src.append(f'L{i}: {seed_syntax}')
    for j, (clang_lines, seed_syntax) in enumerate(MULTI):
        for k, cl in enumerate(clang_lines):
            clang_src.append(f'm{j}_{k}: {cl}\n')
        seed_src.append(f'M{j}: {seed_syntax}')
    for j, (cb, sb) in enumerate(BRANCHES):
        clang_src.append(f'b{j}: {cb}\n')
        clang_src.append('nop\n')
        clang_src.append('nop\n')
        seed_src.append(f'BR{j}: {sb}')
        seed_src.append('nop')
        seed_src.append('nop')
    clang_src.append('tgt:\n')
    seed_src.append('tgt:')
    seed_src.append('_start: mov x0, #0')
    seed_src.append('ret')
    open(f'{tmp}/ref.s', 'w').write('\n'.join(clang_src))
    open(f'{tmp}/seed.s', 'w').write('\n'.join(seed_src) + '\n')
    r = subprocess.run(['clang', '-c', f'{tmp}/ref.s', '-o', f'{tmp}/ref.o'],
                       capture_output=True, text=True)
    if r.returncode:
        print('clang kunde inte assemblera referensen:', r.stderr)
        return 1
    r = subprocess.run(['objdump', '-d', f'{tmp}/ref.o'], capture_output=True, text=True)
    ref_words = []
    for line in r.stdout.split('\n'):
        m = re.match(r'^\s*[0-9a-f]+:\s+([0-9a-f]{8})\s', line)
        if m:
            ref_words.append(int(m.group(1), 16))
    img = seedasm.assemble('\n'.join(seed_src) + '\n')
    import struct
    entryoff = None
    p = 32
    ncmds = struct.unpack_from('<I', img, 16)[0]
    for _ in range(ncmds):
        cmd, cmdsize = struct.unpack_from('<II', img, p)
        if cmd == (0x28 | 0x80000000):
            entryoff = struct.unpack_from('<Q', img, p + 8)[0]
        p += cmdsize
    assert entryoff is not None
    n0 = len(ref_words)
    code = img[entryoff - 4 * n0:entryoff]
    seed_words = [struct.unpack_from('<I', code, 4 * k)[0] for k in range(n0)]
    bad = 0
    ri = 0
    for cs, ss in CASES:
        if ref_words[ri] != seed_words[ri]:
            print(f'SKILLNAD: {ss!r}: ref={ref_words[ri]:08x} seed={seed_words[ri]:08x}')
            bad += 1
        ri += 1
    for j, (clang_lines, ss) in enumerate(MULTI):
        for k, cl in enumerate(clang_lines):
            if ref_words[ri] != seed_words[ri]:
                print(f'SKILLNAD(multi {j}.{k}): ref={ref_words[ri]:08x} seed={seed_words[ri]:08x}')
                bad += 1
            ri += 1
    for j, (cb, sb) in enumerate(BRANCHES):
        if ref_words[ri] != seed_words[ri]:
            print(f'SKILLNAD(branch {j} {sb!r}): ref={ref_words[ri]:08x} seed={seed_words[ri]:08x}')
            bad += 1
        ri += 1
        if ref_words[ri] != seed_words[ri]:
            print(f'SKILLNAD(branch filler1 {j}): ref={ref_words[ri]:08x} seed={seed_words[ri]:08x}')
            bad += 1
        ri += 1
        if ref_words[ri] != seed_words[ri]:
            print(f'SKILLNAD(branch filler2 {j}): ref={ref_words[ri]:08x} seed={seed_words[ri]:08x}')
            bad += 1
        ri += 1
    assert ri == len(ref_words), (ri, len(ref_words))
    print(f'{len(ref_words)} instruktionsord jämförda, {bad} skillnader')
    return 1 if bad else 0


if __name__ == '__main__':
    sys.exit(main())
