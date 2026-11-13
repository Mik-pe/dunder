#!/usr/bin/env python3
"""Jämför två Mach-O-bilder och pekar ut första skillnaden per region.
Utvecklingsstöd för att felsöka frösjälvbygge; ingår ej i byggvägen."""
import sys, struct

def regions(b):
    n = struct.unpack_from('<I', b, 16)[0]
    out = [('header', 0, 32)]
    p = 32
    for _ in range(n):
        cmd, cmdsize = struct.unpack_from('<II', b, p)
        names = {0x19: 'segment', 0x32: 'build_version', 0xe: 'dylinker',
                 0x28 | 0x80000000: 'main', 0x1d: 'code_signature'}
        out.append((names.get(cmd, f'cmd_{cmd:#x}'), p, p + cmdsize))
        p += cmdsize
    # hitta text och signatur från kommandon
    text_off = sig_off = None
    p = 32
    for _ in range(n):
        cmd, cmdsize = struct.unpack_from('<II', b, p)
        if cmd == 0x1d:
            sig_off, sig_size = struct.unpack_from('<II', b, p + 8)
        if cmd == 0x19 and b[p+8:p+24].rstrip(b'\0') == b'__TEXT':
            nsects = struct.unpack_from('<I', b, p + 64)[0]
            if nsects:
                text_off = struct.unpack_from('<I', b, p + 72 + 8)[0]
        p += cmdsize
    if text_off is not None and sig_off is not None:
        out.append(('text', text_off, sig_off))
        out.append(('signatur', sig_off, len(b)))
    return out

def main():
    a = open(sys.argv[1], 'rb').read()
    b = open(sys.argv[2], 'rb').read()
    if a == b:
        print('identiska')
        return 0
    print(f'storlek: {len(a)} vs {len(b)}')
    ra, rb = regions(a), regions(b)
    for (na, sa, ea), (nb, sb, eb) in zip(ra, rb):
        if a[sa:ea] != b[sb:eb]:
            print(f'region som skiljer: {na} [{sa}:{ea}]')
            for i in range(min(ea - sa, eb - sb)):
                if a[sa + i] != b[sb + i]:
                    print(f'  första skillnad vid {na}+{i} (filoffset {sa + i}): '
                          f'{a[sa + i]:02x} vs {b[sb + i]:02x}')
                    print(f'  kontext a: {a[sa + i - 8:sa + i + 8].hex()}')
                    print(f'  kontext b: {b[sb + i - 8:sb + i + 8].hex()}')
                    break
    n = min(len(a), len(b))
    for i in range(n):
        if a[i] != b[i]:
            print(f'globalt första byteskillnad: offset {i} ({a[i]:02x} vs {b[i]:02x})')
            break
    return 1

if __name__ == '__main__':
    sys.exit(main())
