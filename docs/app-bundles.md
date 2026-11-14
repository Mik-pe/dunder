# Native macOS application bundles

`dunder build --app` packages the native ARM64 executable as a macOS `.app`:

```sh
dunder build main.ddr --app -o build/DunderMetal.app
dunder run main.ddr --app -o build/DunderMetal.app -- --frames 120
```

Without `-o`, the output is `build/arm64-macos/<name>.app`. The name comes from
the source file stem or the nearest `dunder.toml` manifest. An explicit output
must have a nonempty name ending in `.app`. `check` and `test` reject `--app`.

Bundles require target `arm64-macos`. When cross-compiling from Linux, select
it with `--target arm64-macos`; `run --app` requires a matching macOS host.

For `build/DunderMetal.app`, Dunder writes:

```text
build/DunderMetal.app/
  Contents/
    Info.plist
    MacOS/
      DunderMetal
    _CodeSignature/
      CodeResources
```

The filename stem supplies the bundle name and display name. ASCII stems also
name the executable. With a Unicode stem such as `Ångström.app`, Dunder names
the executable `dunder-<hash>` using the same eight hex digits as the bundle
identifier. This keeps the signed executable path stable across Unicode
filesystem normalizations. Read `CFBundleExecutable` for its exact name.
`Info.plist` identifies an `APPL` bundle, enables high resolution rendering and
automatic graphics switching, and declares macOS 11.0 as the minimum system
version. The bundle and short version strings use the Dunder compiler version.
Its identifier is `org.dunder.<sanitized-name>-<hash>`: the name uses lowercase
ASCII letters, digits and hyphens, and an eight digit deterministic name hash
distinguishes names that sanitize to the same text.

Packaging uses Dunder's own filesystem operations and ad hoc signer. The bundle
signature binds `Info.plist` and a generated `CodeResources` envelope to the
executable. The envelope records SHA-256 hashes of existing regular resource
files, including executables retained from an earlier bundle name. Rebuilding
refreshes the envelope. Changes to sealed resources or the plist invalidate the
signature until the next build. This follows the
[CodeDirectory special-slot format](https://developer.apple.com/documentation/technotes/tn3126-inside-code-signing-hashes).

Building needs no external compiler, linker, packaging utility, or `codesign`
invocation. Local verification uses macOS's `codesign --verify --strict` as an
independent validator. The bundle is ad hoc signed for local development;
distribution signing and notarization are separate work.

`dunder run --app` executes the binary named by `CFBundleExecutable` under `Contents/MacOS/`, preserves program
arguments after `--`, and returns the program's exit status. The executable lives
inside the bundle so native APIs can discover its application bundle. The
program controls its own application lifecycle and windows. Running a simple
console program with `--app` still prints to the current terminal.

Source loading, type checking, and native generation finish before publication.
A type error leaves an existing bundle untouched. Dunder checks the executable,
`Info.plist`, and `CodeResources` destinations against all loaded source files,
including symlink and hard link aliases, before publishing any file. Each
generated file is written to a temporary sibling and renamed into place.
Publication is atomic per file; replacing the entire bundle is not atomic. If
publication fails between files, the bundle can contain files from different
builds.

Rebuilding updates only the executable, `Contents/Info.plist`, and
`Contents/_CodeSignature/CodeResources`. Existing regular resources and other
executables are kept and sealed. If the bundle is renamed, its old executable is
also kept; the new plist selects the executable matching its current name.

The current packager rejects symlinks and special files in existing resources,
non-UTF-8 or control-character resource filenames, unknown files in
`_CodeSignature`, and entries outside the bundle's `Contents` directory. It also
requires real directories in the bundle rather than directory symlinks. These
checks finish before publication and preserve unsupported files in place.

The self-hosted compiler accepts the same build form, including cross-builds
from Linux. For example, `dunder build triangle.ddr --app
--target arm64-macos -o build/Triangle.app` writes and signs the bundle in
Dunder. Existing resource files are hashed as binary data. The packager uses
POSIX directory and file operations on the build host.
