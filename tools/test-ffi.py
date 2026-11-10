#!/usr/bin/env python3
"""Check the native C ABI of an installed Dunder compiler on the current host."""
import argparse
import os
from pathlib import Path
import subprocess
import sys
import tempfile

FIXTURES = Path(__file__).resolve().parents[1] / "tests" / "fixtures" / "ffi"


def run(command, directory, expected=0, env=None):
    result = subprocess.run(command, cwd=directory, env=env, capture_output=True,
                            text=True, timeout=30)
    if result.returncode != expected:
        raise RuntimeError(f"{command} returned {result.returncode}, expected {expected}\n"
                           f"{result.stdout}{result.stderr}")
    return result


def invalid_sources():
    unsafe = "capability unsafe\n"
    external = '@extern("c","abs") unsafe fn external(x:i32)->i32\n'
    for decl in (external, "unsafe fn external(x:i32)->i32{x}\n"):
        yield unsafe + decl + "fn main()->i32{f:=external;f(7)}"
        yield unsafe + decl + "fn main(){f:fn(i32)->i32=external}"
    yield external + "fn main(){}"
    yield "fn main(){unsafe{p:=ptr(0)}}"
    yield unsafe + "fn main(){p:=ptr(0)}"
    yield unsafe + "fn main(){unsafe{p:=ptr(0)};q:=ptr(1)}"
    yield unsafe + '@extern("c","f") fn f()\nfn main(){}'
    for field in ("string", "[]u8", "[u8;0]", "fn(i32)->i32", "Normal"):
        yield f"struct Normal{{v:i32}}\n@c_layout struct Bad{{f:{field}}}\nfn main(){{}}"
    for parameter in ("string", "[]u8", "&i32", "fn(i32)->i32", "Record"):
        yield (unsafe + "@c_layout struct Record{a:i32}\n" +
               f'@extern("c","bad") unsafe fn foreign(x:{parameter})\nfn main(){{}}')
    yield unsafe + 'fn main(){unsafe{p:=cstr("zero\\0byte")}}'
    yield unsafe + '@extern("","symbol") unsafe fn f()\nfn main(){}'
    yield unsafe + '@extern("c","") unsafe fn f()\nfn main(){}'
    yield "fn main(){small:u8=256}"
    yield "fn main(){small:u8=-1}"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("compiler", type=Path)
    parser.add_argument("--cc", default=os.environ.get("CC", "cc"))
    args = parser.parse_args()
    compiler = args.compiler.resolve()
    with tempfile.TemporaryDirectory(prefix="dunder-ffi-") as temp:
        directory = Path(temp)
        mac = sys.platform == "darwin"
        library = directory / ("probe.dylib" if mac else "probe.so")
        run([args.cc, "-std=c11", "-Werror", "-pthread", "-fPIC",
             "-dynamiclib" if mac else "-shared", FIXTURES / "oracle.c", "-o", library], directory)
        # The compiler emits its own complete runnable Mach-O/ELF after the
        # oracle build. PATH is empty throughout every Dunder build and launch.
        env = {"PATH": ""}
        source = directory / "input.ddr"
        executable = directory / "program"
        template = (FIXTURES / "native.ddr.in").read_text()
        libname = str(library).replace('\\', '\\\\').replace('"', '\\"')
        source.write_text(template.replace("@LIB@", libname))
        run([compiler, "build", source, "-o", executable], directory, env=env)
        actual = run([executable], directory, env=env)
        if actual.stdout != "native ABI ok\n":
            raise AssertionError(f"native ABI probe did not finish: {actual.stdout!r}")

        source.write_text(f'''capability unsafe
@extern("{libname}", "on_worker") unsafe fn on_worker(f:extern fn())
fn noop() {{}}
fn main() {{ unsafe {{ on_worker(callback(noop)) }} }}
''')
        run([compiler, "build", source, "-o", executable], directory, env=env)
        run([executable], directory, expected=111, env=env)

        rejected = list(invalid_sources())
        for index, case in enumerate(rejected):
            source.write_text(case)
            result = run([compiler, "check", source], directory, expected=1, env=env)
            if "error[" not in result.stderr:
                raise AssertionError(f"missing source diagnostic #{index}: {result.stderr}")
        source.write_text("fn main(){limit:u64=18446744073709551615;wide:usize=4294967296}")
        run([compiler, "check", source], directory, env=env)
        run([compiler, "check", source, "--target", "wasm32"], directory, expected=1, env=env)
        print(f"native layout, mixed C args, HFA, reentrant callbacks and worker rejection passed; "
              f"{len(rejected) + 1} invalid programs rejected")


if __name__ == "__main__":
    main()
