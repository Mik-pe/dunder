#!/usr/bin/env python3
"""Dunder origin tool (dev).

Materialiserar fröets första binär från fröspråkskälla. Detta verktyg är
ENDAST för dokumenterat ursprungsfrö och differentiell verifiering. Det ingår
ALDRIG i Dunders byggväg. Se origin/README.md.

Användning: python3 seedasm.py in.s ut.bin
"""
import sys, struct, hashlib, re

PAGE16K = 0x4000
VM = 0x100000000
SIG_ID_DEFAULT = b'dunder'
CS_ADHOC = 0x2
LC_SEGMENT_64 = 0x19
LC_BUILD_VERSION = 0x32
LC_MAIN = 0x28 | 0x80000000
LC_CODE_SIGNATURE = 0x1D


class AsmError(Exception):
    pass


def err(ln, msg):
    raise AsmError(f'seedasm: rad {ln}: {msg}')


CONDS = {'eq': 0, 'ne': 1, 'cs': 2, 'cc': 3, 'mi': 4, 'pl': 5, 'vs': 6,
         'vc': 7, 'hi': 8, 'ls': 9, 'ge': 10, 'lt': 11, 'gt': 12, 'le': 13,
         'al': 14, 'nv': 15, 'hs': 2, 'lo': 3}
SF = {'x': 1, 'w': 0}


def align(x, n):
    return (x + n - 1) // n * n


def u32(v):
    return struct.pack('<I', v & 0xFFFFFFFF)


def u64(v):
    return struct.pack('<Q', v & 0xFFFFFFFFFFFFFFFF)


def unescape(s, ln):
    out = bytearray()
    i = 0
    while i < len(s):
        c = s[i]
        if c == '\\':
            i += 1
            if i >= len(s):
                err(ln, 'avslutad escape')
            e = s[i]
            m = {'n': 10, 't': 9, 'r': 13, '0': 0, '\\': 92, '"': 34}
            if e in m:
                out.append(m[e])
            elif e == 'x':
                out.append(int(s[i + 1:i + 3], 16))
                i += 2
            else:
                err(ln, f'okänd escape \\{e}')
        else:
            out += c.encode('utf-8')
        i += 1
    return bytes(out)


def parse_reg(t, ln):
    if t in ('xzr', 'wzr'):
        return ('reg', t[0], 31)
    if t == 'sp':
        return ('reg', 'x', 31)
    if len(t) >= 2 and t[0] in 'xw' and t[1:].isdigit():
        n = int(t[1:])
        if 0 <= n <= 30:
            return ('reg', t[0], n)
    err(ln, f'ogiltigt register {t!r}')


def reg_or_zr(t, ln):
    return parse_reg(t, ln)


def is_reg(t):
    if t in ('xzr', 'wzr'):
        return True
    return len(t) >= 2 and t[0] in 'xw' and t[1:].isdigit() and int(t[1:]) <= 31


def parse_imm(t, ln):
    if t.startswith('#'):
        t = t[1:]
    try:
        return int(t, 0)
    except ValueError:
        err(ln, f'ogiltig immediate {t!r}')


def parse_mem(t, ln):
    assert t.startswith('[') and t.endswith(']'), t
    parts = t[1:-1].split()
    if not parts:
        err(ln, 'tom minnesoperand')
    base = parse_reg(parts[0], ln)
    idx = None
    shift = 0
    rest = parts[1:]
    if rest:
        if rest[0].startswith('#'):
            idx = ('imm', parse_imm(rest[0][1:], ln))
            rest = rest[1:]
        elif len(rest) == 1:
            idx = ('reg', *parse_reg(rest[0], ln)[1:])
        elif len(rest) == 3 and rest[1] == 'lsl':
            idx = ('reg', *parse_reg(rest[0], ln)[1:])
            sh = rest[2]
            shift = parse_imm(sh[1:] if sh.startswith('#') else sh, ln)
        else:
            err(ln, f'ogiltig minnesoperand {t!r}')
    return (base, idx, shift)


def mov_imm_words(sf, v, rd):
    """movz/movn/movk-sekvens för godtycklig 64-bitarskonstant."""
    bits = 64 if sf else 32
    if not (-(1 << (bits - 1)) <= v < (1 << bits)):
        raise AsmError(f'immediate {v} utanför intervall')
    if -65536 <= v <= -1:
        inv = (~v) & 0xFFFF
        return [0x12800000 | sf << 31 | inv << 5 | rd]
    if 0 <= v <= 0xFFFF:
        return [0x52800000 | sf << 31 | v << 5 | rd]
    u = v & ((1 << bits) - 1)
    words = [0x52800000 | sf << 31 | (u & 0xFFFF) << 5 | rd]
    rest = u >> 16
    hw = 1
    while rest:
        words.append(0x72800000 | sf << 31 | hw << 21 | (rest & 0xFFFF) << 5 | rd)
        rest >>= 16
        hw += 1
    return words


def enc_plain(ln, mn, ops):
    """Kodar icke-adressberoende instruktioner. Returnerar lista av ord."""
    o = ops
    n = len(ops)

    def reg(i):
        return parse_reg(o[i], ln)

    def imm(i):
        return parse_imm(o[i], ln)

    def shval(i):
        t = o[i]
        return imm(i) if not t.startswith('#') else parse_imm(t[1:], ln)

    if mn == 'nop':
        return [0xD503201F]
    if mn == 'svc':
        return [0xD4000001 | (imm(0) & 0xFFFF) << 5]
    if mn == 'brk':
        return [0xD4200000 | (imm(0) & 0xFFFF) << 5]
    if mn == 'ret':
        return [0xD65F03C0] if n == 0 else [0xD65F0000 | reg(0)[2] << 5]
    if mn == 'br':
        return [0xD61F0000 | reg(0)[2] << 5]
    if mn == 'blr':
        return [0xD63F0000 | reg(0)[2] << 5]

    if mn == 'mov':
        rd = reg(0)
        sf = SF[rd[1]]
        if is_reg(o[1]):
            rm = reg(1)
            b = 0xAA000000 if sf else 0x2A000000
            return [b | rm[2] << 16 | 31 << 5 | rd[2]]
        v = imm(1)
        return mov_imm_words(sf, v, rd[2])

    if mn in ('movz', 'movn', 'movk'):
        rd = reg(0)
        sf = SF[rd[1]]
        v = imm(1)
        hw = 0
        if n >= 4 and o[2] == 'lsl':
            hw = shval(3) // 16
        base = {'movz': 0x52800000, 'movn': 0x12800000, 'movk': 0x72800000}[mn]
        return [base | sf << 31 | hw << 21 | (v & 0xFFFF) << 5 | rd[2]]

    if mn in ('add', 'sub', 'adds', 'subs', 'cmp', 'cmn'):
        if mn in ('cmp', 'cmn'):
            rn = reg(0)
            sf = SF[rn[1]]
            rdv = 31
            src = o[1]
        else:
            rd = reg(0)
            rn = reg(1)
            if SF[rd[1]] != SF[rn[1]]:
                err(ln, 'blandade w/x-register')
            sf = SF[rd[1]]
            rdv = rd[2]
            src = o[2]
        imm_base = {'add': (0x11000000, 0x91000000),
                    'adds': (0x31000000, 0xB1000000),
                    'sub': (0x51000000, 0xD1000000),
                    'subs': (0x71000000, 0xF1000000),
                    'cmp': (0x71000000, 0xF1000000),
                    'cmn': (0x31000000, 0xB1000000)}[mn]
        reg_base = {'add': (0x0B000000, 0x8B000000),
                    'adds': (0x2B000000, 0xAB000000),
                    'sub': (0x4B000000, 0xCB000000),
                    'subs': (0x6B000000, 0xEB000000),
                    'cmp': (0x6B000000, 0xEB000000),
                    'cmn': (0x2B000000, 0xAB000000)}[mn]
        if is_reg(src):
            rm = parse_reg(src, ln)
            if SF[rm[1]] != sf and rm[2] != 31:
                err(ln, 'blandade w/x-register')
            return [reg_base[sf] | rm[2] << 16 | rn[2] << 5 | rdv]
        v = parse_imm(src[1:] if src.startswith('#') else src, ln)
        if -0xFFF <= v < 0:
            flip = {'add': 'sub', 'adds': 'subs', 'cmn': 'cmp',
                    'sub': 'add', 'subs': 'adds', 'cmp': 'cmn'}
            mn2 = flip[mn]
            imm_base2 = {'add': (0x11000000, 0x91000000),
                         'adds': (0x31000000, 0xB1000000),
                         'sub': (0x51000000, 0xD1000000),
                         'subs': (0x71000000, 0xF1000000),
                         'cmp': (0x71000000, 0xF1000000),
                         'cmn': (0x31000000, 0xB1000000)}[mn2]
            return [imm_base2[sf] | (-v) << 10 | rn[2] << 5 | rdv]
        if not (0 <= v <= 0xFFF):
            err(ln, f'add/sub-immediate {v} utanför imm12')
        return [imm_base[sf] | v << 10 | rn[2] << 5 | rdv]

    if mn == 'mul':
        rd, rn, rm = reg(0), reg(1), reg(2)
        sf = SF[rd[1]]
        b = 0x9B007C00 if sf else 0x1B007C00
        return [b | rm[2] << 16 | rn[2] << 5 | rd[2]]
    if mn in ('smulh', 'umulh'):
        rd, rn, rm = reg(0), reg(1), reg(2)
        b = 0x9B407C00 if mn == 'smulh' else 0x9BC07C00
        return [b | rm[2] << 16 | rn[2] << 5 | rd[2]]
    if mn in ('udiv', 'sdiv'):
        rd, rn, rm = reg(0), reg(1), reg(2)
        b = 0x9AC00800 if mn == 'udiv' else 0x9AC00C00
        return [b | rm[2] << 16 | rn[2] << 5 | rd[2]]

    if mn in ('and', 'orr', 'eor'):
        rd, rn, rm = reg(0), reg(1), reg(2)
        sf = SF[rd[1]]
        b = {'and': (0x0A000000, 0x8A000000),
             'orr': (0x2A000000, 0xAA000000),
             'eor': (0x4A000000, 0xCA000000)}[mn][sf]
        return [b | rm[2] << 16 | rn[2] << 5 | rd[2]]

    if mn in ('lsl', 'lsr', 'asr'):
        rd, rn = reg(0), reg(1)
        sf = SF[rd[1]]
        bits = 64 if sf else 32
        if is_reg(o[2]):
            rm = reg(2)
            b = {'lsl': (0x1AC02000, 0x9AC02000),
                 'lsr': (0x1AC02400, 0x9AC02400),
                 'asr': (0x1AC02800, 0x9AC02800)}[mn][sf]
            return [b | rm[2] << 16 | rn[2] << 5 | rd[2]]
        v = shval(2)
        if not (0 <= v < bits):
            err(ln, 'shift utanför intervall')
        if mn == 'lsl':
            if v == 0:
                immr, imms = 0, bits - 1
            else:
                immr, imms = bits - v, bits - 1 - v
            b = 0xD3400000 if sf else 0x53000000
            return [b | immr << 16 | imms << 10 | rn[2] << 5 | rd[2]]
        if mn == 'lsr':
            b = 0xD3400000 if sf else 0x53000000
            return [b | v << 16 | (bits - 1) << 10 | rn[2] << 5 | rd[2]]
        b = 0x93400000 if sf else 0x13000000
        return [b | v << 16 | (bits - 1) << 10 | rn[2] << 5 | rd[2]]

    if mn == 'ror':
        rd, rn = reg(0), reg(1)
        sf = SF[rd[1]]
        bits = 64 if sf else 32
        v = shval(2)
        b = 0x93C00000 if sf else 0x13C00000
        return [b | (v % bits) << 10 | rn[2] << 16 | rn[2] << 5 | rd[2]]

    if mn == 'cset':
        rd = reg(0)
        cond = CONDS[o[1]]
        sf = SF[rd[1]]
        b = 0x9A800400 if sf else 0x1A800400
        return [b | 31 << 16 | (cond ^ 1) << 12 | 31 << 5 | rd[2]]
    if mn == 'csel':
        rd, rn, rm = reg(0), reg(1), reg(2)
        cond = CONDS[o[3]]
        sf = SF[rd[1]]
        b = 0x9A800000 if sf else 0x1A800000
        return [b | rm[2] << 16 | cond << 12 | rn[2] << 5 | rd[2]]

    if mn in ('sxtw', 'uxtw'):
        rd = reg(0)
        rn = reg(1)
        b = 0x93407C00 if mn == 'sxtw' else 0xD3407C00
        return [b | rn[2] << 5 | rd[2]]

    if mn == 'rev':
        rd, rn = reg(0), reg(1)
        sf = SF[rd[1]]
        b = 0xDAC00C00 if sf else 0x5AC00800
        return [b | rn[2] << 5 | rd[2]]

    if mn in ('stp', 'ldp'):
        rt1, rt2 = reg(0), reg(1)
        mem = o[2]
        wb = 'pre' if mem.endswith('!') else ''
        if wb:
            mem = mem[:-1]
        post = len(o) > 3 and o[3].startswith('#')
        base, idx, shift = parse_mem(mem, ln)
        sf = SF[rt1[1]]
        if post:
            if idx is not None:
                err(ln, 'post-index kräver tom klammer')
            idx = ('imm', parse_imm(o[3], ln))
        if idx is None or idx[0] != 'imm':
            err(ln, 'stp/ldp kräver immediate-offset [xN, #imm]')
        scale = 3 if sf else 2
        if idx[1] % (1 << scale):
            err(ln, 'stp/ldp-offset ej justerad')
        v = idx[1] // (1 << scale)
        if not -64 <= v <= 63:
            err(ln, 'stp/ldp-offset utanför intervall')
        key = (wb, post)
        bmap = {
            'stp': {False: {('', False): 0x29000000, ('pre', False): 0x29800000,
                            ('', True): 0x28800000},
                    True: {('', False): 0xA9000000, ('pre', False): 0xA9800000,
                           ('', True): 0xA8800000}},
            'ldp': {False: {('', False): 0x29400000, ('pre', False): 0x29C00000,
                            ('', True): 0x28C00000},
                    True: {('', False): 0xA9400000, ('pre', False): 0xA9C00000,
                           ('', True): 0xA8C00000}},
        }[mn][sf]
        if key not in bmap:
            err(ln, 'ogiltig stp/ldp-adressering')
        b = bmap[key]
        return [b | (v & 0x7F) << 15 | rt2[2] << 10 | base[2] << 5 | rt1[2]]

    if mn in ('ldr', 'str', 'ldrb', 'strb', 'ldrh', 'strh', 'ldrsb', 'ldrsw'):
        rt = reg(0)
        base, idx, shift = parse_mem(o[1], ln)
        wform = rt[1] == 'w'
        if mn in ('ldr', 'str'):
            size = 2 if wform else 3
        elif mn in ('ldrb', 'strb', 'ldrsb'):
            size = 0
        elif mn in ('ldrh', 'strh'):
            size = 1
        else:
            size = 2
        if idx is not None and idx[0] == 'imm':
            v = idx[1]
            scale = size
            if v < 0 or v % (1 << scale):
                err(ln, f'ogiltig offset {v} för {mn}')
            if v >> (scale + 12):
                err(ln, f'offset {v} utanför imm12')
            if mn == 'ldr':
                b = (0xB9400000 if wform else 0xF9400000) | v // (1 << scale) << 10
            elif mn == 'str':
                b = (0xB9000000 if wform else 0xF9000000) | v // (1 << scale) << 10
            elif mn == 'ldrb':
                b = 0x39400000 | v << 10
            elif mn == 'strb':
                b = 0x39000000 | v << 10
            elif mn == 'ldrh':
                b = 0x79400000 | v // 2 << 10
            elif mn == 'strh':
                b = 0x79000000 | v // 2 << 10
            elif mn == 'ldrsb':
                b = (0x39C00000 if wform else 0x39800000) | v << 10
            else:
                b = 0xB9800000 | v // 4 << 10
            return [b | base[2] << 5 | rt[2]]
        if idx is not None and idx[0] == 'reg':
            if idx[1] != 'x':
                err(ln, 'indexregister måste vara x')
            S = 1 if shift else 0
            if shift not in (0, size):
                err(ln, 'endast lsl #elementstorlek stöds')
            wmap = {('ldr', False): 0xF8606800, ('str', False): 0xF8206800,
                    ('ldr', True): 0xB8606800, ('str', True): 0xB8206800,
                    ('ldrb', False): 0x38606800, ('strb', False): 0x38206800}
            if mn in ('ldrb', 'strb'):
                key = (mn, False)
            else:
                key = (mn, wform)
            if key not in wmap:
                err(ln, f'registeroffset stöds inte för {mn}')
            return [wmap[key] | S << 12 | idx[2] << 16 | base[2] << 5 | rt[2]]
        err(ln, 'minnesoperand utan index')

    err(ln, f'okänd instruktion {mn!r}')


def enc_branch_word(mn, ln, ops, target, pc):
    if mn in ('b', 'bl'):
        off = (target - pc) // 4
        if not -(1 << 25) <= off < (1 << 25):
            err(ln, 'hopp utanför räckvidd')
        return (0x14000000 if mn == 'b' else 0x94000000) | off & 0x3FFFFFF
    if mn.startswith('b.'):
        cond = mn[2:]
        if cond not in CONDS:
            err(ln, f'okänt villkor {cond!r}')
        off = (target - pc) // 4
        if not -(1 << 18) <= off < (1 << 18):
            err(ln, 'hopp utanför räckvidd')
        return 0x54000000 | (off & 0x7FFFF) << 5 | CONDS[cond]
    if mn in ('cbz', 'cbnz'):
        rt = parse_reg(ops[0], ln)
        off = (target - pc) // 4
        if not -(1 << 19) <= off < (1 << 19):
            err(ln, 'hopp utanför räckvidd')
        if rt[1] == 'x':
            b = 0xB4000000 if mn == 'cbz' else 0xB5000000
        else:
            b = 0x34000000 if mn == 'cbz' else 0x35000000
        return b | (off & 0xFFFFF) << 5 | rt[2]
    if mn in ('tbz', 'tbnz'):
        rt = parse_reg(ops[0], ln)
        bt = ops[1]
        bit = parse_imm(bt[1:] if bt.startswith('#') else bt, ln)
        off = (target - pc) // 4
        if not -(1 << 13) <= off < (1 << 13):
            err(ln, 'hopp utanför räckvidd')
        b = 0x36000000 if mn == 'tbz' else 0x37000000
        return b | (bit & 0x1F) << 19 | (bit >> 5) << 31 | (off & 0x3FFF) << 5 | rt[2]
    return None


def encode_lea_words(ln, ops, target, pc):
    rd = parse_reg(ops[0], ln)
    if rd[1] != 'x':
        err(ln, 'lea kräver x-register')
    diff_pages = ((target & ~0xFFF) - (pc & ~0xFFF)) >> 12
    if not -(1 << 20) <= diff_pages < (1 << 20):
        err(ln, 'adrp utanför räckvidd')
    adrp = 0x90000000 | (diff_pages & 0x3) << 29 | ((diff_pages >> 2) & 0x7FFFF) << 5 | rd[2]
    add = 0x91000000 | (target & 0xFFF) << 10 | rd[2] << 5 | rd[2]
    return (adrp, add)


def tokenize(src):
    out = []
    for i, raw in enumerate(src.split('\n')):
        line = raw.lstrip()
        if line.startswith('#'):
            line = ''
        else:
            k = 0
            while True:
                k = line.find('#', k)
                if k < 0:
                    break
                if line[k - 1] in ' \t' and (k + 1 == len(line) or line[k + 1] in ' \t'):
                    line = line[:k]
                    break
                k += 1
        line = line.strip()
        if not line:
            continue
        if line.strip().endswith(':') and ' ' not in line and ',' not in line:
            out.append((i + 1, 'label', line[:-1]))
            continue
        m = re.match(r'^([A-Za-z_.$][A-Za-z0-9_.$]*)\s*:\s+(.*)$', line)
        if m:
            out.append((i + 1, 'label', m.group(1)))
            line = m.group(2).strip()
            if not line:
                continue
        toks = [t for t in line.replace(',', ' , ').replace('[', ' [ ')
                .replace(']', ' ] ').split() if t != ',']
        merged = []
        j = 0
        while j < len(toks):
            t = toks[j]
            if t == '[':
                buf = []
                j += 1
                while j < len(toks) and toks[j] != ']':
                    buf.append(toks[j])
                    j += 1
                if j >= len(toks):
                    err(i + 1, 'oavslutad minnesoperand')
                j += 1
                mem = '[' + ' '.join(buf) + ']'
                if j < len(toks) and toks[j] == '!':
                    mem += '!'
                    j += 1
                merged.append(mem)
            else:
                merged.append(t)
                j += 1
        mn = merged[0]
        if mn.startswith('.'):
            out.append((i + 1, 'dir', mn, merged[1:]))
        else:
            out.append((i + 1, 'ins', mn, merged[1:]))
    return out


def assemble(src):
    items = tokenize(src)
    sections = {'text': [], 'const': [], 'data': [], 'bss': []}
    labels = {}
    cur = 'text'
    entry = None
    ident = SIG_ID_DEFAULT

    for it in items:
        ln, kind = it[0], it[1]
        if kind == 'label':
            name = it[2]
            if name in labels:
                err(ln, f'dubblettetikett {name!r}')
            labels[name] = (cur, sum(s for _, s, _ in sections[cur]))
            continue
        if kind == 'dir':
            mn, args = it[2], it[3]
            if mn in ('.text', '.const', '.data', '.bss'):
                cur = mn[1:]
                continue
            if mn == '.entry':
                entry = args[0]
                continue
            if mn == '.ident':
                ident = args[0].strip('"').encode()
                continue
            if mn == '.space':
                v = parse_imm(args[0], ln)
                if cur == 'bss':
                    sections[cur].append(('bytes', v, b''))
                else:
                    sections[cur].append(('bytes', v, b'\x00' * v))
                continue
            if mn == '.align':
                a = parse_imm(args[0], ln)
                off = sum(s for _, s in sections[cur])
                pad = align(off, a) - off
                sections[cur].append(('bytes', pad, b'\x00' * pad))
                continue
            if mn in ('.u8', '.u16', '.u32', '.u64'):
                vals = [parse_imm(a, ln) for a in args]
                szb = int(mn[2:])
                b = b''.join(struct.pack('<Q', v) [:szb] for v in vals)
                sections[cur].append(('bytes', len(b), b))
                continue
            if mn in ('.str', '.strz'):
                s = ' '.join(args)
                if not (s.startswith('"') and s.endswith('"')):
                    err(ln, 'str kräver "..."')
                b = unescape(s[1:-1], ln)
                if mn == '.strz':
                    b += b'\x00'
                sections[cur].append(('bytes', len(b), b))
                continue
            err(ln, f'okänt direktiv {mn!r}')
            continue
        # instruktion
        mn, ops = it[2], it[3]
        if mn in ('b', 'bl') or mn.startswith('b.') or mn in ('cbz', 'cbnz', 'tbz', 'tbnz'):
            sections[cur].append(('ins', 4, (ln, mn, ops, 'br')))
            continue
        if mn == 'lea':
            sections[cur].append(('ins', 8, (ln, mn, ops, 'lea')))
            continue
        words = enc_plain(ln, mn, ops)
        sections[cur].append(('ins', 4 * len(words), (ln, mn, words, 'plain')))

    if entry is None:
        err(0, '.entry saknas')
    if entry not in labels:
        err(0, f'.entry-etiketten {entry!r} saknas')

    text_items = sections['text']
    const_items = sections['const']
    data_items = sections['data']
    bss_items = sections['bss']

    const_bytes = b''.join(b for _, _, b in const_items)
    data_bytes = b''.join(b for _, _, b in data_items)
    bss_size = sum(s for _, s, _ in bss_items)
    text_size = sum(s for _, s, _ in text_items)

    has_const = len(const_bytes) > 0
    has_data = len(data_bytes) > 0
    has_bss = bss_size > 0
    nsects_text = 1 + (1 if has_const else 0)
    has_data_seg = has_data or has_bss
    cmds_size = cmds_size_for(nsects_text, has_data_seg)
    text_off = align(32 + cmds_size, 16)
    text_end = text_off + text_size
    const_off = align(text_end, 16) if has_const else 0
    const_end = const_off + len(const_bytes) if has_const else text_end
    data_off = align(const_end, PAGE16K)
    bss_off_in_seg = align(len(data_bytes), 16)
    bss_base = VM + data_off + bss_off_in_seg

    addrs = {}
    for name, (sec, off) in labels.items():
        if sec == 'text':
            addrs[name] = VM + text_off + off
        elif sec == 'const':
            addrs[name] = VM + const_off + off
        elif sec == 'data':
            addrs[name] = VM + data_off + off
        else:
            addrs[name] = VM + data_off + bss_off_in_seg + off

    entry_off = addrs[entry] - VM

    text = bytearray()
    pc = VM + text_off
    for kind, size, payload in text_items:
        if kind == 'ins':
            ln, mn = payload[0], payload[1]
            if payload[3] == 'plain':
                for w in payload[2]:
                    text += u32(w)
                    pc += 4
            elif payload[3] == 'br':
                mnem = payload[1]
                if mnem in ('cbz', 'cbnz'):
                    t = payload[2][1]
                elif mnem in ('tbz', 'tbnz'):
                    t = payload[2][2]
                else:
                    t = payload[2][0]
                if t not in addrs:
                    err(ln, f'okänd hoppmål {t!r}')
                text += u32(enc_branch_word(mn, ln, payload[2], addrs[t], pc))
                pc += 4
            else:
                t = payload[2][1]
                if t not in addrs:
                    err(ln, f'okänd etikett {t!r}')
                a, b2 = encode_lea_words(ln, payload[2], addrs[t], pc)
                text += u32(a) + u32(b2)
                pc += 8
        else:
            text += kind
            pc += size

    return build_macho(bytes(text), const_bytes, data_bytes, bss_size,
                       entry_off, ident, bss_off_in_seg)


def sect64(seg, name, addr, size, off, typ=0):
    return struct.pack('<16s16sQQIIIIIIII', name, seg, addr, size, off,
                       4, 0, 0, typ, 0, 0, 0)[:80]


def cmds_size_for(nsects_text, has_data_seg):
    return (72 + 72 + 80 * nsects_text + (72 + 160 if has_data_seg else 0)
            + 72 + 24 + 32 + 24 + 16)


def build_macho(text, const, data, bss_size, entry_off, ident, bss_off_in_seg):
    has_const = len(const) > 0
    has_data = len(data) > 0
    has_bss = bss_size > 0
    has_data_seg = has_data or has_bss
    nsects_text = 1 + (1 if has_const else 0)
    cmds_size = cmds_size_for(nsects_text, has_data_seg)
    text_off = align(32 + cmds_size, 16)
    text_end = text_off + len(text)
    const_off = align(text_end, 16) if has_const else 0
    const_end = const_off + len(const) if has_const else text_end
    data_off = align(const_end, PAGE16K)
    data_vmsize = align(len(data) + bss_off_in_seg + bss_size, PAGE16K)
    linkedit_off = data_off + data_vmsize
    sig_off = linkedit_off
    nslots = (sig_off + 16383) // 16384
    ident_bytes = ident + b'\x00'
    hash_off = 0x58 + len(ident_bytes)
    cd_len = hash_off + 32 * nslots
    datasize = align(20 + cd_len, 16)

    c = b''
    c += u32(LC_SEGMENT_64) + u32(72) + struct.pack('<16s', b'__PAGEZERO')
    c += u64(0) + u64(VM) + u64(0) + u64(0)
    c += u32(0) + u32(0) + u32(0) + u32(0)
    c += u32(LC_SEGMENT_64) + u32(72 + 80 * nsects_text) + struct.pack('<16s', b'__TEXT')
    c += u64(VM) + u64(align(const_end, PAGE16K)) + u64(0) + u64(const_end)
    c += u32(5) + u32(5) + u32(nsects_text) + u32(0)
    c += sect64(b'__TEXT', b'__text', VM + text_off, len(text), text_off)
    if has_const:
        c += sect64(b'__TEXT', b'__const', VM + const_off, len(const), const_off)
    if has_data_seg:
        dsize = align(len(data) + bss_off_in_seg + bss_size, PAGE16K)
        c += u32(LC_SEGMENT_64) + u32(72 + 160) + struct.pack('<16s', b'__DATA')
        c += u64(VM + data_off) + u64(dsize) + u64(data_off) + u64(len(data))
        c += u32(3) + u32(3) + u32(2) + u32(0)
        c += sect64(b'__DATA', b'__data', VM + data_off, len(data), data_off)
        c += sect64(b'__DATA', b'__bss', VM + data_off + bss_off_in_seg,
                    bss_size, 0, typ=1)
    c += u32(LC_SEGMENT_64) + u32(72) + struct.pack('<16s', b'__LINKEDIT')
    c += u64(VM + linkedit_off) + u64(align(datasize, PAGE16K)) + u64(linkedit_off) + u64(datasize)
    c += u32(1) + u32(1) + u32(0) + u32(0)
    c += u32(LC_BUILD_VERSION) + u32(24) + u32(1) + u32(0xB0000) + u32(0xB0000) + u32(0)
    c += u32(0xE) + u32(32) + u32(12) + struct.pack('<20s', b'/usr/lib/dyld\x00\x00\x00\x00\x00\x00\x00')
    c += u32(LC_MAIN) + u32(24) + u64(entry_off) + u64(0)
    c += u32(LC_CODE_SIGNATURE) + u32(16) + u32(sig_off) + u32(datasize)
    assert len(c) == cmds_size, (len(c), cmds_size)

    ncmds = 7 + (1 if has_data_seg else 0)
    hdr = struct.pack('<IiiIIIII', 0xFEEDFACF, 0x0100000C, 0, 2, ncmds,
                      cmds_size, 0x00200005, 0)
    img = bytearray(hdr + c)
    img += b'\x00' * (text_off - len(img)) + text
    if has_const:
        img += b'\x00' * (const_off - len(img)) + const
    img += b'\x00' * (sig_off - len(img))

    codeLimit = sig_off
    ident_off = 0x58
    cd = bytearray(cd_len)
    struct.pack_into('>III', cd, 0, 0xFade0C02, cd_len, 0x20400)
    struct.pack_into('>I', cd, 12, CS_ADHOC)
    struct.pack_into('>I', cd, 16, hash_off)
    struct.pack_into('>I', cd, 20, ident_off)
    struct.pack_into('>I', cd, 28, nslots)
    struct.pack_into('>I', cd, 32, codeLimit)
    cd[36] = 32
    cd[37] = 2
    cd[39] = 14
    text_vmsize = align(const_end, PAGE16K)
    struct.pack_into('>QQQ', cd, 64, 0, text_vmsize, 1)
    cd[ident_off:ident_off + len(ident_bytes)] = ident_bytes
    for i in range(nslots):
        pg = bytes(img[i * 16384:(i + 1) * 16384]).ljust(16384, b'\x00')
        cd[hash_off + 32 * i:hash_off + 32 * (i + 1)] = hashlib.sha256(pg).digest()
    sig = struct.pack('>III', 0xFade0CC0, 0, 1) + struct.pack('>II', 0, 20) + bytes(cd)
    datasize = align(len(sig), 16)
    sig += b'\x00' * (datasize - len(sig))
    struct.pack_into('>I', sig_sb := bytearray(sig), 4, datasize)
    sig = bytes(sig_sb)
    assert datasize == align(20 + cd_len, 16)
    return bytes(img) + sig


def main():
    if len(sys.argv) != 3:
        print('användning: seedasm.py in.s ut.bin', file=sys.stderr)
        return 2
    src = open(sys.argv[1], 'r', encoding='utf-8').read()
    try:
        img = assemble(src)
    except AsmError as e:
        print(str(e), file=sys.stderr)
        return 1
    with open(sys.argv[2], 'wb') as f:
        f.write(img)
    import os
    os.chmod(sys.argv[2], 0o755)
    return 0


if __name__ == '__main__':
    sys.exit(main())
