//! Mach-O image layout for eager libSystem imports and writable runtime globals.
//!
//! Import slots occupy the first `symbols.len() * 8` bytes of `__DATA.__data`.
//! Another 256 zero bytes follow those slots for private runtime state. With the
//! usual `_dlopen`, `_dlsym` imports, slot 2 (byte offset 16) is the context pointer.
//! This uses ordinary data rather than an indirect-symbol section: the bind
//! opcodes carry the complete symbol names and dylib ordinal.

use super::{align, be32, be64, name, segment, sha256, u32le, u64le};

const PAGE: usize = 16384;
const VM: u64 = 0x100000000;
const COMMANDS: usize = 648;
const RUNTIME_GLOBAL_BYTES: usize = 256;

pub(super) const DYNAMIC_TEXT_OFFSET: usize = 688;

pub(super) fn dynamic_data_offset(text_len: usize) -> usize {
    align(
        DYNAMIC_TEXT_OFFSET
            .checked_add(text_len)
            .expect("dynamic executable text is too large"),
        PAGE,
    )
}

pub(super) fn macho_dynamic(text: &[u8], symbols: &[&str]) -> Vec<u8> {
    let data_off = dynamic_data_offset(text.len());
    let import_bytes = symbols
        .len()
        .checked_mul(8)
        .expect("too many dynamic imports");
    let data_len = import_bytes
        .checked_add(RUNTIME_GLOBAL_BYTES)
        .expect("dynamic executable data is too large");
    let data_size = align(data_len, PAGE);
    let linkedit_off = data_off
        .checked_add(data_size)
        .expect("dynamic executable is too large");

    let mut binds = Vec::new();
    if !symbols.is_empty() {
        // libSystem ordinal 1, pointer type, segment index 2 (__DATA), offset 0.
        binds.extend([0x11, 0x51, 0x72, 0x00]);
        for symbol in symbols {
            assert!(
                !symbol.is_empty() && !symbol.as_bytes().contains(&0),
                "dynamic import names must be nonempty and contain no NUL"
            );
            binds.push(0x40); // SET_SYMBOL_TRAILING_FLAGS_IMM, no weak-import flag
            binds.extend(symbol.as_bytes());
            binds.push(0);
            binds.push(0x90); // DO_BIND also advances the slot address by 8
        }
    }
    binds.push(0x00); // DONE ends the regular (eager) bind stream

    let sig_off = align(
        linkedit_off
            .checked_add(binds.len())
            .expect("dynamic executable bindings are too large"),
        PAGE,
    );
    let slots = sig_off / PAGE;
    let ident = b"dunder\0";
    let hash_off = 0x58 + ident.len();
    let cd_len = hash_off + slots * 32;
    let sig_size = align(20 + cd_len, 16);
    let linkedit_size = sig_off + sig_size - linkedit_off;
    assert!(
        sig_off <= u32::MAX as usize && sig_size <= u32::MAX as usize,
        "dynamic executable exceeds Mach-O file offset range"
    );
    let mut image = Vec::new();
    for n in [
        0xfeedfacf,
        0x0100000c,
        0,
        2,
        10,
        COMMANDS as u32,
        0x00200085, // MH_PIE | MH_TWOLEVEL | MH_DYLDLINK | MH_NOUNDEFS
        0,
    ] {
        u32le(&mut image, n);
    }
    segment(&mut image, "__PAGEZERO", [0, VM, 0, 0], 0, 0);
    segment(
        &mut image,
        "__TEXT",
        [VM, data_off as u64, 0, data_off as u64],
        5,
        1,
    );
    name(&mut image, "__text");
    name(&mut image, "__TEXT");
    u64le(&mut image, VM + DYNAMIC_TEXT_OFFSET as u64);
    u64le(&mut image, text.len() as u64);
    for n in [DYNAMIC_TEXT_OFFSET as u32, 4, 0, 0, 0x80000400, 0, 0, 0] {
        u32le(&mut image, n);
    }
    segment(
        &mut image,
        "__DATA",
        [
            VM + data_off as u64,
            data_size as u64,
            data_off as u64,
            data_size as u64,
        ],
        3,
        1,
    );
    name(&mut image, "__data");
    name(&mut image, "__DATA");
    u64le(&mut image, VM + data_off as u64);
    u64le(&mut image, data_len as u64);
    for n in [data_off as u32, 3, 0, 0, 0, 0, 0, 0] {
        u32le(&mut image, n);
    }
    segment(
        &mut image,
        "__LINKEDIT",
        [
            VM + linkedit_off as u64,
            align(linkedit_size, PAGE) as u64,
            linkedit_off as u64,
            linkedit_size as u64,
        ],
        1,
        0,
    );
    for n in [0x32, 24, 1, 0xb0000, 0xb0000, 0] {
        u32le(&mut image, n);
    }
    for n in [0xe, 32, 12] {
        u32le(&mut image, n);
    }
    let mut dyld = [0u8; 20];
    dyld[..13].copy_from_slice(b"/usr/lib/dyld");
    image.extend(dyld);
    for n in [0x80000028, 24] {
        u32le(&mut image, n);
    }
    u64le(&mut image, DYNAMIC_TEXT_OFFSET as u64);
    u64le(&mut image, 0);
    // LC_LOAD_DYLIB: its position among dylib commands gives library ordinal 1.
    for n in [0xc, 56, 24, 0, 0x10000, 0x10000] {
        u32le(&mut image, n);
    }
    let mut dylib = [0u8; 32];
    dylib[..26].copy_from_slice(b"/usr/lib/libSystem.B.dylib");
    image.extend(dylib);
    for n in [
        0x80000022,
        48,
        0,
        0,
        linkedit_off as u32,
        binds.len() as u32,
        0,
        0,
        0,
        0,
        0,
        0,
    ] {
        u32le(&mut image, n);
    }
    for n in [0x1d, 16, sig_off as u32, sig_size as u32] {
        u32le(&mut image, n);
    }
    assert_eq!(image.len(), 32 + COMMANDS);
    assert_eq!(DYNAMIC_TEXT_OFFSET, align(image.len(), 16));
    image.resize(DYNAMIC_TEXT_OFFSET, 0);
    image.extend(text);
    image.resize(linkedit_off, 0); // text padding, import slots, and runtime globals
    image.extend(binds);
    image.resize(sig_off, 0);

    let mut cd = vec![0u8; cd_len];
    for (at, n) in [
        (0, 0xfade0c02),
        (4, cd_len as u32),
        (8, 0x20400),
        (12, 2),
        (16, hash_off as u32),
        (20, 0x58),
        (28, slots as u32),
        (32, sig_off as u32),
    ] {
        be32(&mut cd, at, n);
    }
    cd[36] = 32;
    cd[37] = 2; // SHA-256
    cd[39] = 14; // 16 KiB signing pages
    be64(&mut cd, 64, 0);
    be64(&mut cd, 72, data_off as u64); // __TEXT.filesize, excluding writable data
    be64(&mut cd, 80, 1); // CS_EXECSEG_MAIN_BINARY
    cd[0x58..hash_off].copy_from_slice(ident);
    for i in 0..slots {
        cd[hash_off + i * 32..hash_off + (i + 1) * 32]
            .copy_from_slice(&sha256(&image[i * PAGE..(i + 1) * PAGE]));
    }
    let mut sig = vec![0u8; 20];
    for (at, n) in [
        (0, 0xfade0cc0),
        (4, sig_size as u32),
        (8, 1),
        (12, 0),
        (16, 20),
    ] {
        be32(&mut sig, at, n);
    }
    sig.extend(cd);
    sig.resize(sig_size, 0);
    image.extend(sig);
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    fn le32(bytes: &[u8], at: usize) -> u32 {
        u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
    }
    fn le64(bytes: &[u8], at: usize) -> u64 {
        u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap())
    }
    fn big32(bytes: &[u8], at: usize) -> usize {
        u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()) as usize
    }
    fn command(image: &[u8], kind: u32) -> usize {
        let mut at = 32;
        for _ in 0..le32(image, 16) {
            if le32(image, at) == kind {
                return at;
            }
            at += le32(image, at + 4) as usize;
        }
        panic!("load command {kind:#x} missing");
    }
    fn data_segment(image: &[u8]) -> usize {
        let mut at = 32;
        while at < 32 + le32(image, 20) as usize {
            if le32(image, at) == 0x19 && &image[at + 8..at + 15] == b"__DATA\0" {
                return at;
            }
            at += le32(image, at + 4) as usize;
        }
        panic!("data segment missing");
    }

    #[test]
    fn eager_bindings_match_import_order_and_leave_writable_globals() {
        let text = 0xd65f03c0u32.to_le_bytes();
        let image = macho_dynamic(&text, &["_dlopen", "_dlsym"]);
        assert_eq!(le32(&image, 16), 10);
        assert_eq!(le32(&image, 20), COMMANDS as u32);
        assert_eq!(&image[DYNAMIC_TEXT_OFFSET..DYNAMIC_TEXT_OFFSET + 4], text);
        let data = data_segment(&image);
        assert_eq!(le64(&image, data + 24), (VM + PAGE as u64));
        assert_eq!(le64(&image, data + 32), PAGE as u64);
        assert_eq!(le64(&image, data + 40), PAGE as u64);
        assert_eq!(le64(&image, data + 48), PAGE as u64);
        assert_eq!(le32(&image, data + 56), 3);
        assert_eq!(le32(&image, data + 60), 3);
        assert_eq!(le64(&image, data + 112), 16 + RUNTIME_GLOBAL_BYTES as u64);
        assert_eq!(le32(&image, data + 136), 0); // S_REGULAR
        assert!(image[PAGE..2 * PAGE].iter().all(|b| *b == 0));
        let info = command(&image, 0x80000022);
        assert_eq!(le32(&image, info + 16), (2 * PAGE) as u32);
        assert_eq!(le32(&image, info + 20), 24);
        assert_eq!(
            &image[2 * PAGE..2 * PAGE + 24],
            b"\x11\x51\x72\x00\x40_dlopen\x00\x90\x40_dlsym\x00\x90\x00"
        );
        let main = command(&image, 0x80000028);
        assert_eq!(le64(&image, main + 8), DYNAMIC_TEXT_OFFSET as u64);
    }

    #[test]
    fn signature_hashes_text_data_and_bindings_but_only_marks_text_executable() {
        let image = macho_dynamic(&vec![0x1f; 2 * PAGE], &["_dlopen", "_dlsym"]);
        let cs = command(&image, 0x1d);
        let sig_off = le32(&image, cs + 8) as usize;
        let sig_size = le32(&image, cs + 12) as usize;
        assert_eq!(image.len(), sig_off + sig_size);
        assert_eq!(sig_off % PAGE, 0);
        let cd = &image[sig_off + 20..];
        assert_eq!(big32(cd, 32), sig_off);
        assert_eq!(big32(cd, 28), sig_off / PAGE);
        assert_eq!(
            u64::from_be_bytes(cd[72..80].try_into().unwrap()),
            dynamic_data_offset(2 * PAGE) as u64
        );
        let hash_off = big32(cd, 16);
        for i in 0..sig_off / PAGE {
            assert_eq!(
                &cd[hash_off + i * 32..hash_off + (i + 1) * 32],
                sha256(&image[i * PAGE..(i + 1) * PAGE])
            );
        }
    }

    #[test]
    fn imports_can_fill_multiple_data_pages_and_empty_imports_keep_globals() {
        let symbols = vec!["_dlopen"; PAGE / 8];
        let image = macho_dynamic(&[], &symbols);
        let data = data_segment(&image);
        assert_eq!(le64(&image, data + 48), (2 * PAGE) as u64);
        assert_eq!(
            le64(&image, data + 112),
            (PAGE + RUNTIME_GLOBAL_BYTES) as u64
        );
        let info = command(&image, 0x80000022);
        assert_eq!(le32(&image, info + 16), (3 * PAGE) as u32);
        let empty = macho_dynamic(&[], &[]);
        let data = data_segment(&empty);
        assert_eq!(le64(&empty, data + 112), RUNTIME_GLOBAL_BYTES as u64);
        let info = command(&empty, 0x80000022);
        let off = le32(&empty, info + 16) as usize;
        assert_eq!(le32(&empty, info + 20), 1);
        assert_eq!(empty[off], 0);
    }
}
