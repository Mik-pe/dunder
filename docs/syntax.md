# Dunders första implementerade syntax

Beslut 2026-11-10: prioritera det typade källspråket och kompilera riktiga
program. Håll stavning och struktur regelbundna så att både människor och
kodagenter kan använda vanliga mönster för funktioner, uttryck och block.
Detta är ett prövbart designval, inte ett påstående om mätt agentpreferens.

Källfiler använder **`.dn`**, till exempel `main.dn` och `functions.dn`.

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
| `bool` | `true` eller `false`; ingen implicit konvertering till/från heltal. |
| `()` | Inget returvärde; implicit returtyp för funktioner utan `->`. |
| `fn(i32) -> i32` | Ett funktionsvärde med angivna parameter- och returtyper. |

Literaler får typ från en annotering, funktionens parameter/returtyp eller
den andra heltalsoperanden. Befintliga värden konverteras aldrig implicit
mellan `i32` och `i64`. `i64(value)` breddar explicit; `i32(value)` smalnar
med en körningskontroll. En literal utanför sitt typintervall är ett typfel.

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
102. Reglerna gäller även optimerade byggen. Wraparound är ännu inte en
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

`dunder run [fil.dn]` bygger och kör den nya binären. Byggmeddelanden går
till stderr och programmets stdout bevaras. Ett kompileringsfel startar
aldrig en tidigare binär från samma utdataplats.

`if` och `while` kräver `bool`, aldrig ett heltal. `else if`, `else`, `break`
och `continue` stöds. Block ger lexikala scope. `print` är den första inbyggda
I/O-operationen: ett `i32`, `i64` eller `bool`, följt av radbrytning.

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

Den första importen läser `math.dn` bredvid den importerande filen. Den andra
läser `geometry/vector.dn` relativt samma fil. Sökningen beror inte på
processens arbetskatalog. Citerade relativa sökvägar kan innehålla `../`.
Absoluta sökvägar och suffix i importer avvisas. Utan `as` kommer aliaset
från det sista sökvägssegmentet; `import "geometry/vector"` får alias `vector`.
Ett alias måste vara ett giltigt, icke reserverat namn. Till exempel kräver
`import "foo-bar" as foo` ett explicit alias eftersom bindestreck inte ingår i namn.

```dunder
// math.dn
pub fn double(value: i32) -> i32 {
    return scale(value, 2)
}

fn scale(value: i32, factor: i32) -> i32 {
    return value * factor
}
```

```dunder
// main.dn
import math

fn main() {
    transform: fn(i32) -> i32 = math.double
    print(transform(21))
}
```

`fn` är privat; `pub fn` exporterar en namngiven funktion. Samma fil kan
anropa sina privata funktioner. Andra moduler får bara använda dess publika
funktioner via sitt importalias. Funktionsvärden och deras strukturella typer
fungerar likadant över modulgränser som inom en fil.

En import kopierar inga namn till aktuell namnrymd. `math.double` fungerar;
`double` kräver en egen lokal bindning, exempelvis `double := math.double`.
Alias får inte krocka med ett annat importalias eller en toppnivåfunktion i
samma fil. Lokala värden får skugga ett alias; det skuggade namnet ger då
inte längre tillgång till modulen i det scopet. En moduls egna importer
exporteras inte vidare. Varje fil importerar sina egna beroenden uttryckligt.

Moduler har i denna version bara funktioner, inga globala värden eller
initialiseringskod. Signaturer från hela grafen fastställs före kropparna.
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

## Nästa språkbeslut

Poster och fältåtkomst, strängar, arrayer/slices, ägande och
lån ska utformas genom riktiga program och feltester. Inga syntaxer för dem
utlovas av denna version. Självhosting och den minimala frökedjan är separata
senare projektsteg och blockerar inte användningen av källspråket här.
