# Dunder ⚡

Ett statiskt typat, kompilerat språk med filändelsen **`.dn`**.
Den första kompilatorn läser vanlig
Dunder-källkod och skriver en körbar ARM64/Mach-O-fil för macOS, inklusive
ad hoc-signatur. Kodgenerering, bildskrivning och signering görs i kompilatorn.

```dunder
fn add(a: i32, b: i32) -> i32 {
    a + b
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
test_root = "dogfood/tests.dn"
```

Utdata hamnar då i `build/arm64-macos/hello`. `name` och `root` krävs;
`test_root` är valfri och väljer rotfil för `dunder test`. Utan den används
`root` även för tester. Kompileringsfel har fil, rad, kolumn och
felkod. Ett misslyckat bygge lämnar tidigare utdata orörd.

## Moduler

En `.dn`-fil är en modul. Importen skrivs **utan filändelse** och söks
relativt filen som importerar:

```dunder
// math.dn
pub fn add(a: i32, b: i32) -> i32 {
    return a + b
}
```

```dunder
// main.dn
import math

fn main() {
    print(math.add(40, 2))
}
```

`fn` är privat inom modulen; `pub fn` exporterar funktionen. `import math`
läser `math.dn`. Undermappar och alias skrivs exempelvis
`import "util/predicates" as predicates`. `import "math"` stöds också.
Ingen importform innehåller `.dn`.

Typer följer samma regel: `struct Point` är privat och `pub struct Point`
exporteras. En importerad typ skrivs exempelvis `geometry.Point`, både i
signaturer och i uttrycket `geometry.Point { x: 3, y: 4 }`.

Kör [exemplet med tre moduler](examples/modules/main.dn):

```sh
cargo run --release -- run examples/modules/main.dn
# 42
# true
# 42
```

Importer är explicita och läses bara en gång per verklig fil. Funktioner
med samma namn i olika moduler hålls åtskilda. Alla signaturer samlas före
typkontroll av kropparna, så ömsesidig rekursion fungerar även över
cirkulära importer. Endast rotmodulens `main` är programstart.

Hela modulträdet byggs till samma körbara fil med Dunders egen kodgenerering,
symbolupplösning, Mach-O-skrivning och signering. Dunder-kompileringen anropar
varken GCC, LLVM, extern assembler, extern länkare eller extern signerare.
Rust används fortfarande för att bygga själva bootstrapkompilatorn.

## Tester och struct-uttryck

Testblock sitter vid funktionen de provar. Argument och förväntat värde
är vanliga uttryck, även när argumentet är en struct:

```dunder
struct Point {
    x: i32,
    y: i32,
}

@test {
    (Point { x: 3, y: 4 }) -> 25
    (Point { x: 6 / 2, y: 2 + 2 }) -> 20 + 5

    point := Point { x: 3, y: 4 }
    length_squared(point) == 25
}
fn length_squared(p: Point) -> i32 {
    p.x * p.x + p.y * p.y
}
```

`(argument) -> förväntat` anropar den annoterade funktionen och kontrollerar
resultatet. Flera argument skrivs `(a, b) -> förväntat`. Testblocket får
också innehålla lokala variabler, anrop och loopar. Fristående bool-uttryck
kontrolleras som assertions. `@test fn name() { ... }` fungerar för fristående
tester. Funktionens sista uttryck utan semikolon är dess returvärde;
explicit `return` fungerar också.

```sh
cargo run --release -- test
cargo run --release -- test dogfood/geometry.dn
cargo run --release -- run dogfood/structs.dn
# 25
# 32
# 3
```

`dunder test` hittar tester i hela det explicit importerade modulträdet.
Varje testblock körs som en egen nativeprocess med fem sekunders tidsgräns;
nästa test körs även om ett tidigare misslyckas. `main` körs inte och behövs
inte i en testrot. Testfunktioner ingår bara i testbyggen.

## Arrayer, loopar och dogfooding

```dunder
fn main() {
    var values: [i32; 8] = [0; 8]
    for i in 0..len(values) {
        values[i] = i + 1
    }
    var sum := 0
    for value in values.iter() {
        sum = sum + value
    }
    assert(sum == 36)
    print(sum)
}
```

Arrayer och structar kopieras som värden, även vid funktionsanrop. Fält och
element är oföränderliga om bindningen saknar `var`. `for` stöder halvöppna
`i32`-intervall, arrayer och arrayiteratorer. Iteration över arrayer använder
en kopia av värdena vid loopstart; `.iter()` skapar också en egen kopia.
Index kontrolleras vid körning.

[Dogfood-programmen](dogfood/README.md) provar rekursion, moduler, sortering,
callbacks, iteratorer, structar och en indexbaserad arena över en fast typad
buffert. Arenan är ett fungerande bump-allokeringsprogram; språket har ännu
inget allmänt allokerings- eller låne-API.

## UTF-8-strängar

```dunder
@test {
    ("världen") -> "Hej, världen!"
}
fn greet(name: string) -> string {
    "Hej, " + name + "!"
}

fn main() {
    text := "åäö 🌍"
    print(greet(text))
    print(text.len)          // 11 byte
    print(text.runes().len)  // 5 Unicode-kodpunkter
    print(text[7..11])       // 🌍; bytegränser måste vara UTF-8-gränser
    for r in text.runes() { print(r) }
}
```

Modellen är inspirerad av [V:s strängar](https://docs.vlang.io/v-types.html#strings):
oföränderlig UTF-8, längd i byte och en separat `rune` för Unicode-kodpunkter.
`"..."` och `'...'` är strängar; en rune skrivs med backticks, exempelvis
`` `🌍` ``. `text[i]` och `text.bytes()[i]` ger ett byte som `i32` (0–255);
`text.runes()[i]` ger en `rune`. `for` stöder båda vyerna.
En kodpunkt är inte alltid ett helt synligt tecken.

Strängar stöder `+`, innehållsjämförelse, typade parametrar och returvärden,
callbacks, arrayelement och structfält. `value.str()` konverterar heltal,
bool och runes. Unicode-escapes och NUL fungerar; alla strängvärden är giltig
UTF-8. Index, snittgränser och `rune(number)` kontrolleras vid körning.

Kör `cargo run --release -- run dogfood/utf8.dn` för textstatistik och
`cargo run --release -- run dogfood/tokenizer.dn` för en liten UTF-8-tokenizer.
[Textfunktionerna](dogfood/text/scan.dn), inklusive `find`, `replace` och
ordräkning, är skrivna i Dunder och har egna taggade tester.

Första implementationen lagrar dynamiskt byggda strängar i en separat
**16 MiB processarena**, som mappas vid första behov. Strängarna lever tills
processen avslutas. Ingen GC används, och minnet återvinns ännu inte under
körningen. En full arena ger ett definierat runtimefel. Detta är en första
körmodell för dogfooding, inte den färdiga ägar-/lånemodellen.

## Implementerat och återstående

Implementerat: `i32`, `i64`, `bool`, `string`, `rune`, typade funktioner och funktionsvärden,
callbacks, funktioner som returvärden, framåtanrop och rekursion,
filmoduler med explicita importer, alias, namnrymder och privata/exporterade funktioner och structar,
härledda/explicit typade lokaler, oföränderlighet, `var`, blockscope, `if/else`,
`while`, `for`, `break/continue`, typade arrayer och arrayiteratorer,
struct-uttryck och fältåtkomst, `@test`, `assert`, kortslutande booleska operatorer,
heltalsaritmetik, kontrollerade heltalskonverteringar och `print`.
Läs [syntaxen](docs/syntax.md).

Anrop stöder både register- och stackargument; fler än åtta parametrar fungerar.
Funktionstyper kontrolleras strukturellt. Anonyma funktioner och closures med
infångade lokaler är inte implementerade. Backend prioriterar korrekt typning
och körbar kod; optimering och byggprestanda är inte uppmätta.
Bootstrapkompilatorn är skriven i Rust och är ännu inte självhostad.
Structfält och arrayelement stöder i dag `i32`, `i64`, `bool`, `string` och `rune`.
Arrayer/structar/iteratorer kan ännu inte returneras från funktioner.
Nästlade aggregat, arrayslices, allmänt allokerings-API, ägande/lån och
användardefinierade iteratorer återstår. Stränginterpolering, normalisering,
grapheme-kluster och Unicode-baserad skiftlägeskonvertering återstår också.

[Aktuellt arbete](docs/status.md) · [Arkitektur](docs/arkitektur.md) ·
[Prestandamål och mätkontrakt](docs/prestanda.md)
