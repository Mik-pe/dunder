# Dunder ⚡

<p align="center">
  <img src="docs/dunder.jpg" width="320" alt="Dunders logga med ett stentroll och en gyllene blixt">
</p>

Ett statiskt typat, kompilerat språk med filändelsen **`.ddr`**.
Kompilatorn är själv skriven i Dunder och bygger körbara program för
ARM64/macOS, ARM64/Linux och x86-64/Linux. Den skriver även wasm32-moduler.
Nativ kodgenerering, länkning och macOS ad hoc-signering ingår i kompilatorn.

```dunder
fn add(a: i32, b: i32) -> i32 {
    a + b
}

fn main() {
    answer := add(40, 2)
    expected: i32 = 42
    print(answer)
    print(answer == expected)

    mut count := 0
    while count < 3 {
        print(count)
        count = count + 1
    }
}
```

`:=` deklarerar med härledd typ. `: i32 =` deklarerar med explicit typ.
Värden och parametrar är oföränderliga som standard; `mut` tillåter senare
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

För att installera kompilatorn lokalt i `~/.dunder/bin/dunder` (välj annan rot med
`DUNDER_HOME`) och senare uppgradera från GitHub Releases:

```sh
./tools/install.sh
export PATH="$HOME/.dunder/bin:$PATH"
dunder --version
dunder upgrade --check
```

Efter installationen visar `dunder upgrade --check` om den senaste
publicerade GitHub-releasen har en annan binär för din värdplattform.
Kör `dunder upgrade` för att installera den, eller välj en bestämd release
med exempelvis `dunder upgrade --version v0.2.0`. Uppgraderingen hämtar
arkivet med `curl`, packar upp det med `gzip` och verifierar SHA-256 före
bytet. Releaser byggs lokalt och publiceras med `gh`.
[Bygg och publicera en release](docs/releases.md)

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

`dunder check` typkontrollerar även biblioteksmoduler utan `fn main()`.
Alla funktionskroppar i importträdet kontrolleras; `build` och `run` kräver
fortfarande en giltig startfunktion.

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

Alla mål använder samma typkontroll, lånekontroll och ägarskapsregler. De tre
nativa värdplattformarna har frysta kompilatorer för återbyggnad offline.

På ARM64/macOS delar typkontrollen och backenden en workerpool.
Samordnaren kan typa en funktion medan arbetarna typar andra nåbara
funktioner. Både seriella och parallella byggen läser typade operationer
direkt för att bygga, verifiera och sänka MIR. Samordnaren binder symboler,
rapporterar fel och länkar objekten i en bestämd ordning. En och fyra
workers producerar byteidentiska kompilatorer. `--jobs N` väljer 1–256
workers. `check` och `--profile-compiler` använder den seriella vägen.
[Taskkontrakt och arbetsfördelning](docs/tasks.md)

Följande historiska mätning gäller kompilatorn före övergången till
läslån och ägarflyttar. På Apple M5 byggde den typade funktionskedjan det
låsta kompilatorkorpuset
på **359,79 ms**, jämfört med **468,17 ms** för revision `4585bc4`.
Medianen omfattar tolv parade byggen med fyra workers, varm filsystemcache
och avstängd inkrementell cache, inklusive signering och publicering.

| 101 024 källrader | Före, ms | Efter, ms |
| --- | ---: | ---: |
| Identiska funktionskroppar | 82,13 | 76,14 |
| En unik konstant per funktion | 146,57 | 135,61 |
| Unika kroppar med blandade uttryck | 220,13 | 202,61 |

Den bygglokala skalärmodellcachen kan återanvända identiska kroppar.
Mätserien omfattar även en stor enskild funktion, anrop med varierade typer
och Tabells bygg- och körtid. [Mätmetod, körtider och rådata](benchmarks/compiler/2026-11-15-shared-frontend/README.md)

Kör verifieringen från reporoten med Python 3.9+, en C-kompilator,
`curl` och `gzip`. C-kompilatorn bygger provbiblioteket för ABI-kontrollen;
uppgraderingsproven använder `curl` även mot en lokal filmirror.

```sh
./tools/verify-selfhost.sh
```

Sviten bygger kompilatorn från fröet, kör program och testgrupper på värden,
kontrollerar förväntade typfel och provar korsbyggen, nativ paketering och FFI.
Wasm-sviterna är senarelagda under det nativa utvecklingsarbetet.
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
    mut values: [i32; 8] = [0; 8]
    for i in 0..len(values) {
        values[i] = i + 1
    }
    mut sum := 0
    for value in &values {
        sum += value
    }
    assert(sum == 36)
    print(sum)
}
```

Resursägande arrayer och structar flyttas vid tilldelning; vanliga
funktionsparametrar lånar dem för läsning. Fält och
element är oföränderliga om bindningen saknar `mut`. `for` stöder halvöppna
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
    mut counts := map.Map::new()
    counts.put("dunder", 42)
    print(counts.get_or("dunder", 0))

    mut names := set.Set::new()
    added := names.insert("å 🌍")
    assert(added)

    mut values: []i64 = [3, 1, 2]
    values.push(4)
    array.sort(&mut values)
    print(array.sum(&values))
}
```

`[]T` växer med `push` och `reserve`; `pop`, `clear`, indexering, längd och
iteration stöds. `next := values` flyttar ägarskapet. `values.clone()`
ger en uttrycklig, oberoende kopia, även inne i structar och enums.
Vanliga funktionsanrop och loopar lånar för läsning; `&mut values` lånar
för uppdatering. En `move`-parameter tar över ägarskapet. Oberoende structfält
kan flyttas separat, och `replace(&mut place, replacement)` lämnar tillbaka
den tidigare ägaren medan destinationen förblir initierad.

Lokala läslån kan namnges med `r := &owner.field` eller `r := &values[index]`.
`&values[start..end]` ger en arrayvy och `&text[start..end]` en UTF-8-kontrollerad
bytevy, utan ny heapbuffert eller retain. Lånen gäller till blockslut.
Temporärer som `&f()` och `&[1, 2]` kan lånas under ett anrop. Returnerade lån,
referenser i aggregat och lokala `&mut` återstår.

Map (`string` → `i64`), set av strängar, arrayalgoritmer, text, bytekodning,
decimalparsning, hashning och SHA-256 är skrivna i Dunder och inbäddade i
kompilatorn. Generiska map/set återstår. Dynamiska buffertar har unikt
ägarskap och återvinns vid tilldelning och scope-slut. Oföränderliga strängar
delas med referensräkning. Läs [minnesmodellen](docs/memory.md) och
[API, borrows och gränser](docs/standard.md).

Nativa program kan också använda [HTTP-server och TLS-klient](docs/http.md),
[SQLite med bundna parametrar](docs/sqlite.md) och processersättning i
`std.process`. Klienten och databasen länkar mot systemets libcurl respektive
SQLite. [Typade variadiska C-anrop](docs/ffi.md#anrop-och-adresser) följer
respektive nativt ABI.

## Enums och match

Ett `match`-uttryck kan packa upp nästlade varianter och ge ett typat resultat.
Ett `if` efter mönstret prövas när mönstret matchar. Variablerna som mönstret
binder kan användas både i villkoret och i armens uttryck.

```dunder
enum Command { Stop, Move(i32, i32) }
enum Event { Idle, Run(Command) }

fn distance(event: Event) -> i32 {
    match event {
        .Run(.Move(x, y)) if x == y => x,
        .Run(.Move(x, y)) => x + y,
        .Run(.Stop) => 0,
        .Idle => 0,
    }
}
```

Armarna prövas i källordning. Armar utan villkor ska tillsammans täcka alla
möjliga värden. I exemplet hanterar den andra armen drag där `x != y`. Vanlig matchning lånar sitt värde. `match move value` tar över dess ägda payloads.
[Enumregler och fler mönster](docs/enums.md)

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

Prefixet `f` aktiverar stränginterpolering:

```dunder
name := "världen"
print(f"Hej, {name}! Svaret är {40 + 2}.")
print(f"{{{name}}}")  // {världen}
```

Varje uttryck mellan `{` och `}` utvärderas en gång, från vänster till
höger. Strängar, heltal, flyttal, bool och runes formateras med sina vanliga
textrepresentationer. `{{` och `}}` ger bokstavliga klamrar. Vanliga strängar
tolkar inga uttryck. [Regler och exempel](docs/syntax.md#stränginterpolering)

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

Structar och fasta arrayer kan returneras till lagring hos anroparen.
Resursägande resultat flyttas utan en extra datakopia, även genom callbacks
och rekursion.

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
härledda/explicit typade lokaler, oföränderlighet, `mut`, blockscope, `if/else`,
`while`, `for`, `break/continue`, typade arrayer och arrayiteratorer,
struct-uttryck och fältåtkomst, `impl`-block med associerade funktioner (`Typ::namn`), metoder (`Self`, `&self`, `&mut self`, `self`/`move self`), värde-enums och varianter med data, uttömmande `match`,
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
värden, strängar och enums utan payload. Resursägande aggregat överförs till
returvärdets ägare. Returnerade lån, lokala skrivlån, generiska samlingar,
automatisk cleanup av externa resurser, ett generiskt allokerings-API och
användardefinierade iteratorer återstår. Normalisering,
grapheme-kluster och Unicode-baserad skiftlägeskonvertering återstår också.

[Aktuellt arbete](docs/status.md) · [Arkitektur](docs/arkitektur.md) ·
[Prestandamål och mätkontrakt](docs/prestanda.md)
