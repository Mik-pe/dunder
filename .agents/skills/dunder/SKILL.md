---
name: dunder
description: Write, review and debug Dunder (.ddr) programs. Use for Dunder syntax, compiler diagnostics, ownership, standard library calls and local tests.
---

# Dunder

Check the project's compiler version with `dunder --version`. The example below runs with 0.13.0. Look at nearby `.ddr` files for the project's imports and capabilities.

- Declare a local with `name := value` or `name: Type = value`. Add `mut` when the binding needs reassignment. A diagnostic such as `unknown local 'let'` on a declaration points to this syntax.
- Convert integers to text with `n.str()`, or interpolate with `f"{n}"`. Widen operands before arithmetic if the result can exceed their original type.
- A function's final expression supplies its return value. Use `.Some(value)` and `.None` for `Option<T>`, and `.Ok(value)` and `.Err(error)` for `Result<T, E>`. A compatible fallible call can propagate its error with `?`.
- For a parameter of type `&[]T`, bind the caller's array as `items: []T = [...]` and pass `&items`. An unannotated array literal in a local binding infers a fixed-length array. Use `&mut` for an exclusive writable borrow, `move` to transfer an owned value and `.clone()` when both places need an independent copy.

```dunder
pub fn numbered(label: string, index: i32) -> string {
    mut number := i64(index)
    number += 1
    prefix := number.str() + ". "
    prefix + label
}

fn first_positive(values: &[]i32) -> Option<i32> {
    for value in values {
        if value > 0 { return .Some(value) }
    }
    .None
}

@test fn labels_and_values() {
    assert(numbered("item", 2147483647) == "2147483648. item")
    values: []i32 = [-2, 5, 3]
    assert(first_positive(&values).unwrap() == 5)
    assert(first_positive(&[-2, 0]).is_none())
}
```

Run `dunder test path/to/test.ddr` for a standalone test module, or `dunder test` for the package test root in `dunder.toml`. In a locked checkout with cached dependencies, add `--offline --locked`. Import a sibling module with `import .module_name` and call its public declarations through that name.

For an unfamiliar operation, look up its actual declaration under `stdlib/` or `packages/` in the compiler revision the project uses. The language reference covers expressions, borrowing and standard APIs. [Syntax](https://github.com/Mik-pe/dunder/blob/v0.13.0/docs/syntax.md) · [Ownership](https://github.com/Mik-pe/dunder/blob/v0.13.0/docs/memory.md) · [Standard library](https://github.com/Mik-pe/dunder/blob/v0.13.0/docs/standard.md)
