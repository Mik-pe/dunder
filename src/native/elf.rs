//! Self-contained, statically linked ELF images. The image has no interpreter,
//! dynamic dependencies, relocations, or executable stack requirement.

pub(crate) const TEXT_OFFSET: usize = 4096;
const BASE: u64 = 0x400000;

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
