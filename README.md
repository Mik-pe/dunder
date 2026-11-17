# Dunder ⚡

<p align="center">
  <img src="docs/dunder.jpg" width="320" alt="Dunders logga med ett stentroll och en gyllene blixt">
</p>

Dunder är ett statiskt typat språk med filändelsen `.ddr`. Kompilatorn är
skriven i Dunder och bygger program för ARM64/macOS, ARM64/Linux, x86-64/Linux
och WebAssembly. För nativa program sköter den också länkningen och signerar
macOS-binärer.

```dunder
@test {
    ("Ada") -> "Hej, Ada!"
}
fn greet(name: string) -> string {
    f"Hej, {name}!"
}

fn main() {
    print(greet("världen"))
}
```

## Bygg och kör

Kör från repots rot på någon av de tre nativa plattformarna. Frökompilatorn
för värden följer med repot och kan bygga kompilatorn offline.

```sh
dunder_bin="$(./tools/bootstrap.sh)"
"$dunder_bin" run examples/hello.ddr
"$dunder_bin" test
```

`examples/hello.ddr` skriver `42` och `true` på varsin rad. Utan filargument använder
`build` och `test` närmaste `dunder.toml`. I det här projektet anger
manifestet det programmet och `tests/suite.ddr` som testrot.
[Projektmanifest och beroenden](docs/dependencies.md)

```sh
"$dunder_bin" check examples/modules/math.ddr
"$dunder_bin" build examples/hello.ddr --target x86_64-linux -o build/hello-linux
```

`check` typkontrollerar även biblioteksmoduler. Alla fyra målen kan byggas
med samma kompilator. [Målplattformar och körmiljöer](docs/targets.md)

För en permanent installation, kör `./tools/install.sh` och lägg
`~/.dunder/bin` i `PATH`. `DUNDER_HOME` väljer installationskatalog.
`dunder upgrade --check` kontrollerar aktuell release och `dunder upgrade`
installerar den. [Releaser](docs/releases.md)

## Språket och biblioteken

- [Syntax](docs/syntax.md) beskriver funktioner, moduler, typer och tester. [Små körbara exempel](examples/)
- [Minnesmodellen](docs/memory.md) beskriver ägarskap, flyttar och lån.
- [Standardbiblioteket](docs/standard.md) omfattar bland annat samlingar, text och serialisering. [Systembibliotek](docs/sys.md) · [HTTP och TLS](docs/http.md) · [SQLite](docs/sqlite.md) · [TOML](docs/toml.md) · [Kryptografi](docs/crypto.md)
- [C-FFI](docs/ffi.md) beskriver externa anrop och datalayout. [Parallella uppgifter](docs/tasks.md)

Tabell, HTTP-servern och grafikexemplen för Metal, Vulkan och WebGPU finns i
[dunder-hundmat](https://github.com/Mik-pe/dunder-hundmat).

## Kompilatorutveckling

`./tools/dev.sh` gör ett inkrementellt bygge och kör kärntesterna. Kör
`./tools/verify-selfhost.sh` för ett fullständigt självbygge och verifiering av
kompilatorn, målprogrammen och diagnostiken. Sviten använder Python 3.11 eller
senare, Node för Wasm, en C-kompilator för ABI-proven samt Git, curl, gzip och OpenSSL.

[Guide till kompilatorkoden](selfhost/README.md) · [Bootstrap från frö](bootstrap/README.md) · [Inkrementell kompilering](docs/incremental.md) · [Prestandamätningar](benchmarks/README.md) · [Aktuellt arbete](docs/next-steps.md)
