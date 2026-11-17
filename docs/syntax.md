# Dunders syntax

Källfiler använder **`.ddr`**, till exempel `main.ddr` och `functions.ddr`.

## Deklarationer och muterbarhet

```dunder
count := 42
wide: i64 = 42
ready: bool = true

mut total := 0
mut big: i64 = 0
total += 1
```

`:=` deklarerar ett nytt namn i det aktuella blocket och härleder dess typ.
`: Typ =` gör samma sak med en explicit typ. Ett nytt `:=` med samma namn i
samma scope är ett fel. `_ := expr` kastar bort ett returvärde utan att binda
ett läsbart namn och kan upprepas i samma scope. Ett inre block får skugga ett
yttre namn.

I värdeuttryck väljs en aktiv lokal bindning före ett namngivet typnamn.
En arrayparameter `Values` kan därför använda `Values.len` även om det
finns en enum som heter `Values`. Typannotationer använder typnamnens
eget uppslag. Regeln gäller både filbaserade byggen och kompilatorns
direkta käll- och token-API, oberoende av optimeringsläge.

Bindningar och funktionsparametrar är oföränderliga som standard. `mut` är
det explicita valet av muterbarhet. Typen ändras aldrig efter deklarationen.
All deklaration kräver initiering. Det finns inga oinitierade lokaler i denna
delmängd. Oföränderlighet kräver inte att uttrycket kan beräknas vid byggtid.
`var` är ett vanligt namn och kan användas för lokaler, parametrar och importalias.

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
| `[]i64` | En dynamisk typad array. |
| `&Point` | Ett läslån som parameter eller oföränderlig lokal referens. |
| `&mut Point` | Ett exklusivt skrivlån som funktionsparameter. |
| `Point` / `geometry.Point` | En namngiven struct i aktuell eller importerad modul. |
| `Command` / `sys.ReadResult` | En nominell enum med namngivna värden eller payload-varianter. |
| `Result<T, E>` | [Typad enum för framgång och fel](enums.md#typade-fel-med-result). |
| `range` | Ett halvöppet intervall med `i32`-gränser. |
| `TaskID<R>` | En kopierbar resultatnyckel till en [isolerad uppgift](tasks.md). |

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
språkoperation. På de nativa målen skriver en misslyckad `assert` källfil,
radnummer och originaluttryck till stderr före exit 104, till exempel
`src/main.ddr:7: assertion failed: answer == 42`. Samma diagnostik gäller
booleska testuttryck i `@test`. Villkoret utvärderas en gång; godkända
assertioner skriver ingenting. Övriga källkopplade runtime-diagnoser återstår.

### Stränginterpolering

`f"..."` och `f'...'` innehåller vanliga Dunder-uttryck mellan `{` och `}`:

```dunder
name := "å 🌍"
count := 3
message := f"{name}: {count + 1} objekt, redo={true}"
label := f"status: {if count > 0 { "aktiv" } else { "tom" }}"
escaped := f"{{{count}}}"
```

Hålen kan innehålla anrop, fältåtkomst, indexering, `if`, `match` och
nästlade interpolerade strängar. De utvärderas exakt en gång i källordning.
Tidigare strängvärden hålls levande även om ett senare hål ersätter deras
ursprungliga variabel. Resultatet är en vanlig ägd `string`.

Tillåtna resultattyper är `string`, signerade och osignerade heltal,
`usize`, `bool`, `rune`, `f32` och `f64`. Formateringen motsvarar `.str()`;
för en sträng delar `.str()` dess oföränderliga text. Aggregat, råpekare och
funktioner kräver att programmet först väljer en textrepresentation.
Formatkoder som `:02` ingår inte.

`{{` och `}}` ger bokstavliga klamrar. I exemplet blir `escaped` strängen `"{3}"`.
Övriga escapes fungerar som i vanliga strängar, inklusive Unicode och NUL. Textsegment får inte innehålla råa
radbrytningar; uttrycken kan sträcka sig över flera rader. Ett tomt hål,
en ensam `}`, obalanserade delimiters eller en oavslutad interpolation ger
ett kompileringsfel på originalpositionen. Nästlade interpolerade strängar
har en gräns på 64 nivåer. En sträng utan `f` behåller alltid sin vanliga
tolkning, även när den innehåller klamrar.

## Funktioner och kontrollflöde

[Fibonacci-exemplet](../examples/fibonacci.ddr) visar en funktion med typad
parameter, returvärde och en loop över ett intervall.

Parameter- och returtyper i funktionssignaturer är explicita. Framåtanrop,
rekursion och ömsesidig rekursion tillåts. Parametrarna är oföränderliga.
Vanliga sträng- och aggregatparametrar lånar indata för läsning under
anropet; skalärer kan överföras som billiga värden. Explicit `&T` fungerar
också för läslån. `&mut T` kräver ett uttryckligt skrivlån vid anropet.
Lokala läsreferenser kan namnges med `r := &value` eller `r: &T = &value`.
Fält och arrayelement går också att låna. Referensen är oföränderlig och
gäller till deklarationsblockets slut, även efter sista användningen.
Oberoende syskonfält får ändras; ett elementlån låser hela arrayen.
Flytt, `replace` och motstridiga skrivningar av lånad lagring avvisas.

Temporära uttryck får lånas direkt vid anrop, exempelvis `read(&f())` och
`sum(&[1, 2])`, och hålls levande tills anropet är klart. De får inte bindas
som lokala referenser. En funktion kan returnera ett läslån från en av sina
delade parametrar. Finns det flera anger `from namn` vilken parameter
returvärdet lånar från.

```dunder
fn tail(values: &[]i32) -> &[]i32 { &values[1..] }
fn first(a: &i32, b: &i32) -> &i32 from a { a }
```

I en funktionstyp anges källan med ett ettbaserat parameternummer, exempelvis
`fn(&i32, &i32) -> &i32 from 1`. Även anrop via en funktionsvariabel
kontrollerar sambandet mellan argument och resultat.
[Låne- och escapegränser](memory.md#lokala-läslån-och-intervallvyer).

En funktion som ska behålla resursen deklarerar `move`:

```dunder
fn keep(move values: []i32) -> []i32 { values }
fn length(values: []i32) -> i32 { values.len }

fn main() {
    mut a: []i32 = [1, 2, 3]
    print(length(a))
    b := a
    c := keep(move b)
    print(length(c))
}
```

Tilldelningen flyttar arrayen från `a` till `b` och anropet lämnar den vidare
till `keep`. Litteraler och färska ägda returvärden kan lämnas direkt till en
`move`-parameter. En `move`-parameter av kopierbar skalär typ fungerar som
ett vanligt skalärvärde. Funktionstyper följer samma regel:
`fn([]i32) -> i32` lånar, medan `fn(move []i32) -> []i32` tar över en array.
Dessa funktionstyper är olika och kan inte bytas mot varandra.

`replace(&mut place, replacement)` returnerar det tidigare värdet och
installerar ersättaren. Det fungerar även bakom en `&mut`-parameter och för
fält och arrayelement. Ersättaren följer samma regel som en `move`-parameter:

```dunder
fn drain(values: &mut []i32) -> []i32 { replace(&mut values, []) }

fn main() {
    mut values: []i32 = [20, 22]
    previous := drain(&mut values)
    assert(previous.len == 2 && values.len == 0)
    next: []i32 = [42]
    empty := replace(&mut values, move next)
}
```

Destinationen måste redan vara initierad. Ersättningsuttrycket får inte
flytta eller mutera samma rot som destinationen. [Minne och utvärderingsordning](memory.md#fält-element-och-begränsningar).

De första åtta argumenten överförs i register och ytterligare argument på
stacken. Nativ C-FFI följer det deklarerade C-ABI:t.

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
Ett vanligt block i en funktion utan returvärde fortsätter till nästa sats.

### Villkorsuttryck

Ett `if`-uttryck kan ge värdet till en bindning, ett funktionsargument eller
en operand. Varje gren har ett eget block. Dess sista uttryck utan semikolon
ger grenens värde.

```dunder
fn biljettpris(alder: i32, grundpris: i32) -> i32 {
    pris := if alder < 18 {
        rabatt := grundpris / 2
        grundpris - rabatt
    } else { grundpris }
    return pris + 5
}
```

Villkoren har typen `bool` och prövas i ordning. Bara den valda grenen körs.
Ett `if` i uttrycksposition kräver ett avslutande `else`. Alla vägar som når
slutet av uttrycket måste ge värden av samma typ. Ett `return` lämnar hela
funktionen. Inuti en loop kan en gren också lämna varvet med `break` eller
`continue`. Om samtliga grenar lämnar uttrycket behöver en bindning en
uttrycklig typ, exempelvis `x: i32 = ...`.

Ett ägt slutvärde flyttas ut innan grenens `defer` körs och dess lokaler
städas. Grenen får använda lokala lån. Kompilatorn avvisar ett lån som
grenens slutvärde.

För en enum kan ett `match`-uttryck välja värde efter variant och binda
variantens payload i den valda armen. [Match som uttryck](enums.md#match-som-uttryck)

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
`mut transform := double` tillåter byte till en annan funktion med samma typ.
Funktioner kan ta och returnera andra funktionsvärden, även i flera nivåer.
Anrop utvärderar först funktionen och därefter argumenten från vänster till
höger. Inbyggda operationer som `print` är ännu inte funktionsvärden; skriv
en vanlig `fn` som omslag vid behov. Funktionsvärden saknar likhetsoperator.

En anonym funktion skrivs `fn(x: i32) -> i32 { x * 2 }`. Lägg till en
typad lista i hakparenteser för att fånga lokala värden. En closure har
samma funktionstyp som en namngiven funktion med samma parametrar och resultat.

```dunder
fn plus(base: i32) -> fn(i32) -> i32 {
    fn[base: i32](x: i32) -> i32 { x + base }
}

fn main() {
    ten := plus(10)
    print(ten(32))  // 42
}
```

Listan utvärderas när closuren skapas. Infångade värden är skrivskyddade
i kroppen och lever så länge något funktionsvärde använder dem. Vanliga
äganderegler gäller vid infångning. Heltal kopieras, medan en ägd sträng
eller dynamisk array flyttas in. Skapa en separat lokal med `.clone()` om
båda behöver äga värdet. En closure kan fånga andra funktionsvärden.
Lån får inte lagras i capture-listan.

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
import på toppnivå med **punkter, utan citattecken eller filändelse**:

```dunder
import math
import geometry.vector as vector
import ..shared.config
import std.map
```

`import sys` väljer det inbäddade systembiblioteket för macOS/Linux; se [sys](sys.md).
Det kan aliaseras och dedupliceras precis som andra moduler. En egen lokal
fil med samma namn importeras uttryckligt som `import .sys as local`.

Den första importen läser `math.ddr` bredvid den importerande filen. Den andra
läser `geometry/vector.ddr` relativt samma fil. Sökningen beror inte på
processens arbetskatalog. Punktnotation översätts till kataloger när filen läses.

| Import | Var modulen finns |
| --- | --- |
| `std.map` | Det inbäddade standardbiblioteket. |
| `geometry.vector` | `geometry/vector.ddr` relativt den importerande filen. |
| `.sys` | `sys.ddr` i samma katalog, även när namnet finns i standardbiblioteket. |
| `..shared.config` | `shared/config.ddr` en katalog upp. |
| `...shared.config` | `shared/config.ddr` två kataloger upp. |

En inledande punkt väljer uttryckligen den lokala katalogen; varje ytterligare
inledande punkt går en katalog upp.
Absoluta sökvägar och suffix i importer avvisas. Utan `as` kommer aliaset
från det sista sökvägssegmentet; `import geometry.vector` får alias `vector`.
Ett alias måste vara ett giltigt, icke reserverat namn. Till exempel kräver
`import foo-bar as foo` ett explicit alias eftersom bindestreck inte ingår i namn.

Modulnamnet skrivs sammanhängande, utan mellanslag runt punkterna. Skriv mellanslag
före `as` eller en efterföljande kommentar: `import std.map // uppslagstabell`.
Tomma segment inne i namnet, som `std..map`, och en avslutande punkt avvisas.
Äldre importer med snedstreck eller citattecken stöds för kompatibilitet;
de behandlas fortfarande som filsökvägar och behövs inte för nya importer.

I `math.ddr` anropar den publika funktionen `double` den privata funktionen `scale`.

```dunder
pub fn double(value: i32) -> i32 {
    scale(value, 2)
}

fn scale(value: i32, factor: i32) -> i32 {
    value * factor
}
```

I `main.ddr` används `math.double` som funktionsvärde.

```dunder
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
Ett nytt bygge läser om beroenden. Manifestet kan ange [lokala kataloger och Git-källor](dependencies.md); Git-källor låses till commits i `dunder.lock`. `--incremental` återanvänder verifierade funktionsfragment.

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
    mut moved := original
    moved.x = moved.x + 1
    print(length_squared(Point { x: 3, y: 4 }))
    print(original.x)
}
```

Structar är nominella: olika deklarationer ger olika typer även med samma
fält. Importerade typer behåller sin identitet över moduler och i callbacks.
Alla fält måste anges exakt en gång; deras ordning i uttrycket är valfri.
Fältuttrycken utvärderas i den ordning de står i källan. Varje fält får vara
en skalär typ, ett funktionsvärde, en fast/dynamisk array, en struct eller
en enum. Inline-cykler avvisas; referensfält är ännu inte tillåtna.

En struct med enbart kopierbara fält kan kopieras vid bindning och
tilldelning. Resursägande structar flyttas i stället. Vanliga
funktionsparametrar lånar structen för läsning.
Oberoende resursfält kan flyttas separat, även i nästlade structar. Ett
flyttat fält i muterbar lagring kan återinitieras med tilldelning; hela
structen kan användas igen när alla fält är initierade.
`@resource` markerar en unik extern resurs även när dess fält bara är
skalärer. En sådan struct och aggregat som innehåller den flyttas och
kan inte klonas. Skriv `@resource(dispose)` för att välja en
[automatisk stängningsmetod](memory.md#automatisk-stängning).
Ändringar i `moved` påverkar därför inte `original`. Parametrar är
oföränderliga; kopiera till en `mut` för att ändra fält. Structar kan också
returneras från funktioner. Anroparen äger resultatets lagring, så en
returnerad lokal lämnar ingen pekare till avslutad stacklagring. Detta
fungerar även för callbackreturer, rekursion och importerade nominella typer.

I `if`-/`while`-villkor och `for`-huvuden behöver ett direkt struct-uttryck
parenteser runt sig för att skiljas från kontrollflödets block. I vanliga
anropsargument, deklarationer och testfall behövs inga extra parenteser.

### `impl`-block och metoder

Structar och enums kan definiera associerade funktioner och metoder i ett eller
flera `impl`-block i samma modul som deklarerar typen:

```dunder
pub opaque struct Counter {
    value: i32,
}

impl Counter {
    pub fn new(start: i32) -> Self {
        Self { value: start }
    }

    pub fn get(&self) -> i32 {
        self.value
    }

    pub fn inc(&mut self) {
        self.value = self.value + 1
    }

    pub fn into_value(self) -> i32 {
        self.value
    }
}
```

- `impl Typ { ... }` får endast förekomma i den modul som deklarerar `Typ`
  (inte för importerade typer eller inbyggda primitiver). `impl`-blocket
  kan inte markeras med `pub`; synlighet styrs per metod med `pub fn` (privat
  `fn` utan `pub` kan endast anropas inom typens modul).
- Inuti ett `impl`-block refererar `Self` till måltypen i typer, structliteraler
  (`Self { ... }`), enumvarianter (`Self.Variant`) och associerade anrop
  (`Self::new(...)`).
- Första parametern kan vara `&self` (delat lån), `&mut self` (muterbart lån),
  `self` eller `move self` (värdeöverföring; `mut self` / `move mut self` tillåter
  även mutering av `self`), eller explicit `self: &Self`, `self: &mut Self` / `self: Self`.
- Funktioner utan `self` anropas med `Typ::namn(...)` (eller via modulalias
  `mod.Typ::namn(...)`) och kan tas som funktionsvärden (`Typ::namn`). Metoder
  med `self` anropas med `val.namn(...)` eller med UFCS `Typ::namn(&val, ...)`.
- `@requires(...)` och `unsafe fn` stöds på enskilda metoder i `impl`, men
  `@test` är reserverat för fristående testfunktioner på modulnivå.

Ett fält eller metodanrop kan fortsätta på nästa rad. Ge raden med
punkten större indrag än raden där satsen började. Följande anrop i
samma kedja kan ha samma indrag.

```dunder
fn tag(a: u8, b: u8, c: u8, d: u8) -> u32 {
    u32(a).shl(24)
        .bit_or(u32(b).shl(16))
        .bit_or(u32(c).shl(8))
        .bit_or(u32(d))
}
```

En punkt på satsens ursprungliga indrag kan börja ett eget uttryck, till
exempel en enumvariant som `.Ok(text)`. Ett semikolon avslutar alltid satsen.

## Arrayer, intervall och iteratorer

```dunder
fn sum(values: [i32; 4]) -> i32 {
    mut result := 0
    for value in values { result = result + value }
    result
}

fn main() {
    mut values: [i32; 4] = [0; 4]
    for i in 0..len(values) { values[i] = i + 1 }
    snapshot := values.clone().iter()
    values[0] = 99
    for value in snapshot { print(value) }
    print(sum([1, 2, 3, 4]))
}
```

`[a, b, c]` bygger en array. `[value; N]` utvärderar `value` en gång och
upprepar värdet. Längden måste vara en heltalsliteral. Elementtypen är
en lagringsbar typ som ett primitivt värde, en struct, en enum eller en
annan array. Den härleds eller hämtas från förväntad arraytyp.
En fast array får använda högst 64 KiB. Tomma arrayliteraler kräver explicit typ.

Arrayer flyttas vid bindning och retur. Vanliga parametrar lånar för
läsning. `mut` krävs för att tilldela ett element. Index har typen `i32`; negativa index och index vid
eller efter längden ger runtimefel. `len(values)` och `values.len()` ger
längden som `i32`. Fasta arrayer kan returneras genom ägaröverföring på samma sätt som
andra resursägande värden.
`values.swap(i, j)` byter två element utan kloning och kräver muterbar lagring.
`values.remove(i)` flyttar ut och returnerar elementet vid index `i` i en
muterbar dynamisk array. Efterföljande element förskjuts åt vänster med
bibehållen ordning.

`middle := &values[start..end]` skapar en lånad `&[]T` utan heapallokering
eller retain. En explicit annotering, `middle: &[]i32 = &values[1..3]`, går
också bra. Vyn använder originalets adress, och dess `len` och `cap()` är
intervallängden. Fasta arrayer och redan lånade vyer kan skäras på samma sätt.
Gränserna utvärderas en gång från vänster till höger; ett intervall utanför
`0 <= start <= end <= values.len` ger status 103. Vyn är oföränderlig och
lånar hela arrayen till blockslut. `.clone()` ger en egen ägd array.

`start..end` har typen `range`, inkluderar start och utesluter slut.
Gränserna är `i32`, utvärderas en gång och iterationen går uppåt.
Om start är större än eller lika med slut blir det noll varv. Intervall kan
lagras, skickas och returneras som vanliga värden.

`for` stöder intervall, arrayer och `.iter()`-värden. Loopvariabeln är
oföränderlig och lokal till loopen. `break` lämnar närmaste loop; `continue`
går till nästa element. Vanlig iteration lånar samlingen för läsning och
avvisar mutation eller flytt av samma samling under loopen.
`for value in move values` konsumerar ägaren.

`for value in values.iter()` lånar också. En lagrad ägande iterator skapas
med `(move values).iter()` eller från en uttrycklig klon, som i exemplet
ovan. Iteratorn kan läslånas flera gånger; bindning till en ny ägare flyttar
den. Iteratorns typ härleds med `:=`; den saknar ännu egen typstavning,
`.next()` och användardefinierat iteratorprotokoll.

## Taggade tester

```dunder
@test {
    (20) -> 4
    (10 + 5) -> 2 + 1
    mut x := 0
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
`mut text := "a"; text = "b"` byter hela värdet; direkt ändring av ett byte
är förbjuden. Strängar kan skickas och returneras, även genom callbacks,
och lagras i arrayer och structfält. Bindningar och returer flyttar ägandet;
en uttrycklig `.clone()` delar den oföränderliga texten med en ny ägare.

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
avvisas. Oescapade radbrytningar och råsträngar stöds ännu inte.
NUL är tillåten i strängdata; längden avgör slutet, ingen nollterminering används.

`f"Hej {name}, {count + 1}!"` och `f'Hej {name}!'` interpolerar uttryck
från vänster till höger med samma konvertering som `.str()`. Uttrycken
lånar sina operander; att formatera en sträng flyttar inte dess ägare.
`{{` och `}}` ger bokstavliga klamrar. Vanliga strängliteraler interpolerar
inte. Fel i uttrycken pekar på deras ursprungliga källpositioner.

| Operation | Betydelse |
| --- | --- |
| `text.len`, `text.len()`, `len(text)` | Antal byte, `i32`, konstant tid. |
| `text[i]`, `text.bytes()[i]` | Ett byte som `i32` i intervallet 0–255. |
| `text[start..end]` | Ny sträng från halvöppna bytegränser. Båda måste vara UTF-8-gränser. |
| `&text[start..end]` | Lånad bytevy av typen `&[]u8`, utan ny sträng eller retain. |
| `text.runes()[i]` | En rune efter avkodning från början; linjär tid. |
| `text.runes().len` | Antal kodpunkter; linjär tid. `.len()` och `len(...)` fungerar också. |
| `for r in text.runes()` | Avkodar kodpunkter i ett linjärt pass. |
| `for byte in text.bytes()` | Itererar byte i ett linjärt pass. |
| `a + b` | Bygger en ny sträng av två strängar. |
| `a == b`, `a != b` | Jämför innehåll, inklusive eventuella NUL. |
| `<`, `<=`, `>`, `>=` | Lexikografisk ordning efter UTF-8-byte, utan språklig sortering. |
| `value.str()` | Sträng av heltal, flyttal, bool eller rune. På string ges samma värde. |

`.bytes()` och `.runes()` ger självständiga arrayer av typen `[]i32`
respektive `[]rune`. Direkt iteration kan strömma från en behållen
strängsnapshot utan mellanarray. Om originalbindningen får en ny sträng
behåller konverteringen eller iterationen sina gamla värden. `for` direkt över en string avvisas:
välj `.bytes()` eller `.runes()` uttryckligt. `break` och `continue` fungerar
för båda. Lagrade arrayiteratorer äger sin samling; skapa dem med
`(move values).iter()` eller `values.clone().iter()`.
Strängsnitt kan också ta ett lagrat eller returnerat `range`-värde,
exempelvis `span := 7..11; part := text[span]`.
En namngiven textvy, exempelvis `utf8 := &text[1..7]` för `"xå🌍y"`,
behåller originalets adress. Dess `len` och `cap()` är sex byte. Båda
gränserna måste vara UTF-8-gränser även när vyn är tom. Intervallfel ger
103 före UTF-8-kontrollen som ger 106. `.clone()` av vyn ger en ägd `[]u8`.

En rune är en Unicode-kodpunkt, inte ett grapheme-kluster. Exempelvis har
`"é"` en rune och `"e\u{301}"` två; strängarna jämförs som olika.
Normalisering, case folding, fullständig Unicode-klassificering och synliga
teckenkluster stöds ännu inte. Dogfood-ordräknaren känner bara igen de
blanktecken som uttryckligen listas i dess Dunder-källa.

Literalerna ligger i programbilden. Dynamiska strängar delar minnespool med
arrayer och frigörs när sista ägaren försvinner. Sammanslagning, snitt och
konvertering av heltal eller runor skapar nya strängar. Temporärer behåller
sina värden över senare sidoeffekter i samma uttryck. `.bytes()` och `.runes()`
behåller sina ursprungliga element om källsträngen ersätts under iterationen.

Ogiltiga index och intervall ger status 103. Strängoperatorerna ger 105 vid
misslyckad allokering och 106 för ett snitt mitt i en UTF-8-sekvens eller
ett ogiltigt `rune(number)`. [Minneskontrakt och körbara prov](memory.md).

## Råtyper, privilegier och C-layout

`u8`, `u16`, `u32`, `u64`, `usize` och `ptr` kan användas i variabler,
fält och samlingar. `@c_layout struct` anger C-fältlayout. En modul med
`capability unsafe` kan använda `unsafe fn`, `unsafe { ... }`, `@extern`
och typade C-funktionspekare. `capability filesystem` ger tillgång till att
skapa filhandtag. En `pub opaque struct` exponerar typen och låter den
definierande modulen äga konstruktion och fältåtkomst.
[Språkkontrakt och exempel](ffi.md)
