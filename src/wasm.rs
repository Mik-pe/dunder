//! Direct WebAssembly binary emission from the checked tree. Local values use
//! Wasm locals; address-taken values and aggregate storage use linear memory.
use crate::{
    check::{Expr, Function, Program},
    syntax::{Type, Types},
};
use std::collections::{BTreeMap, HashMap, HashSet};

mod arithmetic;
mod function;

const I32: u8 = 0x7f;
const I64: u8 = 0x7e;
const F32: u8 = 0x7d;
const F64: u8 = 0x7c;
const EMPTY: u8 = 0x40;
const PAGE: usize = 65536;

fn val(ty: Type) -> u8 {
    match ty {
        Type::I64 | Type::U64 | Type::Range => I64,
        Type::F32 => F32,
        Type::F64 => F64,
        _ => I32,
    }
}
fn index(out: &mut Vec<u8>, mut n: usize) {
    while n >= 128 {
        out.push((n as u8 & 127) | 128);
        n >>= 7;
    }
    out.push(n as u8);
}
fn signed(out: &mut Vec<u8>, mut n: i64) {
    loop {
        let byte = n as u8 & 127;
        n >>= 7;
        if (n == 0 && byte & 64 == 0) || (n == -1 && byte & 64 != 0) {
            out.push(byte);
            break;
        }
        out.push(byte | 128);
    }
}
fn blob(out: &mut Vec<u8>, data: &[u8]) {
    index(out, data.len());
    out.extend(data);
}
fn section(out: &mut Vec<u8>, id: u8, data: Vec<u8>) {
    out.push(id);
    blob(out, &data);
}
#[derive(Clone, PartialEq, Eq)]
struct Signature {
    args: Vec<u8>,
    result: Option<u8>,
}
impl Signature {
    fn new(types: &Types, params: &[Type], ret: Type) -> Self {
        let mut args: Vec<_> = params.iter().copied().map(val).collect();
        let result = if types.storage_bytes(ret).is_some() {
            args.push(I32); // caller-owned result destination
            None
        } else {
            (ret != Type::Unit).then(|| val(ret))
        };
        Self { args, result }
    }
}
struct Import {
    module: String,
    name: String,
    ty: usize,
}
struct Module<'a> {
    types: &'a Types,
    signatures: Vec<Signature>,
    imports: Vec<Import>,
    indices: Vec<Option<usize>>,
    // Table slots correspond to reachable functions. Slot zero is always null.
    table: Vec<usize>,
    data: Vec<u8>,
    strings: HashMap<String, usize>,
}
impl Module<'_> {
    fn signature(&mut self, sig: Signature) -> usize {
        self.signatures
            .iter()
            .position(|s| *s == sig)
            .unwrap_or_else(|| {
                self.signatures.push(sig);
                self.signatures.len() - 1
            })
    }
    fn import(&mut self, module: &str, name: &str, ty: usize) -> usize {
        self.imports
            .iter()
            .position(|i| i.module == module && i.name == name && i.ty == ty)
            .unwrap_or_else(|| {
                self.imports.push(Import {
                    module: module.into(),
                    name: name.into(),
                    ty,
                });
                self.imports.len() - 1
            })
    }
    fn literal(&mut self, s: &str) -> usize {
        if let Some(&off) = self.strings.get(s) {
            return off;
        }
        let off = self.data.len().next_multiple_of(4);
        self.data.resize(off, 0);
        self.data.extend((s.len() as u64).to_le_bytes());
        self.data.extend(s.bytes());
        self.data.push(0);
        self.strings.insert(s.into(), off);
        off
    }
}
pub(super) fn compile(p: &Program, entry: usize, reachable: &[bool]) -> Result<Vec<u8>, String> {
    if p.types.pointer_bytes() != 4 {
        return Err("wasm32 requires checking with its 32-bit pointer layout".into());
    }
    let mut m = Module {
        types: &p.types,
        signatures: vec![],
        imports: vec![],
        indices: vec![None; p.functions.len()],
        table: vec![0; p.functions.len()],
        data: vec![0; 8],
        strings: HashMap::new(),
    };
    let mut internal = vec![];
    let mut funcs = vec![];
    for (id, f) in p
        .functions
        .iter()
        .enumerate()
        .filter(|(i, _)| reachable[*i] && Some(*i) != p.float_formatter)
    {
        m.table[id] = funcs.len() + 1;
        funcs.push(id);
        let sig = m.signature(Signature::new(&p.types, &f.params, f.ret));
        if let Some((module, name)) = &f.external {
            if f.params.iter().chain([&f.ret]).any(|t| p.types.storage_bytes(*t).is_some()) {
                return Err("wasm32 host imports take scalar values; pass C-layout records by pointer".into());
            }
            m.indices[id] = Some(m.import(module, name, sig));
        } else {
            internal.push((id, sig));
        }
    }
    let mut bodies = Vec::new();
    for &(id, _) in &internal {
        let f = &p.functions[id];
        let b = function::compile(&mut m, f).map_err(|e| format!("wasm32: {}: {e}", f.name))?;
        bodies.push(b);
    }
    for (i, &(id, _)) in internal.iter().enumerate() {
        m.indices[id] = Some(m.imports.len() + i);
    }
    let mut out = b"\0asm\x01\0\0\0".to_vec();
    let mut b = vec![];
    index(&mut b, m.signatures.len());
    for s in &m.signatures {
        b.push(0x60);
        blob(&mut b, &s.args);
        if let Some(r) = s.result {
            b.extend([1, r]);
        } else {
            b.push(0);
        }
    }
    section(&mut out, 1, b);
    if !m.imports.is_empty() {
        let mut b = vec![];
        index(&mut b, m.imports.len());
        for i in &m.imports {
            blob(&mut b, i.module.as_bytes());
            blob(&mut b, i.name.as_bytes());
            b.push(0);
            index(&mut b, i.ty);
        }
        section(&mut out, 2, b);
    }
    let mut b = vec![];
    index(&mut b, internal.len());
    for &(_, sig) in &internal {
        index(&mut b, sig);
    }
    section(&mut out, 3, b);
    let mut b = vec![1, 0x70, 0];
    index(&mut b, funcs.len() + 1);
    section(&mut out, 4, b);
    let stack_start = m.data.len().next_multiple_of(16);
    if stack_start > i32::MAX as usize {
        return Err("static data exceeds wasm32 address space".into());
    }
    let mut b = vec![1, 0];
    index(&mut b, stack_start.div_ceil(PAGE).max(1));
    section(&mut out, 5, b);
    let mut b = vec![1, I32, 1, 0x41];
    signed(&mut b, stack_start as i64);
    b.push(0x0b);
    section(&mut out, 6, b);
    let mut b = vec![3];
    blob(&mut b, b"main");
    b.push(0);
    index(&mut b, m.indices[entry].unwrap());
    blob(&mut b, b"memory");
    b.extend([2, 0]);
    blob(&mut b, b"__indirect_function_table");
    b.extend([1, 0]);
    section(&mut out, 7, b);
    let mut b = vec![1, 0, 0x41, 1, 0x0b];
    index(&mut b, funcs.len());
    for id in funcs {
        index(&mut b, m.indices[id].unwrap());
    }
    section(&mut out, 9, b);
    let mut b = vec![];
    index(&mut b, bodies.len());
    for body in bodies {
        blob(&mut b, &body.finish(&m.indices));
    }
    section(&mut out, 10, b);
    if m.data.len() > 8 {
        let mut b = vec![1, 0, 0x41, 0, 0x0b];
        blob(&mut b, &m.data);
        section(&mut out, 11, b);
    }
    Ok(out)
}

/// Branch depths are resolved while visiting structured statements. Direct
/// calls resolve after emission, when all reachable host imports are known.
struct Body {
    bytes: Vec<u8>,
    locals: Vec<u8>,
    calls: Vec<(usize, usize)>,
}
impl Body {
    fn finish(self, functions: &[Option<usize>]) -> Vec<u8> {
        let mut out = Vec::new();
        let groups: Vec<_> = self.locals.chunk_by(|a, b| a == b).collect();
        index(&mut out, groups.len());
        for g in groups {
            index(&mut out, g.len());
            out.push(g[0]);
        }
        let mut start = 0;
        for (pos, id) in self.calls {
            out.extend(&self.bytes[start..pos]);
            index(&mut out, functions[id].unwrap());
            start = pos;
        }
        out.extend(&self.bytes[start..]);
        out
    }
}
struct Emitter<'a, 'p> {
    m: &'a mut Module<'p>,
    f: &'a Function,
    body: Body,
    locals: BTreeMap<usize, usize>,
    params: usize,
    frame: usize,
    frame_used: bool,
    address_taken: HashSet<usize>,
    depth: usize,
    loops: Vec<(usize, usize)>,
}
impl Emitter<'_, '_> {
    fn op(&mut self, op: u8) {
        self.body.bytes.push(op);
    }
    fn bytes(&mut self, bytes: &[u8]) {
        self.body.bytes.extend(bytes);
    }
    fn arg(&mut self, op: u8, n: usize) {
        self.op(op);
        index(&mut self.body.bytes, n);
    }
    fn int(&mut self, ty: u8, n: i64) {
        self.op(if ty == I64 { 0x42 } else { 0x41 });
        signed(
            &mut self.body.bytes,
            if ty == I64 { n } else { i64::from(n as i32) },
        );
    }
    fn tmp(&mut self, ty: u8) -> usize {
        let i = self.params + self.body.locals.len();
        self.body.locals.push(ty);
        i
    }
    fn save(&mut self, ty: u8) -> usize {
        let l = self.tmp(ty);
        self.set(l);
        l
    }
    fn get(&mut self, l: usize) {
        self.arg(0x20, l);
    }
    fn set(&mut self, l: usize) {
        self.arg(0x21, l);
    }
    fn tee(&mut self, l: usize) {
        self.arg(0x22, l);
    }
    fn trap_if(&mut self) {
        self.bytes(&[0x04, EMPTY, 0x00, 0x0b]);
    }
    fn begin(&mut self, op: u8) -> usize {
        self.bytes(&[op, EMPTY]);
        self.depth += 1;
        self.depth
    }
    fn end(&mut self) {
        self.op(0x0b);
        self.depth -= 1;
    }
    fn branch(&mut self, op: u8, label: usize) {
        self.arg(op, self.depth - label);
    }
    fn direct_call(&mut self, id: usize) {
        self.op(0x10);
        self.body.calls.push((self.body.bytes.len(), id));
    }
    fn load_store(&mut self, ty: Type, store: bool, off: usize) {
        let (load, write, align) = match ty {
            Type::Bool | Type::U8 => (0x2d, 0x3a, 0),
            Type::U16 => (0x2f, 0x3b, 1),
            _ => match val(ty) {
                I64 => (0x29, 0x37, 3),
                F32 => (0x2a, 0x38, 2),
                F64 => (0x2b, 0x39, 3),
                _ => (0x28, 0x36, 2),
            },
        };
        self.bytes(&[if store { write } else { load }, align]);
        index(&mut self.body.bytes, off);
    }
    fn copy(&mut self, len: usize) {
        self.int(I32, len as i64);
        self.bytes(&[0xfc, 10, 0, 0]);
    }
    fn address(&mut self, slot: usize) {
        self.frame_used = true;
        self.get(self.frame);
        let off = (self.f.slots - slot - 1) * 8;
        if off != 0 {
            self.int(I32, off as i64);
            self.op(0x6a);
        }
    }
    fn slot(&mut self, slot: usize, ty: Type) -> usize {
        if let Some(&id) = self.locals.get(&slot) {
            return id;
        }
        let id = self.tmp(val(ty));
        self.locals.insert(slot, id);
        id
    }
    fn read(&mut self, slot: usize, ty: Type) {
        if self.address_taken.contains(&slot) {
            self.address(slot);
            self.load_store(ty, false, 0);
        } else {
            let id = self.slot(slot, ty);
            self.get(id);
        }
    }
    fn write(&mut self, slot: usize, ty: Type) {
        if self.address_taken.contains(&slot) {
            let tmp = self.save(val(ty));
            self.address(slot);
            self.get(tmp);
            self.load_store(ty, true, 0);
        } else {
            let id = self.slot(slot, ty);
            self.set(id);
        }
    }
    fn eval_saved(&mut self, e: &Expr) -> Result<usize, String> {
        self.expr(e)?;
        Ok(self.save(val(e.ty)))
    }
}
