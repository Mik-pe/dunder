//! Direct ELF64 layout, with optional eager native-library imports.
//! Ordinary programs retain their static image without an interpreter.

pub(crate) const TEXT_OFFSET: usize = 4096;
const BASE: u64 = 0x400000;

pub(crate) fn dynamic_data_offset(text_len: usize) -> usize {
    (TEXT_OFFSET + text_len).next_multiple_of(4096)
}

/// Native imports occupy the first eight-byte cells of the writable segment.
/// Subsequent cells are process globals owned by the emitted runtime. The ELF
/// loader resolves every import before entering the generated program.
pub(crate) fn dynamic_executable(
    text: &[u8],
    machine: u16,
    symbols: &[&str],
    global_bytes: usize,
) -> Vec<u8> {
    let interpreter = match machine {
        62 => b"/lib64/ld-linux-x86-64.so.2\0".as_slice(),
        183 => b"/lib/ld-linux-aarch64.so.1\0".as_slice(),
        _ => panic!("unsupported dynamic ELF machine"),
    };
    let data = dynamic_data_offset(text.len());
    let mut image = vec![0; data + global_bytes.max(symbols.len() * 8).next_multiple_of(8)];
    image[TEXT_OFFSET..TEXT_OFFSET + text.len()].copy_from_slice(text);
    let mut strings = vec![0];
    let libc = strings.len();
    strings.extend_from_slice(b"libc.so.6\0");
    let libdl = strings.len();
    strings.extend_from_slice(b"libdl.so.2\0");
    let mut names = Vec::new();
    for name in symbols {
        assert!(!name.is_empty() && !name.as_bytes().contains(&0));
        names.push(strings.len());
        strings.extend_from_slice(name.as_bytes());
        strings.push(0);
    }
    let string_table = image.len();
    image.extend_from_slice(&strings);
    image.resize(image.len().next_multiple_of(8), 0);
    let symbol_table = image.len();
    image.resize(symbol_table + (symbols.len() + 1) * 24, 0);
    for (i, name) in names.iter().enumerate() {
        let at = symbol_table + (i + 1) * 24;
        put32(&mut image, at, *name as u32);
        image[at + 4] = 0x12; // STB_GLOBAL | STT_FUNC, undefined.
    }
    let hash_table = image.len();
    image.resize(hash_table + (4 + symbols.len()) * 4, 0);
    put32(&mut image, hash_table, 1);
    put32(&mut image, hash_table + 4, (symbols.len() + 1) as u32);
    if !symbols.is_empty() {
        put32(&mut image, hash_table + 8, 1);
    }
    for i in 1..symbols.len() {
        put32(&mut image, hash_table + 12 + i * 4, (i + 1) as u32);
    }
    image.resize(image.len().next_multiple_of(8), 0);
    let relocations = image.len();
    image.resize(relocations + symbols.len() * 24, 0);
    for i in 0..symbols.len() {
        put64(
            &mut image,
            relocations + i * 24,
            BASE + (data + i * 8) as u64,
        );
        let kind = if machine == 62 { 6 } else { 1025 }; // GLOB_DAT.
        put64(
            &mut image,
            relocations + i * 24 + 8,
            ((i + 1) as u64) << 32 | kind,
        );
    }
    let dynamic = image.len();
    let entries = [
        (1, libc as u64),
        (1, libdl as u64),
        (4, BASE + hash_table as u64),
        (5, BASE + string_table as u64),
        (10, strings.len() as u64),
        (6, BASE + symbol_table as u64),
        (11, 24),
        (7, BASE + relocations as u64),
        (8, (symbols.len() * 24) as u64),
        (9, 24),
        (24, 0), // DT_BIND_NOW: no lazy resolution or PLT trampoline.
        (0, 0),
    ];
    for (tag, value) in entries {
        image.extend_from_slice(&u64::to_le_bytes(tag));
        image.extend_from_slice(&u64::to_le_bytes(value));
    }
    image[..16].copy_from_slice(b"\x7fELF\x02\x01\x01\0\0\0\0\0\0\0\0\0");
    put16(&mut image, 16, 2);
    put16(&mut image, 18, machine);
    put32(&mut image, 20, 1);
    put64(&mut image, 24, BASE + TEXT_OFFSET as u64);
    put64(&mut image, 32, 64);
    put16(&mut image, 52, 64);
    put16(&mut image, 54, 56);
    put16(&mut image, 56, 6);
    const INTERPRETER: usize = 512;
    image[INTERPRETER..INTERPRETER + interpreter.len()].copy_from_slice(interpreter);
    let length = image.len();
    for (i, (kind, flags, offset, size, alignment)) in [
        (6, 4, 64, 6 * 56, 8), // PT_PHDR.
        (3, 4, INTERPRETER, interpreter.len(), 1),
        (1, 5, 0, data, 4096), // Read/execute image headers and text.
        (1, 6, data, length - data, 4096),
        (2, 6, dynamic, entries.len() * 16, 8),
        (0x6474e551, 6, 0, 0, 16), // PT_GNU_STACK, not executable.
    ]
    .into_iter()
    .enumerate()
    {
        let at = 64 + i * 56;
        put32(&mut image, at, kind);
        put32(&mut image, at + 4, flags);
        put64(&mut image, at + 8, offset as u64);
        put64(&mut image, at + 16, BASE + offset as u64);
        put64(&mut image, at + 24, BASE + offset as u64);
        put64(&mut image, at + 32, size as u64);
        put64(&mut image, at + 40, size as u64);
        put64(&mut image, at + 48, alignment);
    }
    image
}

pub(crate) fn executable(text: &[u8], machine: u16) -> Vec<u8> {
    let mut image = vec![0; TEXT_OFFSET];
    image[..16].copy_from_slice(b"\x7fELF\x02\x01\x01\0\0\0\0\0\0\0\0\0");
    put16(&mut image, 16, 2); // ET_EXEC
    put16(&mut image, 18, machine);
    put32(&mut image, 20, 1);
    put64(&mut image, 24, BASE + TEXT_OFFSET as u64);
    put64(&mut image, 32, 64); // program header offset
    put16(&mut image, 52, 64); // ELF header size
    put16(&mut image, 54, 56); // program header size
    put16(&mut image, 56, 2);
    let size = (TEXT_OFFSET + text.len()) as u64;
    put32(&mut image, 64, 1); // PT_LOAD
    put32(&mut image, 68, 5); // PF_R | PF_X
    put64(&mut image, 72, 0);
    put64(&mut image, 80, BASE);
    put64(&mut image, 88, BASE);
    put64(&mut image, 96, size);
    put64(&mut image, 104, size);
    put64(&mut image, 112, 4096);
    put32(&mut image, 120, 0x6474e551); // PT_GNU_STACK
    put32(&mut image, 124, 6); // PF_R | PF_W (no executable stack)
    put64(&mut image, 168, 16);
    image.extend_from_slice(text);
    image
}

fn put16(image: &mut [u8], at: usize, value: u16) {
    image[at..at + 2].copy_from_slice(&value.to_le_bytes());
}
fn put32(image: &mut [u8], at: usize, value: u32) {
    image[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn put64(image: &mut [u8], at: usize, value: u64) {
    image[at..at + 8].copy_from_slice(&value.to_le_bytes());
}
