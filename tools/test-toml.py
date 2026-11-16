#!/usr/bin/env python3
"""Compare emitted TOML parsers with independent TOML 1.0 and JSON oracles.

One Dunder batch driver is built per native target. Python's stdlib tomllib is
only a test oracle; the emitted executable parses every source itself. The
optional upstream corpus is toml-lang/toml-test's files-toml-1.0.0 manifest
(tested with v1.6.0, commit 369cc3ae275fd76b6e21780ebf87609e5b2a951a).

Grammar examples derive from https://toml.io/en/v1.0.0. Contract cases cover
Dunder's positive-u64 extension, lexical date/float representation, and bounded
resources. No external Python packages or runtime TOML helpers are required.
"""
import argparse
from dataclasses import dataclass, field
import datetime
from decimal import Decimal
import hashlib
import json
from pathlib import Path
import platform
import re
import shlex
import subprocess
import sys
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
TARGETS = ("arm64-macos", "arm64-linux", "x86_64-linux")
INPUT_LIMIT = 4 * 1024 * 1024
NODE_LIMIT = 100_000
MISSING = object()
try:
    tomllib.loads("v={x=1,}")
except tomllib.TOMLDecodeError:
    ORACLE_TOML_1_1 = False
else:
    ORACLE_TOML_1_1 = True

# Newer Python stdlibs implement TOML 1.1. These remain independently specified
# TOML 1.0 rejections: https://toml.io/en/v1.0.0#inline-table / #string / #local-time.
TOML_1_0_ONLY_NEGATIVES = {"inline-trailing-comma", "inline-newline", "inline-comment",
                             "basic-x-escape", "temporal-invalid/00:00"}


@dataclass(frozen=True)
class Number:
    kind: str
    spelling: str


@dataclass(frozen=True)
class Temporal:
    kind: str
    spelling: str
    exact: bool = True


@dataclass
class Case:
    name: str
    source: bytes
    ok: bool
    expected: object = None
    lexemes: dict = field(default_factory=dict)
    diagnostic: str = ""
    invalid_utf8: bool = False
    location: bool = True
    oracle: str = "tomllib"


def host_target():
    system, machine = platform.system(), platform.machine()
    if system == "Darwin" and machine in ("arm64", "aarch64"):
        return "arm64-macos"
    if system == "Linux" and machine in ("arm64", "aarch64"):
        return "arm64-linux"
    if system == "Linux" and machine in ("x86_64", "AMD64"):
        return "x86_64-linux"
    raise RuntimeError(f"unsupported execution host {system}/{machine}")


def parse_oracle(text):
    # Decimal avoids binary64 overflow, underflow and rounding in the oracle.
    return tomllib.loads(text, parse_float=Decimal)


def grammar_cases():
    cases = []

    def valid(name, text, *, expected=MISSING, lexemes=None, oracle="tomllib"):
        parsed = parse_oracle(text) if expected is MISSING else expected
        cases.append(Case(name, text.encode(), True, parsed,
                          lexemes=lexemes or {}, oracle=oracle))

    def invalid(name, text, *, contract=False, diagnostic="", location=True):
        oracle = "Dunder public limits/range contract" if contract else "tomllib"
        if not contract:
            try:
                parse_oracle(text)
            except tomllib.TOMLDecodeError:
                pass
            else:
                if not (ORACLE_TOML_1_1 and name in TOML_1_0_ONLY_NEGATIVES):
                    raise AssertionError(f"invalid fixture accepted by tomllib: {name}")
                oracle = "TOML 1.0 specification; Python tomllib implements TOML 1.1"
        cases.append(Case(name, text.encode(), False, diagnostic=diagnostic,
                          location=location, oracle=oracle))

    valid("empty", "")
    valid("comments", "# comment\n\t # tab\n a = '# string' # end\n")
    valid("crlf", "# comment\r\na = 1\r\n[b]\r\nc = [true,\r\nfalse,]\r\n")
    valid("keys", '''bare-key_0 = true
1234 = "digits"
"" = "empty"
"127.0.0.1" = "quoted"
'quoted "key"' = "literal"
"雪🌍" = "unicode"
"escaped\\u0020key" = 42
''')
    valid("dotted-keys", '''name = "Orange"
physical.color = "orange"
physical . shape = "round"
site."google.com" = true
3.14159 = "pi"
''')
    valid("dotted-out-of-order", "apple.type='fruit'\norange.type='fruit'\napple.skin='thin'\norange.skin='thick'\n")
    valid("dotted-empty-segments", '''""."" = 1
"a.b".'c.d' = 2
''')
    valid("tables", '''[dog."tater.man"]
type.name = "pug"
[dog]
age = 2
[empty]
[dog."tater.man".empty]
''')
    valid("implicit-table-later-declared", "[a.b.c]\nx=1\n[a]\ny=2\n[a.b]\nz=3\n")
    valid("dotted-key-then-subtable", "fruit.apple.color='red'\n[fruit.apple.texture]\nsmooth=true\n")
    valid("table-relative-dotted", "[a]\nb.c=1\nb.d=2\n[a.e]\nf=3\n")
    valid("arrays", '''mixed = [1, "two", false, [3, 4], {x = 5}]
empty = []
nested = [[1, 2], [3], []]
commented = [
  1, # first
  # full line
  2,
]
''')
    valid("inline-table", '''a = { x = 1, nested.y = true, nested.z = 'ok', empty = {} }
b = [{x=1}, {x=2}]
''')
    valid("inline-multiline-value", 'a = {text = """\nfirst\nsecond""", list = [\n1,\n2,\n]}\n')
    valid("inline-sibling-table", "a={b=1}\nc=2\n[d]\ne=3\n")
    valid("aot", '''[[products]]
name = "Hammer"
sku = 738594937
[[products]]
[[products]]
name = "Nail"
sku = 284758393
color = "gray"
''')
    valid("aot-nested", '''[[fruits]]
name = "apple"
[fruits.physical]
color = "red"
[[fruits.varieties]]
name = "red delicious"
[[fruits.varieties]]
name = "granny smith"
[[fruits]]
name = "banana"
[[fruits.varieties]]
name = "plantain"
''')
    valid("aot-most-recent-parent", '''[[a]]
[a.b]
x=1
[[a.b.c]]
v=2
[[a]]
[a.b]
x=3
[[a.b.c]]
v=4
''')
    valid("aot-independent-empty-elements", "[[a]]\n[[a.b]]\n[[a.b]]\n[[a]]\n[[a.b]]\n")
    valid("array-table-literal-remains-value", "a=[{x=1},{x=2}]\n")

    strings = {
        "basic": r'''v = "I'm a string. \"quoted\". Name\tJos\u00E9\nLocation\tSF."''',
        "basic-escapes": r'''v = "\b\t\n\f\r\"\\\u0000\u001f\u007f"''',
        "literal": r"v = 'C:\Users\nodejs\templates'",
        "literal-regex": r"v = '<\i\c*\s*>'",
        "multiline-basic": 'v = """\nRoses are red\nViolets are blue"""',
        "multiline-literal": "v = '''\nThe first newline is\ntrimmed.\n  Other whitespace remains.\n'''",
        "multiline-continuation": 'v = """\nThe quick brown \\\n\n  fox jumps over \\ \t\n    the lazy dog.\\\n  """',
        "multiline-tab": 'v = """\tvalue\n\tend"""',
        "literal-backslash-newline": "v = '''a\\\nb'''",
        "literal-quotes": "v = '''I [dw]on't need \\d{2} apples'''",
        "unicode-scalars": r'''v = "\u0000\u007f\u0080\u07ff\u0800\ud7ff\ue000\uffff\U00010000\U0010ffff"''',
        "unicode-direct": 'v = "é é Ελληνικά 雪 🌍"',
        "basic-tab": 'v = "a\tb"',
        "literal-tab": "v = 'a\tb'",
        "multiline-crlf": 'v = """\r\none\r\ntwo"""',
        "multiline-literal-crlf": "v = '''\r\none\r\ntwo'''",
    }
    for name, text in strings.items():
        valid("string/" + name, text)
    for delimiter, label in (("\"\"\"", "basic"), ("'''", "literal")):
        quote = delimiter[0]
        for count in range(3):
            valid(f"string/{label}-terminal-{count}-quotes",
                  "v=" + delimiter + "word" + quote * count + delimiter)
            valid(f"string/{label}-internal-{count}-quotes",
                  "v=" + delimiter + "left" + quote * count + "right" + delimiter)
        valid(f"string/{label}-empty", "v=" + delimiter * 2)

    integers = ["0", "+0", "-0", "1", "+99", "-17", "1_000", "5_349_221",
                "0xDEAD_BEEF", "0o755", "0b1101_0010", "0x0000_FF", "0o0007", "0b0001",
                "9007199254740991", "9007199254740992", "9007199254740993",
                "9223372036854775807", "-9223372036854775808",
                "9223372036854775808", "18446744073709551615",
                "0xffff_ffff_ffff_ffff", "0o1777777777777777777777", "0b" + "1" * 64]
    for index, spelling in enumerate(integers):
        value = parse_oracle("v=" + spelling)["v"]
        canonical = str(value)
        # TOML integer -0 is zero, including when read through as_u64.
        lexemes = {("v",): canonical}
        valid(f"integer/{index}-{spelling}", "v=" + spelling, lexemes=lexemes)
    floats = ["1.0", "+1.0", "-0.0", "+0.0", "0.0", "3.1415", "1e2",
              "1E+02", "1e-2", "-1.234E-12", "6.626e-34", "1_000.0_1E+0_2",
              "0e00", "-0e0", "1.7976931348623157e308", "5e-324",
              "1e10000", "1e-10000", "1.2345678901234567890123456789"]
    for index, spelling in enumerate(floats):
        valid(f"float/{index}-{spelling}", "v=" + spelling,
              lexemes={("v",): spelling.replace("_", "").removeprefix("+")})
    for spelling in ("inf", "+inf", "-inf", "nan", "+nan", "-nan"):
        valid("float/" + spelling, "v=" + spelling)
    valid("numbers-in-nested-values", "v=[9007199254740993, 1.0, {x=18446744073709551615, y=1e2}]",
          lexemes={("v", 0): "9007199254740993", ("v", 1): "1.0",
                   ("v", 2, "x"): "18446744073709551615", ("v", 2, "y"): "1e2"})

    temporals = [
        ("1979-05-27", "date", "1979-05-27"),
        ("2000-02-29", "date", "2000-02-29"),
        ("0001-01-01", "date", "0001-01-01"),
        ("9999-12-31", "date", "9999-12-31"),
        ("07:32:00", "time", "07:32:00"),
        ("00:00:00.000000000", "time", "00:00:00.000000000"),
        ("23:59:59.1234567890123456789", "time", "23:59:59.1234567890123456789"),
        ("1979-05-27T07:32:00Z", "datetime", "1979-05-27T07:32:00Z"),
        ("1979-05-27t07:32:00z", "datetime", "1979-05-27T07:32:00Z"),
        ("1979-05-27 07:32:00Z", "datetime", "1979-05-27T07:32:00Z"),
        ("1979-05-27T00:32:00-07:00", "datetime", "1979-05-27T00:32:00-07:00"),
        ("1979-05-27T07:32:00.999999999+23:59", "datetime", "1979-05-27T07:32:00.999999999+23:59"),
        ("1979-05-27T07:32:00-00:00", "datetime", "1979-05-27T07:32:00-00:00"),
        ("1979-05-27T07:32:00", "datetime", "1979-05-27T07:32:00"),
        ("1979-05-27 07:32:00.000000000", "datetime", "1979-05-27T07:32:00.000000000"),
    ]
    for index, (spelling, kind, normalized) in enumerate(temporals):
        # First establish validity independently, then retain precision that
        # Python datetime deliberately truncates beyond microseconds.
        parse_oracle("v=" + spelling)
        valid(f"temporal/{index}", "v=" + spelling,
              expected={"v": Temporal(kind, normalized)})
    for spelling in ("0000-01-01", "0000-02-29T00:00:00Z"):
        kind = "datetime" if "T" in spelling else "date"
        valid("temporal/year-zero-" + kind, "v=" + spelling,
              expected={"v": Temporal(kind, spelling)},
              oracle="RFC 3339 lexical year and Dunder public contract; Python year-zero gap")

    invalids = {
        "missing-key": "=1", "missing-value": "a=", "missing-equals": "a 1",
        "missing-key-value-newline": "a=1 b=2", "bare-value": "1",
        "bare-unicode-key": "雪=1", "bare-space-key": "a b=1",
        "bare-plus-key": "+a=1", "empty-dotted-key": "a..b=1",
        "leading-dot": ".a=1", "trailing-dot": "a.=1",
        "dotted-newline": "a.\nb=1", "quoted-key-multiline": '"""key"""=1',
        "duplicate-key": "a=1\na=2", "quoted-duplicate": "a=1\n'a'=2",
        "escaped-duplicate": 'a=1\n"\\u0061"=2',
        "scalar-to-table": "a=1\na.b=2", "table-to-scalar": "a.b=1\na=2",
        "duplicate-table": "[a]\n[a]", "dotted-table-redefined": "a.b=1\n[a]",
        "dotted-child-redefined": "a.b.c=1\n[a.b]",
        "header-scalar-conflict": "a=1\n[a]", "header-array-conflict": "a=[]\n[a]",
        "inline-extension-dotted": "a={b=1}\na.c=2",
        "inline-extension-header": "a={b=1}\n[a.c]",
        "inline-child-extension": "a={b={c=1}}\n[a.b.d]",
        "inline-root-redefinition": "a={}\n[a]",
        "inline-dotted-duplicate": "a={b.c=1,b.c=2}",
        "inline-nested-conflict": "a={b=1,b.c=2}",
        "inline-sealed-sibling": "a={b={c=1},b.d=2}",
        "inline-trailing-comma": "a={b=1,}", "inline-newline": "a={\nb=1}",
        "inline-comment": "a={b=1,# comment\nc=2}",
        "inline-missing-comma": "a={b=1 c=2}",
        "array-missing-comma": "a=[1 2]", "array-leading-comma": "a=[,1]",
        "array-double-comma": "a=[1,,2]", "array-missing-close": "a=[1,2",
        "array-table-to-aot": "a=[]\n[[a]]", "inline-array-to-aot": "a=[{}]\n[[a]]",
        "table-to-aot": "[a]\n[[a]]", "aot-to-table": "[[a]]\n[a]",
        "aot-to-scalar": "[[a]]\n[a.b]\na=1\n[a]\n",
        "aot-child-scalar-conflict": "[[a]]\nb=1\n[[a.b]]",
        "aot-inline-sealed": "[[a]]\nb={}\n[a.b.c]",
        "aot-static-array-extension": "[[a]]\nb=[]\n[[a.b]]",
        "header-missing-close": "[a", "header-extra-close": "[a]]",
        "header-extra-text": "[a] junk", "header-newline": "[a\n]",
        "aot-missing-close": "[[a]", "aot-extra-close": "[[a]]]",
        "header-empty": "[]", "aot-empty": "[[]]",
        "basic-unclosed": 'a="unterminated', "literal-unclosed": "a='unterminated",
        "basic-newline": 'a="line\nbreak"', "literal-newline": "a='line\nbreak'",
        "basic-bare-cr": 'a="a\rb"', "literal-bare-cr": "a='a\rb'",
        "multiline-bare-cr": 'a="""a\rb"""',
        "multiline-literal-bare-cr": "a='''a\rb'''",
        "basic-unknown-escape": 'a="\\q"', "basic-x-escape": 'a="\\x41"',
        "basic-escape-eof": 'a="\\', "basic-short-unicode": 'a="\\u123"',
        "basic-invalid-unicode": 'a="\\uZZZZ"',
        "basic-surrogate-high": 'a="\\uD800"', "basic-surrogate-low": 'a="\\uDFFF"',
        "basic-surrogate-pair": 'a="\\uD83C\\uDF0D"',
        "basic-over-unicode": 'a="\\U00110000"',
        "basic-newline-escape": 'a="a\\\nb"',
        "multiline-backslash-space-no-newline": 'a="""x\\  y"""',
        "multiline-comment-continuation": 'a="""x\\ #comment\ny"""',
        "extra-basic-quote": 'a="""x""""""',
        "extra-literal-quote": "a='''x''''''",
        "bare-cr": "a=1\rb=2", "formfeed-whitespace": "a=\f1",
        "bom": "\ufeffa=1", "null-value": "a=null", "true-case": "a=True",
        "false-prefix": "a=falsehood", "false-suffix": "a=false0",
    }
    for name, source in invalids.items():
        invalid(name, source)
    invalid_numbers = ["00", "01", "+01", "-01", "_1", "1_", "1__0", "0_x1",
                       "0x", "0x_1", "0x1_", "0xGG", "0XFF", "-0x1", "+0x1",
                       "0o8", "0O7", "0b2", "0B1", "0b_1", "0o_1", "0x1__2",
                       ".1", "1.", "+.1", "1.e2", "01.0", "00e1", "1e", "1e+",
                       "1e-", "1_e2", "1e_2", "1e+_2", "1.0_", "1._0", "1.0__2",
                       "1.0e1_", "1f", "1e2x", "Inf", "NaN", "infinity", "nanx",
                       "--1", "+-1", "1+2", "0x1p0"]
    for spelling in invalid_numbers:
        invalid("number-invalid/" + spelling, "v=" + spelling)
    for spelling in ("-9223372036854775809", "18446744073709551616",
                     "0x10000000000000000", "0o2000000000000000000000", "0b1" + "0" * 64):
        invalid("number-range/" + spelling, "v=" + spelling, contract=True,
                diagnostic="integer|range|overflow")
    invalid_temporals = ["1979-00-01", "1979-13-01", "1979-02-29", "1900-02-29", "1979-04-31",
                         "1979-01-00", "1979-01-32", "1979-1-01", "1979-01-1",
                         "24:00:00", "00:60:00", "00:00:60", "00:00:61", "1:00:00", "00:00",
                         "00:00:00.", "00:00:00.1_2", "00:00:00Z",
                         "1979-05-27T24:00:00Z", "1979-05-27T00:00:00+24:00",
                         "1979-05-27T00:00:00+00:60", "1979-05-27T00:00:00+0:00",
                         "1979-05-27T00:00:00+0000", "1979-05-27T00:00:00ZZ",
                         "1979-05-27T00:00:00.Z", "1979-05-27T00:00:00Zx",
                         "1979-05-27  00:00:00Z", "1979-05-27\t00:00:00Z"]
    for spelling in invalid_temporals:
        invalid("temporal-invalid/" + spelling, "v=" + spelling)
    invalid("temporal/unsupported-leap-second", "v=2016-12-31T23:59:60Z",
            contract=True, diagnostic="second|time|date")
    for value in [*range(9), *range(11, 32), 127]:
        char = chr(value)
        for context, source in (
                ("comment", "a=1\n# forbidden " + char),
                ("basic", 'a="' + char + '"'),
                ("literal", "a='" + char + "'"),
                ("multiline", 'a="""' + char + '"""'),
                ("multiline-literal", "a='''" + char + "'''")):
            invalid(f"control/{context}-{value:02x}", source)

    # Absolute tree depth includes the root table at zero, every container,
    # every value, and both the array and element for each array of tables.
    valid("limit/depth-array-64", "v=" + "[" * 63 + "0" + "]" * 63)
    invalid("limit/depth-array-65", "v=" + "[" * 64 + "0" + "]" * 64,
            contract=True, diagnostic="depth|nest")
    valid("limit/depth-inline-64", "v=" + "{x=" * 63 + "0" + "}" * 63)
    invalid("limit/depth-inline-65", "v=" + "{x=" * 64 + "0" + "}" * 64,
            contract=True, diagnostic="depth|nest")
    valid("limit/depth-dotted-64", ".".join(["x"] * 64) + "=0")
    invalid("limit/depth-dotted-65", ".".join(["x"] * 65) + "=0",
            contract=True, diagnostic="depth|nest")
    valid("limit/depth-table-64", "[" + ".".join(["x"] * 64) + "]")
    invalid("limit/depth-table-65", "[" + ".".join(["x"] * 65) + "]",
            contract=True, diagnostic="depth|nest")
    aot = "\n".join("[[" + ".".join(["x"] * depth) + "]]" for depth in range(1, 33))
    valid("limit/depth-aot-64", aot)
    invalid("limit/depth-aot-value-65", aot + "\nv=0",
            contract=True, diagnostic="depth|nest")
    invalid("limit/depth-aot-66", aot + "\n[[" + ".".join(["x"] * 33) + "]]",
            contract=True, diagnostic="depth|nest")
    valid("limit/input-exact-4MiB", "#" + "x" * (INPUT_LIMIT - 1))
    invalid("limit/input-over-4MiB", "#" + "x" * INPUT_LIMIT,
            contract=True, diagnostic="4.*MiB|4194304|input.*limit|input.*large", location=False)
    valid("limit/nodes-exact-100000", "v=[" + ",".join(["0"] * (NODE_LIMIT - 2)) + "]")
    invalid("limit/nodes-over-100000", "v=[" + ",".join(["0"] * (NODE_LIMIT - 1)) + "]",
            contract=True, diagnostic="node|value.*limit|too many values")
    return cases


def unique_pairs(pairs):
    out = {}
    for key, value in pairs:
        if key in out:
            raise AssertionError(f"duplicate JSON output key {key!r}")
        out[key] = value
    return out


def decode_json(text):
    numbers = []

    def number(kind, spelling):
        numbers.append(spelling)
        return Number(kind, spelling)

    def constant(spelling):
        raise AssertionError(f"untagged non-JSON numeric constant {spelling}")

    value = json.loads(text, parse_int=lambda text: number("integer", text),
                       parse_float=lambda text: number("float", text),
                       parse_constant=constant, object_pairs_hook=unique_pairs)
    return value, numbers


def compare(actual, expected, path=()):
    at = repr(path)
    if isinstance(expected, Number):
        assert isinstance(actual, Number) and actual.kind == expected.kind, (at, expected, actual)
        assert Decimal(actual.spelling) == Decimal(expected.spelling), (at, expected, actual)
    elif isinstance(expected, Temporal):
        tag = "$toml_" + expected.kind
        assert type(actual) is dict and set(actual) == {tag}, (at, expected, actual)
        if expected.exact:
            assert actual[tag] == expected.spelling, (at, expected, actual)
        else:
            # toml-test's expected .600 may correspond to source .6. Compare
            # exact fractional value without losing digits through datetime.
            def normalized(spelling):
                spelling = spelling.replace("t", "T").replace("z", "Z").replace(" ", "T")
                spelling = re.sub(r"[+-]00:00$", "Z", spelling)
                return re.sub(r"\.([0-9]+)(?=Z|[+-][0-9]{2}:[0-9]{2}|$)",
                              lambda match: ("." + match[1].rstrip("0")) if match[1].rstrip("0") else "",
                              spelling)
            assert normalized(actual[tag]) == normalized(expected.spelling), (at, expected, actual)
    elif isinstance(expected, Decimal):
        if expected.is_infinite():
            assert actual == {"$toml_float": "-inf" if expected.is_signed() else "inf"}, (at, actual)
        elif expected.is_nan():
            assert actual == {"$toml_float": "nan"}, (at, actual)
        else:
            assert isinstance(actual, Number) and actual.kind == "float", (at, expected, actual)
            converted = Decimal(actual.spelling)
            assert converted == expected, (at, expected, actual)
            if expected.is_zero():
                assert converted.is_signed() == expected.is_signed(), (at, expected, actual)
    elif type(expected) is int:
        assert isinstance(actual, Number) and actual.kind == "integer", (at, expected, actual)
        assert int(actual.spelling) == expected, (at, expected, actual)
    elif isinstance(expected, (datetime.datetime, datetime.date, datetime.time)):
        # Corpus-specific explicit Temporal values check retained sub-microsecond
        # spelling. This branch handles incidental dates in external contracts.
        kind = "datetime" if isinstance(expected, datetime.datetime) else (
            "time" if isinstance(expected, datetime.time) else "date")
        assert type(actual) is dict and set(actual) == {"$toml_" + kind}, (at, expected, actual)
        spelling = actual["$toml_" + kind]
        assert type(expected).fromisoformat(spelling.replace("Z", "+00:00")) == expected, (at, expected, actual)
    elif type(expected) is dict:
        assert type(actual) is dict and actual.keys() == expected.keys(), (at, expected, actual)
        for key, value in expected.items():
            compare(actual[key], value, (*path, key))
    elif type(expected) is list:
        assert type(actual) is list and len(actual) == len(expected), (at, expected, actual)
        for index, value in enumerate(expected):
            compare(actual[index], value, (*path, index))
    else:
        assert type(actual) is type(expected) and actual == expected, (at, expected, actual)


def tagged_oracle(value):
    """Translate upstream's independent expected typed JSON into our oracle."""
    if type(value) is list:
        return [tagged_oracle(item) for item in value]
    if type(value) is dict and set(value) == {"type", "value"}:
        kind, spelling = value["type"], value["value"]
        if kind == "integer":
            return int(spelling)
        if kind == "float":
            return Decimal(spelling)
        if kind == "bool":
            assert spelling in ("true", "false"), value
            return spelling == "true"
        if kind == "string":
            return spelling
        names = {"datetime": "datetime", "datetime-local": "datetime",
                 "date-local": "date", "time-local": "time"}
        assert kind in names, value
        return Temporal(names[kind], spelling, exact=False)
    assert type(value) is dict, value
    return {key: tagged_oracle(item) for key, item in value.items()}


def upstream_cases(directory):
    root = directory / "tests" if (directory / "tests").is_dir() else directory
    manifest = root / "files-toml-1.0.0"
    cases = []
    for name in manifest.read_text().splitlines():
        if not name.endswith(".toml"):
            continue
        path = root / name
        raw = path.read_bytes()
        valid = name.startswith("valid/")
        try:
            text = raw.decode("utf-8")
        except UnicodeDecodeError:
            assert not valid, name
            cases.append(Case("upstream/" + name, raw, False, invalid_utf8=True,
                              oracle="toml-test invalid UTF-8 / Dunder string creation boundary"))
            continue
        if valid:
            expected = tagged_oracle(json.loads(path.with_suffix(".json").read_text()))
            # Upstream JSON supplies an independent oracle including types and
            # full temporal precision. tomllib separately confirms the grammar.
            parse_oracle(text)
            cases.append(Case("upstream/" + name, raw, True, expected,
                              oracle="toml-test typed JSON and Python tomllib"))
        else:
            oracle = "toml-test invalid and Python tomllib"
            try:
                parse_oracle(text)
            except tomllib.TOMLDecodeError:
                pass
            else:
                if not ORACLE_TOML_1_1:
                    raise AssertionError(f"upstream invalid disagrees with tomllib: {name}")
                oracle = "toml-test TOML 1.0 manifest; Python tomllib implements TOML 1.1"
            cases.append(Case("upstream/" + name, raw, False,
                              oracle=oracle))
    return cases


def contract_cases(paths):
    cases = []
    for path in paths:
        document = json.loads(path.read_text())
        for item in document["cases"]:
            name = "contract/" + path.stem + "/" + item["name"]
            text = item["text"]
            ok = item["parse_ok"]
            if ok:
                expected, _ = decode_json(item["expected_json"])
                compare(expected, parse_oracle(text))
                cases.append(Case(name, text.encode(), True, expected,
                                  oracle="actual Snus contract plus Python tomllib"))
            else:
                try:
                    parse_oracle(text)
                except tomllib.TOMLDecodeError:
                    pass
                else:
                    raise AssertionError(f"parser-negative contract accepted by tomllib: {name}")
                cases.append(Case(name, text.encode(), False,
                                  oracle="actual Snus parser rejection plus Python tomllib"))
    return cases


def verify(case, record):
    assert record["source_ok"] != case.invalid_utf8, (case.name, record)
    assert record["ok"] is case.ok, (case.name, case.oracle, record)
    if not case.ok:
        assert record["json"] == "" and record["numbers"] == [], (case.name, record)
        assert record["json_ok"] is False, (case.name, record)
        assert type(record["error"]) is str and record["error"], (case.name, record)
        if case.invalid_utf8:
            assert "UTF-8" in record["error"], (case.name, record)
        elif case.location:
            match = re.search(r"TOML line ([1-9][0-9]*), column ([1-9][0-9]*): .+", record["error"])
            assert match, (case.name, record["error"])
            line, column = map(int, match.groups())
            lines = case.source.decode().split("\n")
            assert line <= len(lines), (case.name, line, len(lines), record["error"])
            assert column <= len(lines[line - 1].encode()) + 1, (case.name, line, column, record["error"])
        if case.diagnostic:
            assert re.search(case.diagnostic, record["error"], re.IGNORECASE), (case.name, record["error"])
        return
    assert record["source_ok"] is True and record["error"] == "", (case.name, record)
    assert record["json_ok"] is True, (case.name, record)
    actual, numeric_spans = decode_json(record["json"])
    # Dunder's independent JSON span tape and Python's lexical hooks must see
    # exactly the same number tokens, without conversion through binary64.
    assert numeric_spans == record["numbers"], (case.name, numeric_spans, record["numbers"])
    compare(actual, case.expected)
    for path, spelling in case.lexemes.items():
        value = actual
        for segment in path:
            value = value[segment]
        assert isinstance(value, Number) and value.spelling == spelling, (case.name, path, spelling, value)


def run(command, *, timeout=120):
    result = subprocess.run(list(map(str, command)), cwd=ROOT, capture_output=True,
                            timeout=timeout)
    assert result.returncode == 0, (list(map(str, command)), result.returncode,
                                    result.stdout.decode(errors="replace"),
                                    result.stderr.decode(errors="replace"))
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("compiler", type=Path)
    parser.add_argument("--runner", action="append", default=[], metavar="TARGET=COMMAND",
                        help="also execute another target through a local wrapper or emulator")
    parser.add_argument("--toml-test", type=Path,
                        help="optional pinned toml-lang/toml-test checkout or extracted tests directory")
    parser.add_argument("--contracts", action="append", type=Path, default=[],
                        help="additional extracted Snus *-parse.json file (repeatable)")
    parser.add_argument("--work", type=Path,
                        help="keep images, source cases and result records in this directory")
    parser.add_argument("--only", help="run names containing this substring")
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    host = host_target()
    runners = {host: []}
    for specification in args.runner:
        target, separator, command = specification.partition("=")
        if not separator or target not in TARGETS or not shlex.split(command):
            parser.error("--runner requires native TARGET=COMMAND")
        runners[target] = shlex.split(command)
    cases = grammar_cases()
    corpus = ROOT / "tests/fixtures/toml-snus-corpus.json"
    if corpus.exists():
        cases.extend(contract_cases([corpus]))
    cases.extend(contract_cases(args.contracts))
    if args.toml_test:
        cases.extend(upstream_cases(args.toml_test.resolve(strict=True)))
    if args.only:
        cases = [case for case in cases if args.only in case.name]
    assert cases, "no selected TOML cases"
    assert len({case.name for case in cases}) == len(cases), "duplicate case names"

    def exercise(work):
        paths = []
        manifest = []
        for index, case in enumerate(cases):
            path = work / f"case-{index:04}.toml"
            path.write_bytes(case.source)
            paths.append(path)
            manifest.append({"name": case.name, "source_file": path.name,
                             "source_sha256": hashlib.sha256(case.source).hexdigest(),
                             "parse_ok": case.ok, "invalid_utf8": case.invalid_utf8,
                             "oracle": case.oracle})
        manifest_text = json.dumps(manifest, ensure_ascii=True, indent=2) + "\n"
        (work / "cases.json").write_text(manifest_text)
        builds = executions = assertions = 0
        images = {}
        for target in TARGETS:
            image = work / ("toml-driver-" + target)
            run([compiler, "build", ROOT / "tests/fixtures/toml-driver.ddr",
                 "--target", target, "-o", image], timeout=300)
            builds += 1
            images[target] = hashlib.sha256(image.read_bytes()).hexdigest()
            if target not in runners:
                continue
            records = []
            # A bounded argument batch works with ordinary argv and local
            # containers, without shell input interpolation or recompilation.
            for start in range(0, len(cases), 100):
                selected = paths[start:start + 100]
                result = run([*runners[target], image, *selected], timeout=120)
                assert not result.stderr, (target, result.stderr)
                assert result.stdout.endswith(b"\n"), (target, start, "missing protocol terminator")
                # JSON may contain literal U+0085/U+2028/U+2029 inside a
                # string. They are not protocol LF bytes or record boundaries.
                lines = result.stdout[:-1].split(b"\n")
                assert len(lines) == len(selected), (target, start, len(lines), len(selected))
                for offset, line in enumerate(lines):
                    index = start + offset
                    record = json.loads(line, object_pairs_hook=unique_pairs)
                    assert record["file"] == str(paths[index]), (target, index, record["file"])
                    try:
                        verify(cases[index], record)
                    except AssertionError as failure:
                        raise AssertionError((target, cases[index].name, failure.args)) from failure
                    assertions += 1
                    records.append({"case": cases[index].name, **record})
            (work / ("results-" + target + ".json")).write_text(
                json.dumps(records, ensure_ascii=True, indent=2) + "\n")
            executions += 1
            invalid_utf8 = sum(case.invalid_utf8 for case in cases)
            print(f"PASS {target}: {len(cases) - invalid_utf8} TOML parser cases, "
                  f"{invalid_utf8} UTF-8 input-boundary cases", flush=True)
        summary = {"cases": len(cases), "accepted": sum(case.ok for case in cases),
                   "rejected": sum(not case.ok for case in cases),
                   "utf8_boundaries": sum(case.invalid_utf8 for case in cases),
                   "images_built": builds, "targets_executed": executions,
                   "case_assertions": assertions, "executed_targets": list(runners),
                   "compiler": str(compiler),
                   "compiler_sha256": hashlib.sha256(compiler.read_bytes()).hexdigest(),
                   "driver_source_sha256": hashlib.sha256((ROOT / "tests/fixtures/toml-driver.ddr").read_bytes()).hexdigest(),
                   "case_manifest_sha256": hashlib.sha256(manifest_text.encode()).hexdigest(),
                   "image_sha256": images, "python": sys.version,
                   "tomllib_implements_1_1": ORACLE_TOML_1_1,
                   "known_policy": ["positive integers through u64 max",
                                    "year 0000 retained lexically",
                                    "leap seconds rejected with a diagnostic"],
                   "upstream": str(args.toml_test) if args.toml_test else None}
        (work / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
        print(f"TOML: {builds} images built once each; {executions} target runtimes, "
              f"{assertions} case results; exact numeric kinds/spans, Unicode, tables, "
              "arrays of tables, sealing, diagnostics and limits passed")

    if args.work:
        work = args.work.resolve()
        work.mkdir(parents=True, exist_ok=True)
        exercise(work)
    else:
        # Keep temporary files inside the checkout for local container wrappers
        # which expose this workspace at its existing absolute path.
        base = ROOT / "build"
        base.mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="toml-acceptance-", dir=base) as temporary:
            exercise(Path(temporary).resolve())


if __name__ == "__main__":
    main()
