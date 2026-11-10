"""Exercise the standalone Tabell executable and its CSV contract."""
import argparse
import csv
import io
from pathlib import Path
import subprocess
import tempfile

PROJECT = Path(__file__).resolve().parents[1] / "projects/tabell"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("compiler", type=Path)
    compiler = parser.parse_args().compiler.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix="dunder-tabell-") as tmp:
        work = Path(tmp)
        binary = work / "tabell"
        subprocess.run([compiler, "build", "-o", binary], cwd=PROJECT, check=True,
                       env={"PATH": ""}, timeout=60)
        subprocess.run([compiler, "test", "-o", work / "tests"], cwd=PROJECT,
                       check=True, env={"PATH": ""}, timeout=90)

        def run(args, code=0):
            result = subprocess.run([binary, *args], cwd=work, capture_output=True,
                                    env={}, timeout=15)
            assert result.returncode == code, (args, result)
            if code:
                assert result.stderr and not result.stdout, result
                if code == 1:
                    assert result.stderr.startswith(b"tabell:"), result
            else:
                assert not result.stderr, result
            return result

        assert run([PROJECT / "expenses.csv", "team", "cents"]).stdout == (PROJECT / "expected.csv").read_bytes()
        path = work / "rapport å 🌍.csv"
        keys = ["", "alpha", "Åsa", "comma,value", 'quote"value', "line\nvalue", "emoji 🌍", "Örn"]
        totals = {}
        with path.open("w", encoding="utf-8-sig", newline="") as stream:
            writer = csv.writer(stream)
            writer.writerow(["memo", "amount", "group"])
            for i in range(512):
                key = keys[(i * 17 + i // 7) % len(keys)]
                amount = (i % 13 - 6) * 2_000_000_003
                writer.writerow([f"ignored, {i}", amount, key])
                totals[key] = totals.get(key, 0) + amount
        expected = io.StringIO(newline="")
        writer = csv.writer(expected, lineterminator="\n")
        writer.writerow(["group", "total"])
        writer.writerows(sorted(totals.items()))
        args = [path, "group", "amount"]
        assert run(args).stdout == expected.getvalue().encode()
        for contents, line in [
            (b"group,amount\na,9223372036854775807\na,1", 3),
            (b'group,amount\r\n"first\r\nsecond",3\r\nbad,x\r\n', 4),
            (b'group,amount\n"first\nsecond"x,1', 3),
            (b"group,amount\na,1,extra", 2),
        ]:
            path.write_bytes(contents)
            assert run(args, 1).stderr.startswith(f"tabell: line {line}:".encode())
        path.write_bytes(b"group,amount\n\xff,1")
        run(args, 1)
        path.unlink()
        run(args, 1)
        run([], 2)
        assert run(["--help"]).stdout
    print("Tabell CSV output, grouping, input errors and exit codes passed")


if __name__ == "__main__":
    main()
