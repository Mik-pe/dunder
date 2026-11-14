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

Bygg självhosten från det frysta fröet för din värdplattform och lägg
kompilatorn i sökvägen för den aktuella terminalen.

```sh
dunder_bin="$(./tools/bootstrap.sh)"
export PATH="$(dirname "$dunder_bin"):$PATH"
dunder run examples/hello.ddr
# 42
# true
dunder test
```

På ARM64/macOS verifierar skriptet först en seriell fixpunkt från fröet och
bygger sedan den taskbaserade kompilatorn med byteidentiska självbyggen för
`--jobs=1` och `--jobs=4`. Den publicerade `dunder` använder fyra workers som
standard. Linux behåller den seriella standardkompilatorn.
`./tools/bootstrap.sh --serial` publicerar i stället `dunder-serial` på alla
värdar och lämnar standardbinären orörd.
[Fröbinärer och återbyggnad](bootstrap/README.md) beskriver stegen.

Den byggda `dunder`-kompilatorn behöver inga externa kompilatorer,
assembler, länkare eller signeringsverktyg för att bygga Dunder-källkod.
Den färdiga binären körs som en vanlig process på målplattformen. Linuxbinärer
är fristående när programmet enbart använder Dunders runtime. Nåbara C-importer länkas mot de angivna delade biblioteken. Systemets
verktyg används för oberoende verifiering. Testgrupper körs i egna processer.
På macOS kan en apps signatur verifieras med `codesign --verify --strict`.

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
dunder check examples/fibonacci.ddr
dunder build examples/fibonacci.ddr -o build/fibonacci
./build/fibonacci
# 6765
# true
```

`dunder run` bygger och kör programmet direkt. `fn main()` ger exitkod 0;
`fn main() -> i32` låter programmet välja exitkod med `return`.
Kör `dunder run examples/functions.ddr` för att prova
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
test_root = "tests/suite.ddr"
```

Utdata hamnar då i `build/<target>/hello`, exempelvis `build/x86_64-linux/hello`.
`name` och `root` krävs;
`test_root` är valfri och väljer rotfil för `dunder test`. Utan den används
`root` även för tester. Kompileringsfel har fil, rad, kolumn och
felkod. Ett misslyckat bygge lämnar tidigare utdata orörd.

Läs om [FFI, C-layout och modulkapabiliteter](docs/ffi.md) och
[funktionscachen, `check --agent` och `test --affected`](docs/incremental.md).
Större dogfood-program (Metal-, Vulkan- och WebGPU-trianglar, Tabell samt en
händelsestyrd HTTP/1.1-server) finns i syskonrepot [dunder-hundmat](https://github.com/Mik-pe/dunder-hundmat).

## Självhostad kompilator

Dunders kompilator är skriven i Dunder. Samma typkontroll, lånekontroll och
ägarskapsregler används för samtliga fyra mål. WebAssembly-backenden skriver
körbara moduler med linjärt minne, typade värdimporter och callbacks.
De tre nativevärdarna har frysta kompilatorer för offlineåterbyggnad.

På ARM64/macOS använder kompilatorn språkets egna isolerade tasks för
verifiering, optimering och sänkning av hela funktions-IR:er. Frontend och
typning behåller en Session; färdiga funktionsobjekt valideras och länkas i ordning.
`--jobs N` eller `--jobs=N` väljer 1–256 workers. `--jobs=1`, `check`,
`--mir=off` och `--profile-compiler` använder samma seriella Session-kedja.
Antalet workers ändrar varken optimeringsbudgetar, cacheidentitet eller
programbilder. Läs [taskkontraktet](docs/tasks.md) och
[kompilatormätningen](benchmarks/2026-11-12-regional-workers/README.md).
Samma nya binär med en respektive fyra workers bygger det genererade
korpuset på median 982,428 respektive 623,820 ms. Det senaste paketet är
samtidigt 0,6–1,6 procent långsammare än föregående fyrtrådade standard,
med lägre minnestopp i större byggen. Runtimebilderna är identiska.

Kör verifieringen från reporoten med Python 3.9+, Node och en C-kompilator. C-kompilatorn bygger
ett separat provbibliotek för ABI-kontrollen:

```sh
./tools/verify-selfhost.sh
```

Sviten bygger kompilatorn från fröet, kör program och testgrupper på värden
och i Wasm, kontrollerar förväntade typfel och provar nativ paketering och FFI.
Grafik-, CSV- och HTTP-dogfood testas i syskonrepot `dunder-hundmat`.
[Arkitektur](docs/arkitektur.md) · [Lånekontroll och mål](selfhost/README.md)

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
dunder run examples/modules/main.ddr
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
Självbygget använder samma kompilator och samma byggkommando som andra projekt.

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
dunder test
dunder test tests/language_extensions.ddr
dunder run examples/hello.ddr
```

`dunder test` hittar tester i hela det explicit importerade modulträdet.
Varje testblock körs som en egen nativeprocess med fem sekunders tidsgräns;
nästa test körs även om ett tidigare misslyckas. `main` körs inte och behövs
inte i en testrot. Testfunktioner ingår bara i testbyggen.

## Arrayer, loopar och kontrollflöde

```dunder
fn main() {
    var values: [i32; 8] = [0; 8]
    for i in 0..len(values) {
        values[i] = i + 1
    }
    var sum := 0
    for value in &values {
        sum += value
    }
    assert(sum == 36)
    print(sum)
}
```

Arrayer och structar kopieras som värden, även vid funktionsanrop. Fält och
element är oföränderliga om bindningen saknar `var`. `for` stöder halvöppna
`i32`-intervall, arrayer, arrayiteratorer samt lånade element med `for x in &values`.
Sammansatt tilldelning (`+=`, `-=`, `*=`, `/=`, `%=`), byteliteraler (`b'A'`),
hex-/binärliteraler (`0xff`, `0b1010`) och lexikal `defer { ... }` stöds.
Index kontrolleras vid körning.

Större dogfood-program (sortering, tokenizer, arena, Tabell, grafik och HTTP/1.1-server)
finns samlade i [dunder-hundmat](https://github.com/Mik-pe/dunder-hundmat).

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
arrayer, andra enums och funktionsvärden. Matchningen använder taggar och direkta grenar utan vtables. Läs [enumreglerna](docs/enums.md)
(se även `dogfood/enums.ddr` i `dunder-hundmat`).

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

Tester för Unicode-text och bytefunktioner körs med `dunder test tests/suite.ddr`
(se även `utf8.ddr`, `tokenizer.ddr` och `text/scan.ddr` i `dunder-hundmat`).

Dynamiska strängar återvinns när sista ägaren försvinner, med icke-atomär
referensräkning för dessa oföränderliga värden. Literaler ligger kvar i
programbilden. Strängar och dynamiska arrayer delar en återanvändbar heap.
Läs [ägarskap, kostnader och gränser](docs/memory.md).

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

Numeriska dogfood-program (vektorer, bisektion och Runge–Kutta) ligger i
`dunder-hundmat`. Läs [flyttalsreglerna och begränsningarna](docs/floating.md).

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
Läs [sys-reglerna](docs/sys.md) för beteende, klocka, rå byte-I/O, kapabilitetsavgränsade
kataloger (`std.fs.ReadDir`) och nätverkssockets (`capability network`, `std.net`).
Kör `dunder test tests/net_loopback.ddr` för filsystem- och loopbacktester.

Structar och fasta arrayer kan nu returneras som värden. Resultatet kopieras
till lagring hos anroparen och fungerar även genom callbacks och rekursion.

## Syskonprojekt i dunder-hundmat

Större exempel och program byggda i Dunder — inklusive CSV-verktyget Tabell,
en händelsestyrd HTTP/1.1-server (`http/`), AppKit/Metal-, Vulkan- och
WebGPU-trianglar samt äldre språkövningar — underhålls i
[dunder-hundmat](https://github.com/Mik-pe/dunder-hundmat).

## Implementerat och återstående

Implementerat: `i32`, `i64`, `u8`, `u16`, `u32`, `u64`, `f32`, `f64`, `bool`, `string`, `rune`, byteliteraler `b'..'`,
hex-/binärliteraler `0x..`/`0b..`, sammansatt tilldelning (`+=`, `-=`, `*=`, `/=`, `%=`),
lexikal `defer { ... }`, `for x in &..`, typade funktioner och funktionsvärden,
callbacks, funktioner som returvärden, framåtanrop och rekursion,
filmoduler med explicita importer, modulkapabiliteter (`unsafe`, `filesystem`, `network`), alias, namnrymder och privata/exporterade funktioner och typer,
härledda/explicit typade lokaler, oföränderlighet, `var`, blockscope, `if/else`,
`while`, `for`, `break/continue`, typade arrayer och arrayiteratorer,
struct-uttryck och fältåtkomst, värde-enums och varianter med data, uttömmande `match`,
`@test`, `assert`, kortslutande booleska operatorer, `std.net` (TCP-sockets och `PollSet`),
isolerade `task scope`/`spawn`/`await` med typade `TaskID<R>` på ARM64/macOS,
heltals- och flyttalsaritmetik, kontrollerade konverteringar och `print`.
Läs [syntaxen](docs/syntax.md).

Anrop stöder både register- och stackargument; fler än åtta parametrar fungerar.
Funktionstyper kontrolleras strukturellt. Anonyma funktioner och closures med
infångade lokaler är inte implementerade. Kodgeneratorn bevarar
evalueringsordning, overflowkontroller och städning av ägda värden.
Se [kompilatorns kodgenerering](docs/optimization.md).

Structfält kan innehålla nästlade structar, enums, fasta/dynamiska arrayer
och funktionsvärden. Arrayelement är fortsatt begränsade till primitiva
skalärer och enums utan payload. Aggregat returneras med värdesemantik.
Ett generiskt allokerings-API, lån som kan returneras och
användardefinierade iteratorer återstår. Stränginterpolering, normalisering,
grapheme-kluster och Unicode-baserad skiftlägeskonvertering återstår också.

[Aktuellt arbete](docs/status.md) · [Arkitektur](docs/arkitektur.md) ·
[Prestandamål och mätkontrakt](docs/prestanda.md)
