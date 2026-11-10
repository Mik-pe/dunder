# Dunder ⚡

Ett statiskt typat, kompilerat språk med filändelsen **`.dn`**.
Den första kompilatorn läser vanlig
Dunder-källkod och skriver en körbar ARM64/Mach-O-fil för macOS, inklusive
ad hoc-signatur. Kodgenerering, bildskrivning och signering görs i kompilatorn.

```dunder
fn add(a: i32, b: i32) -> i32 {
    return a + b
}

fn main() {
    answer := add(40, 2)
    expected: i32 = 42
    print(answer)
    print(answer == expected)

    var count := 0
    while count < 3 {
        print(count)
        count = count + 1
    }
}
```

`:=` deklarerar med härledd typ. `: i32 =` deklarerar med explicit typ.
Värden och parametrar är oföränderliga som standard; `var` tillåter senare
tilldelning med `=`. En oföränderlig bindning kan få sitt värde vid körning;
den behöver inte vara en kompileringstidskonstant.

## Bygg och kör

Bygg bootstrapkompilatorn med systemets Rust, utan externa crates:

```sh
cargo build --release
cargo run --release -- build examples/hello.dn -o build/hello --timings
./build/hello
# 42
# true
```

Den byggda `dunder`-kompilatorn behöver inga externa kompilatorer, Python,
assembler, länkare eller signeringsverktyg för att bygga Dunder-källkod.
Den färdiga binären körs som en vanlig macOS-process. Systemets verktyg får
användas för oberoende verifiering; `cargo test` kontrollerar bland annat
signaturerna med `codesign --verify` och kör riktiga binärer.

```sh
cargo run --release -- check examples/fibonacci.dn
cargo run --release -- build examples/fibonacci.dn -o build/fibonacci
./build/fibonacci
# 6765
# true
```

`dunder run` bygger och kör programmet direkt. `fn main()` ger exitkod 0;
`fn main() -> i32` låter programmet välja exitkod med `return`.
Kör `cargo run --release -- run examples/functions.dn` för att prova
funktioner, callbacks och funktioner som returvärden. Exemplet skriver
`42`, `42` och `-42`.

Funktioner kan vara värden med statiskt kontrollerad signatur:

```dunder
fn apply(transform: fn(i32) -> i32, value: i32) -> i32 {
    return transform(value)
}
```

Se [funktionsexemplet](examples/functions.dn) för en funktion som
returnerar en annan funktion, lagring i lokala variabler och callbackanrop.

`dunder build` utan fil hittar närmaste `dunder.toml` uppåt från arbetskatalogen:

```toml
name = "hello"
root = "examples/hello.dn"
```

Utdata hamnar då i `build/arm64-macos/hello`. Manifestets första delmängd
består av dessa två strängfält. Kompileringsfel har fil, rad, kolumn och
felkod. Ett misslyckat bygge lämnar tidigare utdata orörd.

## Implementerat och återstående

Implementerat: `i32`, `i64`, `bool`, typade funktioner och funktionsvärden,
callbacks, funktioner som returvärden, framåtanrop och rekursion,
härledda/explicit typade lokaler, oföränderlighet, `var`, blockscope, `if/else`,
`while`, `break/continue`, kortslutande booleska operatorer, heltalsaritmetik,
kontrollerade heltalskonverteringar och `print`. Läs [syntaxen](docs/syntax.md).

Anrop stöder både register- och stackargument; fler än åtta parametrar fungerar.
Funktionstyper kontrolleras strukturellt. Anonyma funktioner och closures med
infångade lokaler är inte implementerade. Backend prioriterar korrekt typning
och körbar kod; optimering och byggprestanda är inte uppmätta.
Bootstrapkompilatorn är skriven i Rust och är ännu inte självhostad. Strängar,
poster, moduler, arrayer/slices och ägande/lån är kommande språksteg.

[Aktuellt arbete](docs/status.md) · [Arkitektur](docs/arkitektur.md) ·
[Prestandamål och mätkontrakt](docs/prestanda.md)
