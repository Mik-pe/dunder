# Dunders första implementerade syntax

Beslut 2026-11-10: prioritera det typade källspråket och kompilera riktiga
program. Håll stavning och struktur regelbundna så att både människor och
kodagenter kan använda vanliga mönster för funktioner, uttryck och block.
Detta är ett prövbart designval, inte ett påstående om mätt agentpreferens.

Källfiler använder **`.ddr`**, till exempel `main.ddr` och `functions.ddr`.

## Deklarationer och muterbarhet

```dunder
count := 42            // härledd i32, oföränderlig
wide: i64 = 42         // explicit i64, oföränderlig
ready: bool = true     // explicit bool, oföränderlig

var total := 0         // explicit muterbar i32
var big: i64 = 0       // explicit muterbar i64
total = total + 1      // tilldelning till befintlig muterbar bindning
```

`:=` deklarerar ett nytt namn i det aktuella blocket och härleder dess typ.
`: Typ =` gör samma sak med en explicit typ. Ett nytt `:=` med samma namn i
samma scope är ett fel. Ett inre block får skugga ett yttre namn.

Bindningar och funktionsparametrar är oföränderliga som standard. `var` är
det explicita valet av muterbarhet. Typen ändras aldrig efter deklarationen.
All deklaration kräver initiering. Det finns inga oinitierade lokaler i denna
delmängd. Oföränderlighet kräver inte att uttrycket kan beräknas vid byggtid.

## Typer och uttryck

| Typ | Betydelse |
| --- | --- |
| `i32` | Signerat 32-bitars heltal; standard för okontextualiserade heltalsliteraler. |
| `i64` | Signerat 64-bitars heltal. |
| `f32` | IEEE 754 binary32. |
| `f64` | IEEE 754 binary64; standard för okontextualiserade flyttalsliteraler. |
| `bool` | `true` eller `false`; ingen implicit konvertering till/från heltal. |
| `string` | Oföränderlig, giltig UTF-8. |
| `rune` | En Unicode-kodpunkt; skild från heltal och sträng. |
| `()` | Inget returvärde; implicit returtyp för funktioner utan `->`. |
| `fn(i32) -> i32` | Ett funktionsvärde med angivna parameter- och returtyper. |
| `[i32; 8]` | En array med åtta element; längden ingår i typen. |
| `Point` / `geometry.Point` | En namngiven struct i aktuell eller importerad modul. |
| `Command` / `sys.ReadResult` | En nominell enum med namngivna värden eller payload-varianter. |
| `range` | Ett halvöppet intervall med `i32`-gränser. |

Literaler får typ från en annotering, funktionens parameter/returtyp eller
den andra heltalsoperanden. Befintliga värden konverteras aldrig implicit
mellan `i32` och `i64`. `i64(value)` breddar explicit; `i32(value)` smalnar
med en körningskontroll. En literal utanför sitt typintervall är ett typfel.
Flyttalsliteraler, konverteringar, NaN, avrundning och numeriska metoder
beskrivs i [flyttalsreglerna](floating.md).

```dunder
small := 42
large := i64(small) + 1
ok := large == 43
```

Operatorer i fallande prioritet: unärt `-`/`!`, `*`/`/`/`%`, `+`/`-`,
`<`/`<=`/`>`/`>=`, `==`/`!=`, `&&`, `||`. `&&` och `||` kortsluter.
Bool stöder likhet/olikhet och logiska operationer. Heltalsoperationer kräver
matchande typer. Division avrundar mot noll; resten har täljarens tecken.

Spill i `+`, `-`, `*`, unärt minus, divisionen `MIN/-1` och avsmalnande
konvertering avslutar processen med status 101. Division/rest med noll ger
102. Ogiltiga arrayindex ger 103 och en misslyckad `assert` ger 104.
Konvertering från `i32` till en enum utan payload kontrollerar deklarerade
värden och ger 108 för okända värden; se [enums och match](enums.md).
Reglerna gäller även optimerade byggen. Wraparound är ännu inte en
språkoperation. Statuskoderna är den första felmekanismen; källkopplade
runtime-diagnoser återstår.

## Funktioner och kontrollflöde

```dunder
fn fibonacci(n: i32) -> i64 {
    var previous: i64 = 0
    var current: i64 = 1
    var i := 0
    while i < n {
        next := previous + current
        previous = current
        current = next
        i = i + 1
    }
    return previous
}

fn main() {
    result := fibonacci(20)
    if result == 6765 {
        print(result)
    } else {
        print(false)
    }
}
```

Parameter- och returtyper i funktionssignaturer är explicita. Framåtanrop,
rekursion och ömsesidig rekursion tillåts. Parametrarna är oföränderliga;
kopiera till en `var`-lokal om de behöver ändras. De första åtta argumenten
överförs i register och ytterligare argument på stacken. Blandade `i32`,
`i64`, `bool` och funktionsargument stöds även på stacken.

`main` tar inga argument och returnerar `()` eller `i32`. En `i32`-retur blir
processens exitstatus enligt operativsystemets vanliga regler. Övriga
funktioner med returvärde måste returnera på varje väg. Kontrollen behandlar
loopar konservativt: lägg en retur efter loopen om funktionen returnerar ett värde.

Funktionens sista uttryck utan semikolon får ge returvärdet:

```dunder
fn div_by_five(x: i32) -> i32 {
    x / 5
}

fn maximum(a: i32, b: i32) -> i32 {
    if a > b { a } else { b }
}
```

Det gäller även slutliga grenar i `if/else` och slutliga nästlade block.
Ett semikolon gör uttrycket till en vanlig sats och ger inget returvärde.
`if` är ännu inte ett allmänt uttryck som kan tilldelas en variabel.
Ett vanligt block i en funktion utan returvärde fortsätter till nästa sats.

### Funktionsvärden och callbacks

```dunder
fn double(value: i32) -> i32 {
    return value * 2
}

fn selected() -> fn(i32) -> i32 {
    return double
}

fn apply(transform: fn(i32) -> i32, value: i32) -> i32 {
    return transform(value)
}

fn main() -> i32 {
    transform := selected()
    explicit: fn(i32) -> i32 = double
    print(apply(transform, 21))
    print(explicit(21))
    print(selected()(21))
    return 0
}
```

Ett namngivet funktionsnamn utan `(...)` är ett värde. `:=` härleder hela
funktionstypen. Typen skrivs `fn(Parametertyper) -> Returtyp`, utan
parameternamn; `fn(i32)` betyder `fn(i32) -> ()`.

Parameter- och returtyper måste matcha exakt när en funktion lagras, skickas,
returneras eller tilldelas. Funktionsvärden är oföränderliga som standard;
`var transform := double` tillåter byte till en annan funktion med samma typ.
Funktioner kan ta och returnera andra funktionsvärden, även i flera nivåer.
Anrop utvärderar först funktionen och därefter argumenten från vänster till
höger. Inbyggda operationer som `print` är ännu inte funktionsvärden; skriv
en vanlig `fn` som omslag vid behov. Funktionslikhet och anonyma closures är
ännu inte språkoperationer.

### Startpunkt och returvärden

`fn main()` körs vid programstart och ger exitkod 0. Med `fn main() -> i32`
används returvärdet som processens exitkod. Exempelvis ger `return 7` exitkod
7, även genom `dunder run`. Felaktig eller saknad `main` upptäcks vid
typkontroll. En vanlig funktion utan `->` returnerar `()`; den får använda
`return` utan värde eller nå slutet av kroppen. En funktion med returtyp
måste returnera ett värde av rätt typ på alla vägar enligt kontrollen ovan.

`dunder run [fil.ddr]` bygger och kör den nya binären. Byggmeddelanden går
till stderr och programmets stdout bevaras. Ett kompileringsfel startar
aldrig en tidigare binär från samma utdataplats.

`if` och `while` kräver `bool`, aldrig ett heltal. `else if`, `else`, `break`
och `continue` stöds. Block ger lexikala scope. `print` är den första inbyggda
I/O-operationen: ett `i32`, `i64` eller `bool`, följt av radbrytning.
`print` tar också en UTF-8-sträng eller rune. `assert(condition)` kräver
`bool` och avslutar processen vid `false`.

Vanliga enkla satser avslutas med radbrytning eller valfritt semikolon.
`}` får avsluta den sista satsen i ett block. Parenteser tillåter flerradiga
anrop och uttryck; en rad får fortsätta efter en binär operator.
Kommentarer skrivs `// ...` eller `/* ... */`. Källan är UTF-8 och namn
får innehålla bokstäver som `å`, `ä` och `ö`; nyckelorden är engelska.

## Moduler och importer

En fil är en modul. Namnen i modulen är lokala för den filen. Skriv en
import på toppnivå **utan suffix**:

```dunder
import math
import "geometry/vector" as vector
```

`import sys` väljer det inbäddade macOS-systembiblioteket; se [sys](sys.md).
Det kan aliaseras och dedupliceras precis som andra moduler. En egen lokal
fil med samma namn importeras uttryckligt som `import "./sys" as local`.

Den första importen läser `math.ddr` bredvid den importerande filen. Den andra
läser `geometry/vector.ddr` relativt samma fil. Sökningen beror inte på
processens arbetskatalog. Citerade relativa sökvägar kan innehålla `../`.
Absoluta sökvägar och suffix i importer avvisas. Utan `as` kommer aliaset
från det sista sökvägssegmentet; `import "geometry/vector"` får alias `vector`.
Ett alias måste vara ett giltigt, icke reserverat namn. Till exempel kräver
`import "foo-bar" as foo` ett explicit alias eftersom bindestreck inte ingår i namn.

```dunder
// math.ddr
pub fn double(value: i32) -> i32 {
    return scale(value, 2)
}

fn scale(value: i32, factor: i32) -> i32 {
    return value * factor
}
```

```dunder
// main.ddr
import math

fn main() {
    transform: fn(i32) -> i32 = math.double
    print(transform(21))
}
```

`fn` är privat; `pub fn` exporterar en namngiven funktion. Samma fil kan
anropa sina privata funktioner. Andra moduler får bara använda dess publika
funktioner via sitt importalias. `struct`/`pub struct` och `enum`/`pub enum`
följer samma synlighetsregel; fält och varianter följer typens synlighet.
Funktionsvärden och deras strukturella typer
fungerar likadant över modulgränser som inom en fil.

En import kopierar inga namn till aktuell namnrymd. `math.double` fungerar;
`double` kräver en egen lokal bindning, exempelvis `double := math.double`.
Alias får inte krocka med ett annat importalias eller en toppnivåfunktion i
samma fil. Lokala värden får skugga ett alias; det skuggade namnet ger då
inte längre tillgång till modulen i det scopet. En moduls egna importer
exporteras inte vidare. Varje fil importerar sina egna beroenden uttryckligt.

Moduler innehåller funktioner, struct- och enumdeklarationer, inga globala värden
eller initialiseringskod. Typer och signaturer från hela grafen fastställs före kropparna.
Cirkulära importer är därför tillåtna och kan innehålla ömsesidig rekursion.
Den kanoniska filsökvägen identifierar en modul: flera alias, gemensamma
beroenden och symboliska länkar laddar samma fil en gång. Filernas och
funktionernas ordning bestäms av källans import-/deklarationsordning.

Rotfilen är filen som ges till kommandot eller manifestets `root`. Bara dess
`main` är startpunkt; en importerad `main` ersätter aldrig en saknad rot-main.
Fel i importerade filer anger deras egen fil, rad och kolumn. Ett missat eller
oläsbart beroende anger importplatsen. `dunder check`, `build` och `run` läser
hela det uttryckligt importerade trädet och ignorerar andra filer i katalogen.
Ett nytt bygge läser om beroenden; inkrementell cache är inte implementerad.

Hela programmet länkas av Dunders egen backend. Moduler är källnamnrymder,
inte dynamiska bibliotek. Ingen GCC/LLVM-, assembler- eller länkprocess startas
under Dunder-kompileringen. Bibliotekets strängbaserade `check`/`compile`
använder en fil; filimporter kräver `modules::check_path` för en verklig rotfil.

## Structar och uttryck som argument

```dunder
pub struct Point {
    x: i32,
    y: i32,
}

fn length_squared(p: Point) -> i32 {
    p.x * p.x + p.y * p.y
}

fn main() {
    original := Point { x: 6 / 2, y: 2 + 2 }
    var moved := original
    moved.x = moved.x + 1
    print(length_squared(Point { x: 3, y: 4 }))
    print(original.x)
}
```

Structar är nominella: olika deklarationer ger olika typer även med samma
fält. Importerade typer behåller sin identitet över moduler och i callbacks.
Alla fält måste anges exakt en gång; deras ordning i uttrycket är valfri.
Fältuttrycken utvärderas i den ordning de står i källan. Varje fält får vara
`i32`, `i64`, `f32`, `f64`, `bool`, `string`, `rune` eller en enum utan payload;
nästlade structar och arrayfält återstår.

En struct kopieras vid bindning, tilldelning och parameteröverföring.
Ändringar i `moved` påverkar därför inte `original`. Parametrar är
oföränderliga; kopiera till en `var` för att ändra fält. Structar kan också
returneras från funktioner. Anroparen äger resultatets lagring, så en
returnerad lokal lämnar ingen pekare till avslutad stacklagring. Detta
fungerar även för callbackreturer, rekursion och importerade nominella typer.

I `if`-/`while`-villkor och `for`-huvuden behöver ett direkt struct-uttryck
parenteser runt sig för att skiljas från kontrollflödets block. I vanliga
anropsargument, deklarationer och testfall behövs inga extra parenteser.

## Arrayer, intervall och iteratorer

```dunder
fn sum(values: [i32; 4]) -> i32 {
    var result := 0
    for value in values { result = result + value }
    result
}

fn main() {
    var values: [i32; 4] = [0; 4]
    for i in 0..len(values) { values[i] = i + 1 }
    snapshot := values.iter()
    values[0] = 99
    for value in snapshot { print(value) }
    print(sum([1, 2, 3, 4]))
}
```

`[a, b, c]` bygger en array. `[value; N]` utvärderar `value` en gång och
upprepar värdet. Längden måste vara en heltalsliteral. Elementtypen är
`i32`, `i64`, `f32`, `f64`, `bool`, `string`, `rune` eller en enum utan payload;
den härleds eller hämtas från förväntad arraytyp.
En array får använda högst 64 KiB. Tomma arrayliteraler kräver explicit typ.
Nästlade arrayer, structelement och slices är ännu inte implementerade.

Arrayer kopieras som värden, även till parametrar. `var` krävs för att
tilldela ett element. Index har typen `i32`; negativa index och index vid
eller efter längden ger runtimefel. `len(values)` och `values.len()` ger
längden som `i32`. Fasta arrayer kan returneras från funktioner med samma
kopiering till anroparens lagring som structar.

`start..end` har typen `range`, inkluderar start och utesluter slut.
Gränserna är `i32`, utvärderas en gång och iterationen går uppåt.
Om start är större än eller lika med slut blir det noll varv. Intervall kan
lagras, skickas och returneras som vanliga värden.

`for` stöder intervall, arrayer och `.iter()`-värden. Loopvariabeln är
oföränderlig och lokal till loopen. `break` lämnar närmaste loop; `continue`
går till nästa element. Arrayiteration tar en kopia vid loopstart. `.iter()`
skapar en ägd kopia vid anropet, som kan itereras flera gånger och kopieras
till andra bindningar eller parametrar. Ändringar i ursprungsarrayen påverkar
inte kopian. Iteratorns typ härleds med `:=`; den saknar ännu egen
typstavning, `.next()` och användardefinierat iteratorprotokoll.

## Taggade tester

```dunder
@test {
    (20) -> 4
    (10 + 5) -> 2 + 1
    var x := 0
    for i in 0..4 { x = x + i }
    div_by_five(x + 14) == 4
}
fn div_by_five(x: i32) -> i32 {
    x / 5
}
```

`@test { ... }` hör till den direkt följande funktionen. Varje rad
`(arg1, arg2, ...) -> expected` anropar just den funktionen och kontrollerar
likhet. Samtliga argument och det förväntade värdet är vanliga typkontrollerade
uttryck. Struct-uttryck fungerar direkt:

```dunder
struct Point { x: i32, y: i32 }

@test {
    (Point { x: 3, y: 4 }) -> 25
    point := Point { x: 6 / 2, y: 2 + 2 }
    length_squared(point) == 25
}
fn length_squared(p: Point) -> i32 {
    p.x * p.x + p.y * p.y
}
```

Testblocket har eget scope och får använda vanliga deklarationer, uttryck,
anrop, villkor och loopar. Ett fristående bool-uttryck blir en assertion;
`assert(...)` fungerar också. Oanvända taluttryck är fel. Förväntade
tabellresultat måste stödja likhet; structar och arrayer saknar ännu
likhetsoperator. Kortformen anropar alltid den annoterade funktionen, även
om en lokal bindning i testblocket skuggar dess namn.

Fristående tester skrivs `@test fn descriptive_name() { ... }`. En sådan
testfunktion ska sakna parametrar och returnera `()`. Den ingår bara i
testbyggen; den vanliga funktionen efter ett kopplat testblock ingår i
både testbyggen och vanliga byggen.

`dunder test [file.ddr]` samlar tester ur hela det explicit importerade trädet,
en gång per kanonisk fil. Saknad `main` är tillåten och en befintlig `main`
körs inte. Varje testfunktion eller kopplat block blir en egen nativeprocess.
Första felaktiga assertionen avbryter det blocket; nästa test körs ändå.
Fel redovisar testnamn, felorsak och upp till 4096 byte vardera av stdout
och stderr. Assertionens exakta källrad och värden redovisas ännu inte.

Standardtidsgränsen är fem sekunder per test, ändringsbar med
`--timeout-ms 1000`. Runnern stoppar och väntar in en process som överskrider
gränsen. `-o directory` väljer baskatalog för tillfälliga testbinärer;
varje körning får en egen underkatalog som städas efteråt. Exitstatus är 0
när alla tester passerar, annars 1. Noll hittade tester rapporteras uttryckligt.

Utan fil väljer `dunder test` manifestets valfria `test_root`, annars `root`.
`build`, `run` och `check` använder alltid `root`. Manifestet kräver `name`
och `root`; alla tre fält är enkla citerade strängar.

## UTF-8-strängar och runes

```dunder
fn greet(name: string) -> string { "Hej, " + name + "!" }

fn main() {
    text := 'åäö 🌍'
    print(greet(text))
    assert(text.len == 11)
    assert(text.runes().len == 5)
    assert(text[7..11] == "🌍")
    for r in text.runes() { print(r) }
    for byte in text.bytes() { print(byte) }
}
```

Strängar är alltid giltig UTF-8 och deras byte är oföränderliga.
`var text := "a"; text = "b"` byter hela värdet; direkt ändring av ett byte
är förbjuden. Strängar kan skickas och returneras, även genom callbacks,
och lagras i arrayer och structfält. Kopior kan dela oföränderliga data.

`"..."` och `'...'` ger samma typ `string`. Backticks ger typen `rune`,
exempelvis `` `å` `` eller `` `🌍` ``. En sådan literal måste innehålla exakt
en Unicode-skalär. `rune` är en egen typ som stödjer jämförelse, `print`,
`.str()` och explicit heltalskonvertering med `i32(r)`/`i64(r)`.
`rune(number)` kontrollerar att talet är 0–0x10FFFF och inte en surrogate.
Ingen implicit konvertering mellan rune, heltal och string sker.

Escape-sekvenser: `\\`, `\"`, `\'`, `` \` ``, `\n`, `\r`, `\t`, `\0`,
`\xHH`, `\uHHHH` och `\u{H...}` med en till sex hexsiffror.
Byte-escapes valideras tillsammans: `"\xC3\xA5"` är `"å"`, men `"\xFF"`
är ett kompileringsfel. Surrogates och värden över Unicode-intervallet
avvisas. Oescapade radbrytningar, råsträngar och interpolation stöds ännu inte.
NUL är tillåten i strängdata; längden avgör slutet, ingen nollterminering används.

| Operation | Betydelse |
| --- | --- |
| `text.len`, `text.len()`, `len(text)` | Antal byte, `i32`, konstant tid. |
| `text[i]`, `text.bytes()[i]` | Ett byte som `i32` i intervallet 0–255. |
| `text[start..end]` | Ny sträng från halvöppna bytegränser. Båda måste vara UTF-8-gränser. |
| `text.runes()[i]` | En rune efter avkodning från början; linjär tid. |
| `text.runes().len` | Antal kodpunkter; linjär tid. `.len()` och `len(...)` fungerar också. |
| `for r in text.runes()` | Avkodar kodpunkter i ett linjärt pass. |
| `for byte in text.bytes()` | Itererar byte i ett linjärt pass. |
| `a + b` | Bygger en ny sträng av två strängar. |
| `a == b`, `a != b` | Jämför innehåll, inklusive eventuella NUL. |
| `<`, `<=`, `>`, `>=` | Lexikografisk ordning efter UTF-8-byte, utan språklig sortering. |
| `value.str()` | Sträng av heltal, flyttal, bool eller rune. På string ges samma värde. |

`.bytes()` och `.runes()` är återanvändbara vyer över ett oföränderligt
strängvärde. De allokerar ingen array. Deras typer härleds med `:=` och har
ännu ingen egen typstavning. Om originalbindningen får en ny sträng behåller
en befintlig vy sitt gamla värde. `for` direkt över en string avvisas:
välj `.bytes()` eller `.runes()` uttryckligt. `break` och `continue` fungerar
för båda. Arrayiteratorernas befintliga kopieringsregler gäller fortfarande.
Strängsnitt kan också ta ett lagrat eller returnerat `range`-värde,
exempelvis `span := 7..11; part := text[span]`.

En rune är en Unicode-kodpunkt, inte ett grapheme-kluster. Exempelvis har
`"é"` en rune och `"e\u{301}"` två; strängarna jämförs som olika.
Normalisering, case folding, fullständig Unicode-klassificering och synliga
teckenkluster stöds ännu inte. Dogfood-ordräknaren känner bara igen de
blanktecken som uttryckligen listas i dess Dunder-källa.

Första minnesmodellen använder statiska literaler i programbilden och en
separat 16 MiB arena för dynamiska strängar. Arenan mappas först vid behov
och lever till processlut, även för returnerade strängar och vyer.
Kopior av string delar data säkert genom oföränderligheten. Sammanslagning,
snitt och konvertering av heltal/rune kopierar data till arenan. Ingen GC
eller återvinning av enskilda strängar under körning finns ännu; många
upprepade operationer kan därför fylla arenan även när gamla värden inte
längre används. Gränsen räknar också metadata och justering.

Runtimefel: 103 för ogiltiga index eller intervall, 105 för full arena eller
misslyckad mappning, 106 för snitt mitt i en UTF-8-sekvens eller ogiltigt
`rune(number)`. Kompilering och körning använder fortfarande ingen extern
kompilator, länkare, signerare eller strängruntime.

## Nästa språkbeslut

Nästlade aggregat, arrayslices, allmänt allokerings-API,
ägande/lån och egna iteratorer ska utvecklas genom riktiga program och
feltester. Dogfood-arenan är tills vidare indexbaserad bump-allokering över
en fast typad buffert. Självhosting är ett separat senare steg.
