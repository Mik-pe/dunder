# Agent rules

## Work directly on main

Work directly on `main` and push verified milestones to `origin/main`.
Routine work does not need a feature branch, pull request, or another
permission round. Keep local verification proportional to the change.
Preserve existing work in other checkouts.

## Develop with the Dunder compiler

Use `tools/bootstrap.sh` to build the compiler from its frozen seed. Verify
changes with self-built executables, expected outputs, compile-time diagnostics
and target runtimes. Run `tools/verify-selfhost.sh` for full correctness verification. Keep the
build and test path independent of Cargo and use target runtimes and expected
results for verification. Run benchmarks only when specifically requested.

## Graphics dogfood

Keep graphics API bindings, shaders and window logic with the examples under
`dogfood/`. Exercise the same file imports and FFI available to external
projects. When an example exposes a missing language or compiler feature,
implement and test the general mechanism in the toolchain.

## Use subagents

Use subagents for independent research, implementation, and verification
when it helps the task. Give parallel writers separate files or isolated
worktrees, then integrate their work on `main`.

## No GitHub CI

Never add, configure, enable, or require CI through GitHub for this repository.
This includes GitHub Actions workflows and CI integrations triggered through
GitHub. Do not create `.github/workflows` or require GitHub CI status checks.

Run builds, tests, and performance measurements locally or on a machine the
user explicitly selects. Report the checks actually performed. Local
verification scripts are allowed; do not turn them into GitHub CI.
