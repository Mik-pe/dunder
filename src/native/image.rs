//! Mach-O image layout for eager libSystem imports and writable runtime globals.
//!
//! Import slots occupy the first `symbols.len() * 8` bytes of `__DATA.__data`.
//! The requested writable static storage and callback context follow the slots.
//! This uses ordinary data rather than an indirect-symbol section: the bind
//! opcodes carry the complete symbol names and dylib ordinal.

use super::{align, be32, be64, name, segment, sha256, u32le, u64le};

const PAGE: usize = 16384;
const VM: u64 = 0x100000000;
const COMMANDS: usize = 648;
#[cfg(test)]
const TEST_GLOBAL_BYTES: usize = 256;

pub(super) const LIBSYSTEM: &str = "/usr/lib/libSystem.B.dylib";
#[cfg(test)]
const DYNAMIC_TEXT_OFFSET: usize = 688;

fn dylib_command_size(name: &str) -> usize {
    align(24 + name.len() + 1, 8)
}

fn commands_size(imports: &super::ffi::Imports) -> usize {
    COMMANDS - 56
        + imports
            .libraries
            .iter()
            .map(|lib| dylib_command_size(lib))
            .sum::<usize>()
}

pub(super) fn text_offset(imports: &super::ffi::Imports) -> usize {
    align(32 + commands_size(imports), 16)
}

pub(super) fn dynamic_data_offset(text_offset: usize, text_len: usize) -> usize {
    align(
        text_offset
            .checked_add(text_len)
            .expect("dynamic executable text is too large"),
        PAGE,
    )
}

/// Bind external bundle metadata to an already emitted, directly signed image.
/// The existing page-aligned signature offset stays fixed, so text and data
/// fixups do not move. Only the final signature and its load-command extents
/// change. The bundle publisher owns the exact bytes of both external files.
pub(super) fn sign_bundle(
    image: &mut Vec<u8>,
    identifier: &str,
    info: &[u8],
    resources: &[u8],
) -> Result<(), String> {
    let malformed = || "cannot sign bundle: malformed generated Mach-O image".to_string();
    let read32 = |at: usize| -> Result<u32, String> {
        Ok(u32::from_le_bytes(
            image
                .get(at..at.checked_add(4).ok_or_else(malformed)?)
                .ok_or_else(malformed)?
                .try_into()
                .map_err(|_| malformed())?,
        ))
    };
    let read64 = |at: usize| -> Result<u64, String> {
        Ok(u64::from_le_bytes(
            image
                .get(at..at.checked_add(8).ok_or_else(malformed)?)
                .ok_or_else(malformed)?
                .try_into()
                .map_err(|_| malformed())?,
        ))
    };
    if identifier.is_empty() || identifier.as_bytes().contains(&0) {
        return Err("cannot sign bundle: identifier must be nonempty and contain no NUL".into());
    }
    if image.len() < 32
        || read32(0)? != 0xfeedfacf
        || read32(4)? != 0x0100000c
        || read32(8)? != 0
        || read32(12)? != 2
    {
        return Err(malformed());
    }
    // Match the two layouts emitted by our writers. The dynamic writer emits
    // one LC_LOAD_DYLIB for each library imported by the program.
    let count = read32(16)? as usize;
    let (prefix, dylibs, suffix): (&[u32], usize, &[u32]) = match read32(24)? {
        0x00200005 if count == 7 && read32(20)? == 392 => {
            (&[0x19, 0x19, 0x19, 0x32, 0xe, 0x80000028], 0, &[0x1d])
        }
        0x00200085 if count >= 10 => (
            &[0x19, 0x19, 0x19, 0x19, 0x32, 0xe, 0x80000028],
            count - 9,
            &[0x80000022, 0x1d],
        ),
        _ => return Err("cannot sign bundle: unsupported generated image format".into()),
    };
    let commands_end = 32usize
        .checked_add(read32(20)? as usize)
        .ok_or_else(malformed)?;
    if commands_end > image.len() {
        return Err(malformed());
    }
    let mut at = 32;
    let mut signature = None;
    let mut linkedit = None;
    let mut text = None;
    let expected = prefix
        .iter()
        .copied()
        .chain(std::iter::repeat_n(0xc, dylibs))
        .chain(suffix.iter().copied());
    for expected_kind in expected {
        if at + 8 > commands_end {
            return Err(malformed());
        }
        let kind = read32(at)?;
        let size = read32(at + 4)? as usize;
        let end = at.checked_add(size).ok_or_else(malformed)?;
        if kind != expected_kind || size < 8 || !size.is_multiple_of(8) || end > commands_end {
            return Err(malformed());
        }
        if kind == 0xc {
            if size < 32 || read32(at + 8)? != 24 || !image[at + 24..end].contains(&0) {
                return Err(malformed());
            }
        } else if kind == 0x1d {
            if size != 16 || signature.is_some() {
                return Err(malformed());
            }
            signature = Some((at, read32(at + 8)? as usize, read32(at + 12)? as usize));
        } else if kind == 0x19 {
            if size < 72 {
                return Err(malformed());
            }
            let name = &image[at + 8..at + 24];
            if name.starts_with(b"__LINKEDIT\0") {
                if linkedit.is_some() {
                    return Err(malformed());
                }
                linkedit = Some((at, read64(at + 40)?));
            } else if name.starts_with(b"__TEXT\0") {
                if text.is_some() {
                    return Err(malformed());
                }
                text = Some((read64(at + 40)?, read64(at + 48)?));
            }
        }
        at = end;
    }
    let (sig_cmd, sig_off, old_sig_size) = signature.ok_or_else(malformed)?;
    let (linkedit_cmd, linkedit_off) = linkedit.ok_or_else(malformed)?;
    let (exec_base, exec_size) = text.ok_or_else(malformed)?;
    if at != commands_end
        || sig_off < commands_end
        || sig_off % PAGE != 0
        || sig_off.checked_add(old_sig_size) != Some(image.len())
        || linkedit_off > sig_off as u64
        || exec_base
            .checked_add(exec_size)
            .is_none_or(|end| end > sig_off as u64)
    {
        return Err(malformed());
    }
    let old_signature = &image[sig_off..];
    let old_be32 = |at: usize| -> Result<usize, String> {
        Ok(u32::from_be_bytes(
            old_signature
                .get(at..at.checked_add(4).ok_or_else(malformed)?)
                .ok_or_else(malformed)?
                .try_into()
                .map_err(|_| malformed())?,
        ) as usize)
    };
    if old_sig_size < 20 + 0x58
        || old_be32(0)? != 0xfade0cc0
        || old_be32(4)? != old_sig_size
        || old_be32(8)? != 1
        || old_be32(12)? != 0
        || old_be32(16)? != 20
        || old_be32(20)? != 0xfade0c02
        || old_be32(24)? < 0x58
        || old_be32(24)? > old_sig_size - 20
        || old_be32(28)? != 0x20400
        || old_be32(32)? != 2
        || old_be32(52)? != sig_off
        || old_signature[56] != 32
        || old_signature[57] != 2
        || old_signature[59] != 14
    {
        return Err("cannot sign bundle: unsupported generated signature format".into());
    }

    let slots = sig_off / PAGE;
    let ident_end = 0x58usize
        .checked_add(identifier.len())
        .and_then(|end| end.checked_add(1))
        .ok_or_else(malformed)?;
    let hash_off = ident_end.checked_add(3 * 32).ok_or_else(malformed)?;
    let cd_len = slots
        .checked_mul(32)
        .and_then(|len| len.checked_add(hash_off))
        .ok_or_else(malformed)?;
    let sig_size = cd_len
        .checked_add(20 + 15)
        .map(|len| len & !15)
        .filter(|len| *len <= u32::MAX as usize)
        .ok_or_else(malformed)?;
    let linkedit_size = (sig_off as u64)
        .checked_add(sig_size as u64)
        .and_then(|end| end.checked_sub(linkedit_off))
        .ok_or_else(malformed)?;
    let linkedit_vm_size = linkedit_size
        .checked_add(PAGE as u64 - 1)
        .map(|size| size & !(PAGE as u64 - 1))
        .ok_or_else(malformed)?;

    image[sig_cmd + 12..sig_cmd + 16].copy_from_slice(&(sig_size as u32).to_le_bytes());
    image[linkedit_cmd + 32..linkedit_cmd + 40].copy_from_slice(&linkedit_vm_size.to_le_bytes());
    image[linkedit_cmd + 48..linkedit_cmd + 56].copy_from_slice(&linkedit_size.to_le_bytes());
    image.truncate(sig_off);
    let mut cd = vec![0u8; cd_len];
    for (offset, value) in [
        (0, 0xfade0c02),
        (4, cd_len as u32),
        (8, 0x20400),
        (12, 2), // CS_ADHOC
        (16, hash_off as u32),
        (20, 0x58),
        (24, 3), // Info.plist, absent requirements, CodeResources
        (28, slots as u32),
        (32, sig_off as u32),
    ] {
        be32(&mut cd, offset, value);
    }
    cd[36] = 32;
    cd[37] = 2;
    cd[39] = 14;
    be64(&mut cd, 64, exec_base);
    be64(&mut cd, 72, exec_size);
    be64(&mut cd, 80, 1);
    cd[0x58..ident_end - 1].copy_from_slice(identifier.as_bytes());
    // Special slots count backwards from hashOffset. An all-zero slot denotes
    // an absent component; requirements (-2) are optional for ad hoc signing.
    cd[hash_off - 32..hash_off].copy_from_slice(&sha256(info));
    cd[hash_off - 96..hash_off - 64].copy_from_slice(&sha256(resources));
    for i in 0..slots {
        cd[hash_off + i * 32..hash_off + (i + 1) * 32]
            .copy_from_slice(&sha256(&image[i * PAGE..(i + 1) * PAGE]));
    }
    let mut signature = vec![0u8; 20];
    for (offset, value) in [
        (0, 0xfade0cc0),
        (4, sig_size as u32),
        (8, 1),
        (12, 0),
        (16, 20),
    ] {
        be32(&mut signature, offset, value);
    }
    signature.extend(cd);
    signature.resize(sig_size, 0);
    image.extend(signature);
    Ok(())
}

#[cfg(test)]
fn macho_imports(text: &[u8], imports: &super::ffi::Imports) -> Vec<u8> {
    macho_imports_with_storage(text, imports, TEST_GLOBAL_BYTES)
}

pub(super) fn macho_imports_with_storage(
    text: &[u8],
    imports: &super::ffi::Imports,
    global_bytes: usize,
) -> Vec<u8> {
    let symbols = &imports.symbols;
    let text_offset = text_offset(imports);
    let commands = commands_size(imports);
    let data_off = dynamic_data_offset(text_offset, text.len());
    let import_bytes = symbols
        .len()
        .checked_mul(8)
        .expect("too many dynamic imports");
    let data_len = import_bytes
        .checked_add(global_bytes)
        .expect("dynamic executable data is too large");
    let data_size = align(data_len, PAGE);
    let linkedit_off = data_off
        .checked_add(data_size)
        .expect("dynamic executable is too large");

    let mut binds = Vec::new();
    if !symbols.is_empty() {
        // libSystem ordinal 1, pointer type, segment index 2 (__DATA), offset 0.
        binds.extend([0x11, 0x51, 0x72, 0x00]);
        let mut previous = 0;
        for (library, symbol) in symbols {
            if *library != previous {
                let ordinal = library + 1;
                if ordinal < 16 {
                    binds.push(0x10 | ordinal as u8);
                } else {
                    binds.push(0x20); // SET_DYLIB_ORDINAL_ULEB
                    let mut value = ordinal;
                    while value >= 128 {
                        binds.push(value as u8 | 0x80);
                        value >>= 7;
                    }
                    binds.push(value as u8);
                }
                previous = *library;
            }
            assert!(
                !symbol.is_empty() && !symbol.as_bytes().contains(&0),
                "dynamic import names must be nonempty and contain no NUL"
            );
            binds.push(0x40); // SET_SYMBOL_TRAILING_FLAGS_IMM, no weak-import flag
            binds.push(b'_');
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
        (9 + imports.libraries.len()) as u32,
        commands as u32,
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
    u64le(&mut image, VM + text_offset as u64);
    u64le(&mut image, text.len() as u64);
    for n in [text_offset as u32, 4, 0, 0, 0x80000400, 0, 0, 0] {
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
    u64le(&mut image, text_offset as u64);
    u64le(&mut image, 0);
    for library in &imports.libraries {
        let size = dylib_command_size(library);
        let start = image.len();
        for n in [0xc, size as u32, 24, 0, 0x10000, 0x10000] {
            u32le(&mut image, n);
        }
        image.extend(library.as_bytes());
        image.resize(start + size, 0);
    }
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
    assert_eq!(image.len(), 32 + commands);
    assert_eq!(text_offset, align(image.len(), 16));
    image.resize(text_offset, 0);
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
fn macho_dynamic(text: &[u8], symbols: &[&str]) -> Vec<u8> {
    let imports = super::ffi::Imports {
        libraries: vec![LIBSYSTEM.into()],
        symbols: symbols
            .iter()
            .map(|name| (0, name.strip_prefix('_').unwrap().to_owned()))
            .collect(),
    };
    macho_imports(text, &imports)
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
        assert_eq!(le64(&image, data + 112), 16 + TEST_GLOBAL_BYTES as u64);
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
    fn long_dylib_command_lists_move_text_and_bind_high_ordinals() {
        let mut imports = super::super::ffi::Imports::default();
        imports.libraries.push(LIBSYSTEM.into());
        for i in 1..131 {
            imports
                .libraries
                .push(format!("/tmp/{}/{i}.dylib", "a".repeat(128)));
        }
        imports.symbols.push((130, "last".into()));
        let text = 0xd65f03c0u32.to_le_bytes();
        let image = macho_imports(&text, &imports);
        let entry = command(&image, 0x80000028);
        let off = le64(&image, entry + 8) as usize;
        assert!(off > PAGE);
        assert_eq!(image[off..off + 4], text);
        let info = command(&image, 0x80000022);
        let binds = le32(&image, info + 16) as usize;
        assert_eq!(&image[binds + 4..binds + 7], &[0x20, 0x83, 0x01]);
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
            dynamic_data_offset(DYNAMIC_TEXT_OFFSET, 2 * PAGE) as u64
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
        assert_eq!(le64(&image, data + 112), (PAGE + TEST_GLOBAL_BYTES) as u64);
        let info = command(&image, 0x80000022);
        assert_eq!(le32(&image, info + 16), (3 * PAGE) as u32);
        let empty = macho_dynamic(&[], &[]);
        let data = data_segment(&empty);
        assert_eq!(le64(&empty, data + 112), TEST_GLOBAL_BYTES as u64);
        let info = command(&empty, 0x80000022);
        let off = le32(&empty, info + 16) as usize;
        assert_eq!(le32(&empty, info + 20), 1);
        assert_eq!(empty[off], 0);
    }

    #[test]
    fn bundle_signature_binds_metadata_and_updated_load_commands_deterministically() {
        let mut image = macho_dynamic(&[0; 32], &["_dlopen", "_dlsym"]);
        let info = b"exact Info.plist bytes";
        let resources = b"exact CodeResources bytes";
        sign_bundle(&mut image, "ai.dunder.probe", info, resources).unwrap();
        let cs = command(&image, 0x1d);
        let sig_off = le32(&image, cs + 8) as usize;
        assert_eq!(image.len(), sig_off + le32(&image, cs + 12) as usize);
        let cd = &image[sig_off + 20..];
        assert_eq!(big32(cd, 24), 3);
        let hash_off = big32(cd, 16);
        assert_eq!(&cd[hash_off - 32..hash_off], sha256(info));
        assert_eq!(&cd[hash_off - 64..hash_off - 32], &[0u8; 32]);
        assert_eq!(&cd[hash_off - 96..hash_off - 64], sha256(resources));
        assert_eq!(&cd[hash_off..hash_off + 32], sha256(&image[..PAGE]));
        assert_eq!(
            u64::from_be_bytes(cd[72..80].try_into().unwrap()),
            PAGE as u64
        );
        let sealed = image.clone();
        sign_bundle(&mut image, "ai.dunder.probe", info, resources).unwrap();
        assert_eq!(image, sealed);
        sign_bundle(&mut image, "ai.dunder.probe", b"changed plist", resources).unwrap();
        assert_ne!(image, sealed);
    }

    #[test]
    fn malformed_bundle_inputs_fail_before_mutating_image() {
        let source = macho_dynamic(&[], &[]);
        for len in [0, 4, 31, 32, 200, source.len() - 1] {
            let mut image = source[..len].to_vec();
            let before = image.clone();
            assert!(sign_bundle(&mut image, "ai.dunder.probe", &[], &[]).is_err());
            assert_eq!(image, before);
        }
        let cs = command(&source, 0x1d);
        for (offset, value) in [(4, 0), (20, u32::MAX), (36, 0), (cs + 8, 16), (cs + 12, 1)] {
            let mut image = source.clone();
            image[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            let before = image.clone();
            assert!(sign_bundle(&mut image, "ai.dunder.probe", &[], &[]).is_err());
            assert_eq!(image, before);
        }
        let mut image = source;
        let before = image.clone();
        assert!(sign_bundle(&mut image, "", &[], &[]).is_err());
        assert_eq!(image, before);
    }
}
