//! Linux process entry and the syscall ABI boundary for native x86-64 output.
use super::Lower;

impl Lower {
    /// Keep the backend's private register bank and value stack separate from
    /// the x86 call stack. This is compile-time register spilling: no opcodes,
    /// instruction pointer, dispatch loop or emulator are shipped in the image.
    pub(super) fn prelude(&mut self) {
        const STACK_BYTES: u64 = 64 * 1024 * 1024;
        self.emit(&[0x49, 0x89, 0xe6]); // mov r14,rsp: original argc/argv
        self.imm64(0, 9); // mmap(NULL, size, READ|WRITE, PRIVATE|ANON, -1, 0)
        self.imm64(7, 0);
        self.imm64(6, STACK_BYTES);
        self.imm64(2, 3);
        self.imm64(10, 0x22);
        self.imm64(8, u64::MAX);
        self.imm64(9, 0);
        self.emit(&[0x0f, 0x05]);
        self.emit(&[0x48, 0x85, 0xc0]); // test rax,rax
        self.emit(&[0x0f, 0x89]); // jns allocated
        let allocated = self.code.len();
        self.disp32(0);
        self.syscall_fatal();
        self.patch_local(allocated, self.code.len());
        self.emit(&[0x49, 0x89, 0xc7]); // mov r15,rax

        // A guard page separates the spilled register bank from the value
        // stack: exhausting that stack must fault before corrupting state.
        self.imm64(0, 10); // mprotect(base + 4096, 4096, PROT_NONE)
        self.emit(&[0x49, 0x8d, 0xbf]);
        self.disp32(4096);
        self.imm64(6, 4096);
        self.imm64(2, 0);
        self.emit(&[0x0f, 0x05, 0x48, 0x85, 0xc0]);
        self.emit(&[0x0f, 0x89]);
        let guarded = self.code.len();
        self.disp32(0);
        self.syscall_fatal();
        self.patch_local(guarded, self.code.len());

        // Copy argc, argv pointers and argv's null terminator. Pointed-to
        // strings retain their kernel-provided lifetime on the original stack.
        self.emit(&[0x49, 0x8b, 0x0e]); // mov rcx,[r14]
        self.emit(&[0x48, 0x83, 0xc1, 0x02]); // add rcx,2
        self.emit(&[0x48, 0x89, 0xca]); // mov rdx,rcx
        self.emit(&[0x48, 0xc1, 0xe2, 0x03]); // shl rdx,3
        self.emit(&[0x48, 0x83, 0xc2, 0x0f]); // add rdx,15
        self.emit(&[0x48, 0x83, 0xe2, 0xf0]); // and rdx,-16
        self.emit(&[0x49, 0x8d, 0xbf]); // lea rdi,[r15+STACK_BYTES]
        self.disp32(STACK_BYTES as i32);
        self.emit(&[0x48, 0x29, 0xd7]); // sub rdi,rdx
        self.state_store(248, 7); // initial virtual SP, always 16-byte aligned
        self.emit(&[0x4c, 0x89, 0xf6]); // mov rsi,r14
        self.emit(&[0xfc, 0xf3, 0x48, 0xa5]); // cld; rep movsq

        // Explicit IEEE rounding and gradual underflow, independent of the
        // launcher. SSE2 is part of the x86-64 baseline.
        self.imm64(0, 0x1f80);
        self.state_store(520, 0);
        self.emit(&[0x41, 0x0f, 0xae, 0x97]); // ldmxcsr [r15+520]
        self.disp32(520);
    }

    /// Translate the emitted Linux AArch64 operation number into the native
    /// Linux x86-64 syscall ABI. All Dunder registers except x0 are preserved;
    /// the shared runtime normalizes the returned negative errno afterwards.
    pub(super) fn syscall(&mut self) {
        self.load(0, 8);
        let mut mapped = Vec::new();
        for (arm, native) in [
            (93, 60),  // exit
            (63, 0),   // read
            (64, 1),   // write
            (222, 9),  // mmap
            (56, 257), // openat
            (57, 3),   // close
            (35, 263), // unlinkat
            (53, 268), // fchmodat
            (34, 258), // mkdirat
            (172, 39), // getpid
            (38, 264), // renameat
            (79, 262), // newfstatat
            (80, 5),   // fstat
            (46, 77),  // ftruncate
        ] {
            self.emit(&[0x48, 0x3d]); // cmp rax,imm32
            self.disp32(arm);
            self.emit(&[0x0f, 0x85]); // jne next
            let next = self.code.len();
            self.disp32(0);
            self.imm64(0, native);
            self.emit(&[0xe9]); // jmp mapped
            mapped.push(self.code.len());
            self.disp32(0);
            self.patch_local(next, self.code.len());
        }
        // Unsupported internal operations must never accidentally execute a
        // different host syscall with the same operation number.
        self.syscall_fatal();
        let target = self.code.len();
        for at in mapped {
            self.patch_local(at, target);
        }
        for (arm, native) in [(0, 7), (1, 6), (2, 2), (3, 10), (4, 8), (5, 9)] {
            self.load(native, arm);
        }
        self.emit(&[0x0f, 0x05]);
        self.store(0, 0);
    }

    fn syscall_fatal(&mut self) {
        self.imm64(0, 60);
        self.imm64(7, 74);
        self.emit(&[0x0f, 0x05, 0x0f, 0x0b]); // exit(74); ud2
    }

    fn patch_local(&mut self, at: usize, target: usize) {
        let delta =
            i32::try_from(target as i64 - (at + 4) as i64).expect("local syscall branch range");
        self.code[at..at + 4].copy_from_slice(&delta.to_le_bytes());
    }
}
