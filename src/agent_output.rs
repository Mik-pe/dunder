//! One JSON record per line. Offsets are UTF-8 byte offsets, with an exclusive end.
use dunder::{
    check::BindingInfo,
    modules::{Checked, Diagnostic},
};
use std::{fmt::Write, path::Path};

fn quote(value: &str) -> String {
    let mut result = String::with_capacity(value.len() + 2);
    result.push('"');
    for c in value.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c.is_control() => write!(result, "\\u{:04x}", c as u32).unwrap(),
            c => result.push(c),
        }
    }
    result.push('"');
    result
}

pub fn diagnostic(d: &Diagnostic, root: &Path) {
    let e = &d.error;
    let path = d
        .sources
        .files
        .get(e.file)
        .map_or(root, |s| s.path.as_path());
    println!(
        "{{\"kind\":\"error\",\"code\":{},\"path\":{},\"start\":{},\"end\":{},\"message\":{}}}",
        quote(e.code),
        quote(&path.to_string_lossy()),
        e.span.start,
        e.span.end,
        quote(&e.message)
    );
}

pub fn success(project: &Checked, bindings: &[BindingInfo]) {
    for b in bindings {
        let path = &project.sources.files[b.file].path;
        println!(
            "{{\"kind\":\"binding\",\"path\":{},\"start\":{},\"end\":{},\"name\":{},\"type\":{}}}",
            quote(&path.to_string_lossy()),
            b.span.start,
            b.span.end,
            quote(&b.name),
            quote(&project.program.types.name(b.ty))
        );
    }
    println!(
        "{{\"kind\":\"ok\",\"modules\":{}}}",
        project.sources.files.len()
    );
}
