//! Native function fragments are saved before image layout. Branch targets belong
//! to the shared runtime, a function-local code offset, or the image literal pool.
use super::{fingerprint::digest, Branch, Code, Fingerprints};
use crate::{check::Program, target::Target};
use std::{
    collections::HashMap,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Default)]
pub struct CacheStats {
    pub hits: usize,
    pub misses: usize,
}
pub struct Incremental<'a> {
    program: &'a Program,
    target: Target,
    dir: PathBuf,
    keys: Fingerprints,
    pub stats: CacheStats,
}
impl<'a> Incremental<'a> {
    pub fn new(dir: &Path, program: &'a Program, target: Target) -> Result<Self, String> {
        if target == Target::Wasm32 {
            return Err("the function code cache currently supports native targets".into());
        }
        Ok(Self {
            dir: dir.to_owned(),
            program,
            target,
            keys: Fingerprints::new(program, target),
            stats: CacheStats::default(),
        })
    }

    pub fn test_key(&self, entry: usize) -> Option<[u8; 32]> {
        self.keys.test(entry)
    }

    pub fn compile(&mut self, entry: usize) -> Result<Vec<u8>, String> {
        super::compile_cached(self.program, entry, self.target, Some(self))
    }
    pub(super) fn path(&self, f: usize, p: &Program, imports: &[u8; 32]) -> PathBuf {
        let deps = self.keys.deps[f]
            .functions
            .iter()
            .filter_map(|&callee| {
                super::optimize::inline_body(&p.functions[callee]).map(|_| self.keys.bodies[callee])
            })
            .collect::<Vec<_>>();
        let key = digest((self.keys.context, self.keys.bodies[f], deps, imports));
        self.dir
            .join(key.iter().map(|b| format!("{b:02x}")).collect::<String>())
    }
}

pub(super) struct Mark {
    word: usize,
    branch: usize,
    data: usize,
}
impl Mark {
    pub fn new(c: &mut Code) -> Self {
        c.literal_trace = Some(Vec::new());
        Self {
            word: c.words.len(),
            branch: c.fixups.len(),
            data: c.data_fixups.len(),
        }
    }
}

fn put(data: &mut Vec<u8>, n: usize) {
    data.extend_from_slice(&(n as u64).to_le_bytes());
}

pub(super) fn save(c: &mut Code, start: Mark, path: &Path, globals: usize) -> io::Result<()> {
    let mut bytes = Vec::new();
    put(&mut bytes, c.words.len() - start.word);
    for w in &c.words[start.word..] {
        bytes.extend_from_slice(&w.to_le_bytes());
    }
    let mut literals = HashMap::new();
    let mut names = Vec::new();
    for label in c.literal_trace.take().unwrap() {
        if let std::collections::hash_map::Entry::Vacant(entry) = literals.entry(label) {
            entry.insert(names.len());
            let (value, _) = c
                .strings
                .literals
                .iter()
                .find(|(_, l)| *l == label)
                .unwrap();
            names.push(value);
        }
    }
    put(&mut bytes, names.len());
    for text in names {
        put(&mut bytes, text.len());
        bytes.extend_from_slice(text.as_bytes());
    }
    put(&mut bytes, c.fixups.len() - start.branch);
    for (site, label, how) in &c.fixups[start.branch..] {
        put(&mut bytes, site - start.word);
        let (tag, operand) = match how {
            Branch::Jump => (0, 0),
            Branch::Call => (1, 0),
            Branch::Cond(v) => (2, *v),
            Branch::Zero(v) => (3, *v),
            Branch::Nonzero(v) => (4, *v),
            Branch::Address(v) => (5, *v),
        };
        bytes.extend_from_slice(&[tag, operand as u8]);
        let (kind, id) = if *label < globals {
            (0, *label)
        } else if let Some(index) = literals.get(label) {
            (1, *index)
        } else {
            (2, c.labels[*label].unwrap() - start.word)
        };
        bytes.push(kind);
        put(&mut bytes, id);
    }
    put(&mut bytes, c.data_fixups.len() - start.data);
    for (site, slot, reg) in &c.data_fixups[start.data..] {
        put(&mut bytes, site - start.word);
        put(&mut bytes, *slot);
        bytes.push(*reg as u8);
    }
    let checksum = digest((path.file_name(), &bytes));
    bytes.extend_from_slice(&checksum);
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let parent = path.parent().unwrap();
    fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(
        ".{}-{}.tmp",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)?;
    let result = f.write_all(&bytes).and_then(|()| fs::rename(&tmp, path));
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}
struct Reader<'a>(&'a [u8]);
impl<'a> Reader<'a> {
    fn bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        let (v, rest) = self.0.split_at_checked(n)?;
        self.0 = rest;
        Some(v)
    }
    fn num(&mut self) -> Option<usize> {
        usize::try_from(u64::from_le_bytes(self.bytes(8)?.try_into().ok()?)).ok()
    }
    fn byte(&mut self) -> Option<u8> {
        Some(self.bytes(1)?[0])
    }
}
pub(super) fn restore(c: &mut Code, path: &Path, globals: usize) -> Option<()> {
    let file = fs::read(path).ok()?;
    let (payload, sum) = file.split_at_checked(file.len().checked_sub(32)?)?;
    if digest((path.file_name(), payload)) != sum {
        return None;
    }
    let mut r = Reader(payload);
    let word_count = r.num()?;
    let words = r.bytes(word_count.checked_mul(4)?)?;
    let mut strings = Vec::new();
    for _ in 0..r.num()? {
        let len = r.num()?;
        strings.push(std::str::from_utf8(r.bytes(len)?).ok()?);
    }
    let mut branches = Vec::new();
    for _ in 0..r.num()? {
        let site = r.num()?;
        let tag = r.byte()?;
        let value = u32::from(r.byte()?);
        let how = match tag {
            0 => Branch::Jump,
            1 => Branch::Call,
            2 if value < 16 => Branch::Cond(value),
            3 if value < 32 => Branch::Zero(value),
            4 if value < 32 => Branch::Nonzero(value),
            5 if value < 32 => Branch::Address(value),
            _ => return None,
        };
        let kind = r.byte()?;
        let id = r.num()?;
        if site >= word_count
            || (tag >= 2 && site + 1 >= word_count)
            || !matches!(kind, 0 if id < globals)
                && !matches!(kind, 1 if id < strings.len())
                && !matches!(kind, 2 if id <= word_count)
        {
            return None;
        }
        branches.push((site, how, kind, id));
    }
    let mut data = Vec::new();
    for _ in 0..r.num()? {
        let site = r.num()?;
        let slot = r.num()?;
        let reg = r.byte()?;
        if site.checked_add(1)? >= word_count || reg >= 32 {
            return None;
        }
        data.push((site, slot, u32::from(reg)));
    }
    if !r.0.is_empty() {
        return None;
    }
    let first = c.words.len();
    c.words.extend(
        words
            .as_chunks::<4>()
            .0
            .iter()
            .map(|w| u32::from_le_bytes(*w)),
    );
    let literals = strings
        .iter()
        .map(|s| c.string_literal(s))
        .collect::<Vec<_>>();
    let mut local = HashMap::new();
    for (site, how, kind, id) in branches {
        let target = match kind {
            0 => id,
            1 => literals[id],
            _ => *local.entry(id).or_insert_with(|| {
                let label = c.label();
                c.labels[label] = Some(first + id);
                label
            }),
        };
        c.fixups.push((first + site, target, how));
    }
    c.data_fixups.extend(
        data.into_iter()
            .map(|(at, slot, reg)| (first + at, slot, reg)),
    );
    Some(())
}
