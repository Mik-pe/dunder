# SQLite

`std.sqlite` exposes the system SQLite 3 library on native macOS and Linux.
The application grants filesystem access when opening a connection. The
opaque connection is a resource: move it to transfer ownership and call
`close()` when finished. Closing an already closed connection succeeds.

```dunder
capability filesystem
import std.sqlite as sqlite

fn main() {
    opened := sqlite.Database::open("application.db", false)
    assert(opened.ok)
    mut db := opened.db
    defer { _ := db.close() }
    params: []string = ["a value with ' quotes"]
    none: []string = []
    assert(db.exec("CREATE TABLE IF NOT EXISTS items(value TEXT)", &none).ok)
    inserted := db.exec("INSERT INTO items(value) VALUES(?)", &params)
    assert(inserted.ok && inserted.changes == 1)
    rows := db.query("SELECT rowid AS id, value FROM items", &none)
    assert(rows.ok)
    for row in &rows.rows { print(row) }
}
```

`Database::open(path, read_only)` returns `OpenResult { ok, db, error }`.
Read-only connections use SQLite's `READONLY` flag and enable `query_only`.
Every connection uses a five-second busy timeout and `FULLMUTEX`.

`exec(sql, &params)` returns `ExecResult { ok, changes, last_id, error }`.
`query(sql, &params)` returns `QueryResult { ok, rows, error }`. It accepts
read-only statements and returns each row as a JSON object string. Use column
aliases to choose object keys. Integer values retain all 64 bits, finite REAL
values retain their double precision, and TEXT preserves embedded NUL and
UTF-8. SQL NULL becomes JSON null. BLOB and non-finite REAL values are rejected;
explicitly select `hex(blob_column)` when a text encoding is wanted.

`execute_rows(sql, &params)` deliberately permits a statement with effects
that returns rows, such as `INSERT ... RETURNING` or
`PRAGMA wal_checkpoint(TRUNCATE)`. It retains the same single-statement,
binding, row-encoding and finalization rules. Use `query` when reads alone
are intended.

`exec_blob(sql, &params, blob_index, &data)` returns `ExecResult` and binds
one binary value at a one-based placeholder index. `params` contains the
other text parameters in order. Binary data is bounded to 64 MiB; an empty
buffer binds an empty BLOB rather than SQL NULL.

Parameters bind text with explicit byte lengths and `SQLITE_TRANSIENT`.
SQLite applies the usual column affinity; SQL NULL/numbers can also appear
directly in trusted SQL expressions. Parameter counts must match exactly.
Only one statement is accepted, including a complete trigger definition.
Statement boundaries are checked before preparation, including for PRAGMAs
that can take effect during compilation. A failed tail or parameter check
does not execute the statement. Each prepared statement is finalized on
both success and error. Query failure returns no partial rows.

Limits are 4 KiB per path, 1 MiB per statement, 1,024 parameters, 16 MiB per
parameter or TEXT column, 10,000 rows, and 64 MiB of serialized results.
A trigger may contain up to 256 statement terminators. Statements and paths
cannot contain NUL. TEXT containing invalid UTF-8 is rejected.

Native dependencies are `/usr/lib/libsqlite3.dylib` on macOS and
`libsqlite3.so.0` on Linux. The compiler links those shared libraries directly;
SQLite queries do not spawn a command-line process. Test with
`python3 tools/test-sqlite.py /absolute/path/to/dunder`. It builds all three
native targets and executes the host; use `--runner TARGET=COMMAND` for
additional available runtimes.
