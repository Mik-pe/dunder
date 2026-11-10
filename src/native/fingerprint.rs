//! Content keys use checked bodies and resolved types. Numbered types and functions
//! are scoped to one shared context so keys remain valid after name resolution.
use super::reachable::{self, Dependencies};
use crate::{check::Program, target::Target};
use std::hash::{Hash, Hasher};

#[derive(Default)]
struct Bytes(Vec<u8>);
impl Hasher for Bytes {
    fn write(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }
    fn finish(&self) -> u64 {
        unreachable!("digest uses all 256 bits")
    }
}
pub(super) fn digest(input: impl Hash) -> [u8; 32] {
    let mut stream = Bytes::default();
    input.hash(&mut stream);
    super::sha256(&stream.0)
}

pub struct Fingerprints {
    pub(super) context: [u8; 32],
    pub(super) bodies: Vec<[u8; 32]>,
    pub(super) deps: Vec<Dependencies>,
}
impl Fingerprints {
    pub fn new(program: &Program, target: Target) -> Self {
        let context = digest((
            env!("DUNDER_COMPILER_HASH"),
            target.name(),
            &program.types,
            program.functions.len(),
            program.float_formatter,
        ));
        let bodies = program.functions.iter().map(digest).collect();
        let deps = (0..program.functions.len())
            .map(|id| reachable::dependencies(program, id))
            .collect();
        Self {
            context,
            bodies,
            deps,
        }
    }

    /// A test that observes files, processes or foreign code must always run.
    pub fn test(&self, entry: usize) -> Option<[u8; 32]> {
        let mut pending = vec![entry];
        let mut visited = vec![false; self.bodies.len()];
        let mut closure = Vec::new();
        while let Some(i) = pending.pop() {
            if visited[i] {
                continue;
            }
            visited[i] = true;
            if self.deps[i].external_effects {
                return None;
            }
            closure.push((i, &self.bodies[i]));
            pending.extend(&self.deps[i].functions);
        }
        Some(digest((self.context, entry, closure)))
    }
}
