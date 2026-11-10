# Dunder ⚡

Ett statiskt typat, kompilerat språk med filändelsen **`.ddr`**.
Kompilatorn läser vanlig Dunder-källkod och skriver körbara filer för
ARM64/macOS, ARM64/Linux, x86-64/Linux och WebAssembly (wasm32). Kodgenerering, ELF-/Mach-O-skrivning
och macOS ad hoc-signering görs i kompilatorn.
Den aktuella utvecklings- och återställningsgrenen är `main`.

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

En fryst Dunder-kompilator kan bygga verktygskedjan på nytt från Dunder-källkod:

```sh
./tools/bootstrap.sh
```

Kommandot skriver ut sökvägen till den färdiga kompilatorn.
[Fröbinärer och återbyggnad](bootstrap/README.md) beskriver hur varje mål verifieras.

Bygg den optimerande Rust-kompilatorn med systemets Rust, utan externa crates:

```sh
cargo build --release
cargo run --release -- build examples/hello.ddr -o build/hello --timings
./build/hello
# 42
# true
```

Den byggda `dunder`-kompilatorn behöver inga externa kompilatorer,
assembler, länkare eller signeringsverktyg för att bygga Dunder-källkod.
Den färdiga binären körs som en vanlig process på målplattformen. Linuxbinärer
är fristående när programmet enbart använder Dunders runtime. Nåbara C-importer länkas mot de angivna delade biblioteken. Systemets
verktyg får användas för oberoende verifiering; `cargo test` kör riktiga binärer
på värdplattformen och verifierar även signaturer med `codesign` på macOS.

Målet väljs automatiskt för den egna plattformen. Korskompilering fungerar med
samma kompilator, utan extra SDK, targetpaket, assembler eller länkare:

```sh
# Exempelvis på macOS: bygg fristående Linuxprogram för båda arkitekturerna.
dunder build examples/hello.ddr --target x86_64-linux -o build/hello-linux
dunder build examples/hello.ddr --target arm64-linux -o build/hello-linux-arm64
# Även på Linux: bygg en signerad macOS-binär.
dunder build examples/hello.ddr --target arm64-macos -o build/hello-macos
dunder --list-targets
```

`dunder run` och `dunder test` kräver att målet matchar maskinen där de körs.
Läs [målplattformar och korskompilering](docs/targets.md) för detaljer.

```sh
cargo run --release -- check examples/fibonacci.ddr
cargo run --release -- build examples/fibonacci.ddr -o build/fibonacci
./build/fibonacci
# 6765
# true
```

`dunder run` bygger och kör programmet direkt. `fn main()` ger exitkod 0;
`fn main() -> i32` låter programmet välja exitkod med `return`.
Kör `cargo run --release -- run examples/functions.ddr` för att prova
funktioner, callbacks och funktioner som returvärden. Exemplet skriver
`42`, `42` och `-42`.

Funktioner kan vara värden med statiskt kontrollerad signatur:

```dunder
fn apply(transform: fn(i32) -> i32, value: i32) -> i32 {
    return transform(value)
}
```

Se [funktionsexemplet](examples/functions.ddr) för en funktion som
returnerar en annan funktion, lagring i lokala variabler och callbackanrop.

`dunder build` utan fil hittar närmaste `dunder.toml` uppåt från arbetskatalogen:

```toml
name = "hello"
root = "examples/hello.ddr"
test_root = "dogfood/tests.ddr"
```

Utdata hamnar då i `build/<target>/hello`, exempelvis `build/x86_64-linux/hello`.
`name` och `root` krävs;
`test_root` är valfri och väljer rotfil för `dunder test`. Utan den används
`root` även för tester. Kompileringsfel har fil, rad, kolumn och
felkod. Ett misslyckat bygge lämnar tidigare utdata orörd.

Läs om [FFI, C-layout och modulkapabiliteter](docs/ffi.md),
[funktionscachen, `check --agent` och `test --affected`](docs/incremental.md)
samt [en delad triangel på WebGPU, Metal och Vulkan](dogfood/webgpu/README.md).

## Verifierad täckning och återstående arbete

Native-självbyggen för macOS/ARM64, Linux/ARM64 och Linux/x86-64 når
byteidentiska fixpunkter och kör dogfood-sviten. Grafikbiblioteken för Metal
med AppKit och Vulkan med SDL är Dunder-kod. Rust-bootstrapens Wasm-backend
kör de portabla dogfood-grupperna och WebGPU-exemplet använder samma
triangeldata som nativeexemplen.

Rust-bootstrapen finns kvar som optimerande kompilator och återställningsväg.
Självhostens Wasm-backend omfattar skalärer, flyttal och typade importer.
Fullständigt stöd för hanterade samlingar i den backenden återstår, liksom
full diagnostik- och lånekontrollsparitet. Modulkapabiliteter och delegerade
filhandtag finns, medan legacy-API:erna i `sys` fortfarande ger direkt OS-åtkomst.

[Bygg- och agentmätningar](benchmarks/compiler/2026-11-10-macos-arm64/README.md)
och [runtimejämförelser](benchmarks/runtime/README.md) redovisar mätbara
framsteg och kvarvarande avstånd till målen för självhostens genomströmning,
C-prestanda och ändrade tester under 15 ms.

## Moduler

En `.ddr`-fil är en modul. Importen skrivs **utan filändelse** och söks
relativt filen som importerar:

```dunder
// math.ddr
pub fn add(a: i32, b: i32) -> i32 {
    return a + b
}
```

```dunder
// main.ddr
import math

fn main() {
    print(math.add(40, 2))
}
```

`fn` är privat inom modulen; `pub fn` exporterar funktionen. `import math`
läser `math.ddr`. Undermappar och alias skrivs exempelvis
`import util.predicates as predicates`. Modulnamn använder punkter utan citattecken;
även `import ..shared.math` och `import .sys as local` fungerar.
Importerna skrivs utan `.ddr`.

Typer följer samma regel: `struct Point` och `enum Command` är privata;
`pub struct Point` och `pub enum Command` exporteras.
En importerad typ skrivs exempelvis `geometry.Point`, både i
signaturer och i uttrycket `geometry.Point { x: 3, y: 4 }`.

Kör [exemplet med tre moduler](examples/modules/main.ddr):

```sh
cargo run --release -- run examples/modules/main.ddr
# 42
# true
# 42
```

Importer är explicita och läses bara en gång per verklig fil. Funktioner
med samma namn i olika moduler hålls åtskilda. Alla signaturer samlas före
typkontroll av kropparna, så ömsesidig rekursion fungerar även över
cirkulära importer. Endast rotmodulens `main` är programstart.

Hela modulträdet byggs till samma körbara fil med Dunders egen kodgenerering,
symbolupplösning, ELF-/Mach-O-skrivning och eventuell signering. Dunder-kompileringen anropar
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
cargo run --release -- test dogfood/geometry.ddr
cargo run --release -- run dogfood/structs.ddr
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
buffert. Arenan är ett fungerande bump-allokeringsprogram. Växande samlingar
och begränsade funktionsborrows finns nu i standardbibliotekets första steg.

## Standardbibliotek och dynamiska arrayer

```dunder
import std.map as map
import std.set as set
import std.array as array

fn main() {
    var counts := map.new()
    map.put(&mut counts, "dunder", 42)
    print(map.get_or(&counts, "dunder", 0))

    var names := set.new()
    added := set.insert(&mut names, "å 🌍")
    assert(added)

    var values: []i64 = [3, 1, 2]
    values.push(4)
    array.sort(&mut values)
    print(array.sum(&values))
}
```

`[]T` växer med `push` och `reserve`; `pop`, `clear`, indexering, längd och
iteration stöds. Kopior är oberoende, även inne i nästlade structar och enums.
`&values` lånar för läsning och `&mut values` för uppdatering under ett
funktionsanrop. Referenser kan ännu inte returneras eller lagras.

Map (`string` → `i64`), set av strängar, arrayalgoritmer, text, bytekodning,
decimalparsning, hashning och SHA-256 är skrivna i Dunder och inbäddade i
kompilatorn. Generiska map/set återstår. Dynamiska buffertar har unikt
ägarskap och återvinns vid tilldelning och scope-slut. Oföränderliga strängar
delas med referensräkning. Läs [minnesmodellen](docs/memory.md) och
[API, borrows och gränser](docs/standard.md).

## Enums och match

Både namngivna värden och varianter med data är typade värden:

```dunder
enum FileKind { Other = 0, File = 1, Directory = 2 }
enum Command { Stop, Print(string), Move(i32, i32) }

fn evaluate(command: Command) -> i32 {
    match command {
        .Stop => 0,
        .Print(text) => text.len,
        .Move(x, y) => x + y,
    }
}
```

Skriv `Command.Move(20, 22)` eller `.Move(20, 22)` när typen framgår av
kontexten. `match` kontrolleras för saknade och dubbla fall. Payloads ligger
direkt i värdet, kopieras med värdesemantik och kan innehålla structar,
arrayer, andra enums och funktionsvärden. Matchningen använder taggar och
direkta grenar utan vtables. Läs [enumreglerna](docs/enums.md) och prova
`cargo run --release -- run dogfood/enums.ddr`.

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

Kör `cargo run --release -- run dogfood/utf8.ddr` för textstatistik och
`cargo run --release -- run dogfood/tokenizer.ddr` för en liten UTF-8-tokenizer.
[Textfunktionerna](dogfood/text/scan.ddr), inklusive `find`, `replace` och
ordräkning, är skrivna i Dunder och har egna taggade tester.

Dynamiska strängar återvinns när sista ägaren försvinner, med icke-atomär
referensräkning för dessa oföränderliga värden. Literaler ligger kvar i
programbilden. Den återanvändbara strängpoolen är begränsad till **16 MiB**
plus en sida bokföring; en full pool ger ett definierat runtimefel.
Ingen spårande GC används. Läs [ägarskap, kostnader och gränser](docs/memory.md).

## Flyttal

`f32` och `f64` stöder aritmetik, jämförelser, decimal- och exponentliteraler,
explicita konverteringar samt parametrar, returvärden, callbacks, arrayer
och structfält:

```dunder
@test {
    (3.0, 4.0) -> 5.0
}
fn length(x: f64, y: f64) -> f64 {
    (x * x + y * y).sqrt()
}

fn main() {
    narrow: f32 = 0.1
    wide := 1.25e3        // f64 som standard
    print(length(3.0, 4.0))
    print(narrow)
    print(wide)
}
```

IEEE-beteende omfattar NaN, oändlighet, signerad noll och subnormala tal.
Metoder inkluderar `.abs()`, `.sqrt()`, `.floor()`, `.ceil()`, `.trunc()`,
`.round()`, `.round_ties_even()` och klassificering. Konvertering till
heltal trunkerar och kontrollerar intervallet. `.str()` och `print` använder
en kort decimalrepresentation som avrundas tillbaka till samma flyttal.
Utskriftsrutinen är [skriven i Dunder](runtime/float.ddr).

Kör `cargo run --release -- test dogfood/numeric/main.ddr` för sju numeriska
testgrupper med vektorer, stabil summering, rotberäkning och numerisk
integration. Läs [flyttalsreglerna och begränsningarna](docs/floating.md).

## Systembibliotek på macOS och Linux

`sys` följer med kompilatorn och kräver ingen lokal biblioteksfil:

```dunder
import sys

fn main() {
    saved := sys.write_text("hello.txt", "Hej från Dunder 🌍\n")
    if !saved.ok { print(sys.error_message(saved.error)); return }
    match sys.read("hello.txt") {
        .Ok(text) => { print(text) },
        .Err(error) => { print(sys.io_error_message(error)) },
    }
}
```

API:t omfattar läsning, skrivning, append, metadata, mkdir, rename och
borttagning av filer/tomma kataloger. `read` ger `Ok(string)` eller
`Err(IoError)`. Övriga I/O-resultat och den lägre läsfunktionen `read_text`
har `.ok` och `.error`; lästa strängar finns då i `.value`.
Vanliga I/O-fel avbryter inte processen.
`read_text` validerar UTF-8 och läser högst 4 MiB per fil till strängarenan.
Läs [sys-reglerna](docs/sys.md) för beteende och begränsningar och kör
`cargo run --release -- test dogfood/filesystem.ddr` för filsystemets dogfood.

Structar och fasta arrayer kan nu returneras som värden. Resultatet kopieras
till lagring hos anroparen och fungerar även genom callbacks och rekursion.

## Fristående projekt: Tabell

[Tabell](projects/tabell/README.md) är ett CSV-verktyg skrivet i Dunder med
eget manifest. Det grupperar rader och summerar en heltalskolumn, till exempel
kostnader i ören per team:

```sh
cargo build --release
cd projects/tabell
../../target/release/dunder run -- expenses.csv team cents
../../target/release/dunder test
```

Projektet använder importer utan citattecken, `array.sort_strings` och
`sys.eprint`. CSV-parsning, gruppering, sortering och felhantering körs i
den färdiga binären utan externa verktyg. Integrationstesterna jämför även
genererade CSV-data med en oberoende referens och bygger alla tre mål.

## Nativefönster och Metal

`import std.macos as macos` ger typade fönsterhandtag, händelser och en
Metal-triangelista. AppKit-delegaten och Metal-renderaren ligger i
`stdlib/macos/` och använder Dunders typade C-anrop och callbacks. Ingen C-, Objective-C- eller Swift-källkod
kompileras. Fönsterloopen, scenens tillstånd, geometrin och matematiken i
exemplen är Dunder-kod. Apples Metal-runtime kompilerar den fasta MSL-pipelinen
när ett fönster öppnas.

```sh
cargo run --release -- run dogfood/metal/triangle.ddr -- --frames 3
cargo run --release -- run dogfood/metal/main.ddr
cargo run --release -- build dogfood/metal/main.ddr --app -o build/DunderMetal.app
open build/DunderMetal.app
```

Atlas-exemplet har dragning, paus med mellanslag, palettbyte med C och reset
med R. Escape och fönstrets stängknapp avslutar. Layout och pekarinmatning
använder logiska punkter; ritmåtten följer Retina-skalan. Första API:t har
ett aktivt fönster per process och en fast pipeline för färgad geometri.
Grafik-API:t gäller bara målet `arm64-macos` och kräver en grafisk session.
Textlayout, nativekontroller, egen shaderkod och flera samtidiga fönster
återstår. Läs [macOS-/Metal-kontraktet](docs/macos-metal.md) och
[app-paketeringen](docs/app-bundles.md).

## Vulkan och lavapipe på Linux

`import std.vulkan as vulkan` ger motsvarande triangel- och Atlas-dogfood
på Linux. Vulkan renderar geometrin; headless-läget kan läsa tillbaka
RGBA och spara bilder. SDL2 visar samma renderade bild i fönsterläget.

```sh
cargo run --release -- run dogfood/vulkan/main.ddr -- --headless --frames 3 --capture build/atlas.ppm
cargo run --release -- run dogfood/vulkan/main.ddr
```

Välj Mesas lavapipe-ICD för mjukvarurendering. Dunder skriver nativekod och
dynamisk ELF själv, med inbäddad SPIR-V och utan extern länkare.
Se [Linux/Vulkan-guiden](docs/linux-vulkan.md) för installation, kontroller
och pixeltester med valideringslager.

## Implementerat och återstående

Implementerat: `i32`, `i64`, `f32`, `f64`, `bool`, `string`, `rune`, typade funktioner och funktionsvärden,
callbacks, funktioner som returvärden, framåtanrop och rekursion,
filmoduler med explicita importer, alias, namnrymder och privata/exporterade funktioner och typer,
härledda/explicit typade lokaler, oföränderlighet, `var`, blockscope, `if/else`,
`while`, `for`, `break/continue`, typade arrayer och arrayiteratorer,
struct-uttryck och fältåtkomst, värde-enums och varianter med data, uttömmande `match`,
`@test`, `assert`, kortslutande booleska operatorer,
heltals- och flyttalsaritmetik, kontrollerade konverteringar och `print`.
Läs [syntaxen](docs/syntax.md).

Anrop stöder både register- och stackargument; fler än åtta parametrar fungerar.
Funktionstyper kontrolleras strukturellt. Anonyma funktioner och closures med
infångade lokaler är inte implementerade. Standardbygget optimerar konstanta
uttryck, villkorshopp, immediater och operandernas stacktrafik; x86-64 håller
vanliga arbetsvärden i CPU-register. [Maskinkodsoptimeringen](docs/optimization.md)
bevarar overflowkontroller, evalueringsordning och minnesägarskap. Fyra lokala
[runtime-prov](benchmarks/runtime/README.md) visar 6–17 % kortare mediankörtid
och cirka 21 % mindre binärer på Linux/x86-64; det är ingen generell
prestandagaranti. Byggmätningar av ett genererat korpus finns i
[prestandarapporten](benchmarks/README.md); hela prestandakontraktet är ännu
inte uppfyllt. Oanvända funktioner och oanvänd flyttalsformatering tas bort
ur den genererade binären; parsern har en ASCII-väg utan teckenindexallokering.

Den [Dunder-skrivna kompilatorn](selfhost/README.md) bygger nu sin egen
källkod till ARM64/Linux. Stage 2 och stage 3 är byteidentiska, och stage 3
bygger körbara program med moduler, structar, arrayer och strängar.
Full språk- och plattformsparitet återstår; Rust-bootstrapen finns kvar
som återställningsväg.

Structfält kan innehålla nästlade structar, enums, fasta/dynamiska arrayer
och funktionsvärden. Arrayelement är fortsatt begränsade till primitiva
skalärer och enums utan payload. Aggregat returneras med värdesemantik.
Arrayslices, generiskt allokerings-API, generell ägar-/lånekontroll och
användardefinierade iteratorer återstår. Stränginterpolering, normalisering,
grapheme-kluster och Unicode-baserad skiftlägeskonvertering återstår också.

[Aktuellt arbete](docs/status.md) · [Arkitektur](docs/arkitektur.md) ·
[Prestandamål och mätkontrakt](docs/prestanda.md)
