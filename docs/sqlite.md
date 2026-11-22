# SQLite

`sqlite` is a source package for the system SQLite 3 library on native
macOS and Linux. To migrate from `import std.sqlite as sqlite`, add
the package to the application's `dunder.toml` and use `import sqlite`.
With a sibling Dunder checkout, the dependency is

```toml
[dependencies]
sqlite = { path = "../dunder/packages/sqlite", capabilities = ["filesystem", "unsafe"] }
```

A Git dependency can select the same directory with `subdir = "packages/sqlite"`.
The package uses FFI internally. Opening a database requires `capability filesystem`
in the calling module. The opaque connection is a
`@resource(dispose)`: move it to transfer ownership and call `close()` when
finished. Closing an already closed connection succeeds; closing while a
transaction is still active fails.

```dunder
capability filesystem
import sqlite

fn main() {
    mut db := sqlite.Database::open("application.db", false).unwrap()
    defer { _ := db.close() }
    none: []sqlite.SqlValue = []
    _ := db.exec("CREATE TABLE IF NOT EXISTS items(value TEXT)", &none).unwrap()
    params: []sqlite.SqlValue = [.Text("a value with ' quotes")]
    inserted := db.exec("INSERT INTO items(value) VALUES(?)", &params).unwrap()
    assert(inserted.changes == 1)
    rows := db.query("SELECT rowid AS id, value FROM items", &none).unwrap()
    for row in &rows {
        id := row.field_i64("id").unwrap()
        assert(id >= 1)
        text := row.field_text("value").unwrap()
        print(text)
    }
}
```

All operations return `Result<T, SqliteError>`; `SqliteError` carries `code`
and `message` (with `Display` and `Eq`). `Database::open(path, read_only)`
returns `Result<Database, SqliteError>`. Read-only connections use SQLite's
`READONLY` flag and enable `PRAGMA query_only=ON`. Every connection uses a
five-second busy timeout and `FULLMUTEX`.

Parameters are `&[]SqlValue` with the enum variants `Null`, `Int(i64)`,
`Real(f64)`, `Text(string)` and `Blob([]u8)` (with `is_null`/`as_i64`/`as_f64`/
`as_text`/`as_blob` returning `Option`). Binding uses explicit byte lengths
and `SQLITE_TRANSIENT`; an empty `Blob` binds an empty BLOB rather than SQL
NULL. Parameter counts must match exactly, and only one statement is accepted
per call, including a complete trigger definition.

`exec(sql, &params)` returns `Result<ExecInfo, SqliteError>` with `changes`
and `last_id` (64-bit). `query(sql, &params)` returns `Result<[]Row,
SqliteError>` and accepts only read-only statements. `query_one` returns
`Result<Option<Row>, SqliteError>` and fails if more than one row matches.
`execute_rows` deliberately permits a statement with effects that returns
rows, such as `INSERT ... RETURNING` or `PRAGMA wal_checkpoint(TRUNCATE)`.
`exec_batch(sql)` runs a multi-statement script without parameters and
returns `Result<(), SqliteError>`.

Rows are read directly as typed values: `Row::len`, `column_name`,
`column_index`, `value(index)` / `by_name(name)` (returning `Option<SqlValue>`),
plus `is_null`, `get_i64`, `get_f64`, `get_text`, `get_blob` and the
`field_*` by-name variants (returning `Result<_, SqliteError>`). Integer
values retain all 64 bits, REAL values their double precision, TEXT preserves
embedded NUL and is validated as UTF-8, BLOB becomes `[]u8`, and SQL NULL
becomes `SqlValue::Null`. Query failure returns no partial rows.

`db.begin()` / `db.begin_immediate()` return `Result<Transaction,
SqliteError>`. A `Transaction` offers the same `exec`/`query`/`query_one`/
`execute_rows`/`exec_batch`, plus `commit()` and `rollback()`; dropping an
active transaction rolls it back automatically (`dispose`). Only one
transaction can be active per connection.

Limits are 4 KiB per path, 1 MiB per statement, 1,024 parameters, 16 MiB per
TEXT parameter or column, 64 MiB per BLOB parameter or column, 10,000 rows,
64 MiB of accumulated results, and 1,024 statements per batch. A trigger may
contain up to 256 statement terminators. Statements and paths cannot contain
NUL. Statement boundaries are checked before preparation, including for
PRAGMAs that can take effect during compilation; a failed tail or parameter
check does not execute the statement. Each prepared statement is finalized on
both success and error.

Native dependencies are `/usr/lib/libsqlite3.dylib` on macOS and
`libsqlite3.so.0` on Linux. The compiler links those shared libraries
directly; SQLite queries do not spawn a command-line process. Test with
`/absolute/path/to/dunder run tools/test_sqlite.ddr -- /absolute/path/to/dunder`. It builds all three
native targets and executes the host; use `--runner TARGET=COMMAND` for
additional available runtimes.
