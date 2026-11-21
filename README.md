# Dunder ⚡

<p align="center">
  <img src="docs/dunder.jpg" width="320" alt="Dunders logga med ett stentroll och en gyllene blixt">
</p>

Dunder är ett statiskt typat språk med filändelsen `.ddr`. Kompilatorn är
skriven i Dunder och bygger program för ARM64/macOS, ARM64/Linux, x86-64/Linux,
Windows x86-64 (`x86_64-windows`), WebAssembly (`wasm32`) och WASI (`wasm32-wasi`).
För nativa program sköter den också länkningen, valfri DWARF-debuginformation (`-g`)
och macOS-signering, och levereras med formaterare (`dunder fmt`) samt inbyggd
språkserver (`dunder lsp`).

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

`check` typkontrollerar snabbt med flera fel, varningar och `--agent`-diagnos.
Alla sex målen kan byggas med samma kompilator. [Målplattformar och körmiljöer](docs/targets.md)
`build --app` paketerar ett ARM64/macOS-program som en signerad `.app`.
[App-paket för macOS](docs/app-bundles.md)

För en permanent installation, kör `./tools/install.sh` och lägg
`~/.dunder/bin` i `PATH`. `DUNDER_HOME` väljer installationskatalog.
`dunder upgrade --check` kontrollerar aktuell release och `dunder upgrade`
installerar den. [Releaser](docs/releases.md)

## Språket och biblioteken

- [Syntax](docs/syntax.md) beskriver funktioner, moduler, traits, råa/flerradiga strängar, `@embed_file`, `&str` och tester. [Små körbara exempel](examples/)
- [Minnesmodellen](docs/memory.md) beskriver ägarskap, flyttar, dynamiska slabs och begränsade lån (`&[]T`, `&str`).
- [Standardbiblioteket](docs/standard.md) omfattar bland annat samlingar, `std.sync`, `std.text.view`, `std.path`, `std.time`, `std.signal` och serialisering. [Systembibliotek](docs/sys.md) · [HTTP och TLS](docs/http.md) · [SQLite](docs/sqlite.md) · [TOML](docs/toml.md) · [Kryptografi](docs/crypto.md)
- [Ekosystempaketen](packages/) innehåller fristående paket för [`http`](packages/http/), [`http_client`](packages/http_client/), [`cli`](packages/cli/) och [`log`](packages/log/).
- [C-FFI](docs/ffi.md) beskriver externa anrop och datalayout. [Parallella uppgifter och synkronisering](docs/tasks.md)

Hundmat är Dunders testbädd. Tabell, HTTP-servern och grafikexemplen för
Metal, Vulkan och WebGPU låter oss prova språket, kompilatorn och
standardbiblioteket i riktiga program. [dunder-hundmat](https://github.com/Mik-pe/dunder-hundmat)

## Kompilatorutveckling

`./tools/dev.sh` gör ett inkrementellt bygge och kör kärntesterna. Kör
`./tools/verify-selfhost.sh` för ett fullständigt självbygge och verifiering av
kompilatorn, målprogrammen och diagnostiken. Sviten använder Python 3.11 eller
senare, Node för Wasm, en C-kompilator för ABI-proven samt Git, curl, gzip och OpenSSL.

[Guide till kompilatorkoden](selfhost/README.md) · [Arkitekturöversikt](docs/arkitektur.md) · [Bootstrap från frö](bootstrap/README.md) · [Inkrementell kompilering](docs/incremental.md) · [Aktuellt arbete](docs/next-steps.md)
