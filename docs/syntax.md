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

### Konstanter på modulnivå

Konstanter kan deklareras på modulnivå med `const` (och exporteras med
`pub const`), antingen som skalärer eller konstanta fasta tabeller i
programbilden:

```dunder
const LIMIT: i32 = 64
pub const WEIGHTS: [i32; 4] = [10, 20, 30, 40]
const ZEROS: [u8; LIMIT] = [0; LIMIT]
```

Heltalskonstanter kan även ge längden i fasta arraytyper som `[u8; LIMIT]`.
Konstanta tabeller byggs vid kompilering och delas som skrivskyddade data utan
att någon ny array allokeras vid varje anrop. `@resource`-typer och körtidsberoende
uttryck tillåts inte i `const`.

## Typer och uttryck

| Typ | Betydelse |
| --- | --- |
| `i16` | Signerat 16-bitars heltal från −32768 till 32767. |
| `i32` | Signerat 32-bitars heltal; standard för okontextualiserade heltalsliteraler. |
| `i64` | Signerat 64-bitars heltal. |
| `f32` | IEEE 754 binary32. |
| `f64` | IEEE 754 binary64; standard för okontextualiserade flyttalsliteraler. |
| `bool` | `true` eller `false`; ingen implicit konvertering till/från heltal. |
| `string` | Oföränderlig, giltig UTF-8. |
| `&str` | Scope-bunden lånad, oföränderlig UTF-8-vy av en sträng (t.ex. `&text[a..b]`, `text.as_str()`; ej lagringsbar i struct/enum/array). |
| `u8` .. `u64`, `usize` | Osignerade heltal (bland annat `b'a'`-byteliteraler och `text[i]: u8`). |
| `rune` | En Unicode-kodpunkt; skild från heltal och sträng. |
| `()` | Inget returvärde; implicit returtyp för funktioner utan `->`. |
| `fn(i32) -> i32` | Ett funktionsvärde med angivna parameter- och returtyper. |
| `[i32; 8]` | En array med åtta element; längden ingår i typen. |
| `[]i64` | En dynamisk typad array. |
| `&Point` | Ett läslån som parameter eller oföränderlig lokal referens. |
| `&mut Point` | Ett exklusivt skrivlån i en parameter, lokal bindning eller retur från en muterbar parameter. |
| `Point` / `geometry.Point` | En namngiven struct i aktuell eller importerad modul. |
| `Command` / `sys.IoError` | En nominell enum med namngivna värden eller payload-varianter. |
| `Option<T>` | [Preludiets optionstyp (`Some(T)`, `None`)](enums.md). |
| `Result<T, E>` | [Typad enum för framgång och fel](enums.md#typade-fel-med-result). |
| `range` | Ett intervall med `i32`-gränser; `a..b` är halvöppet, `a..=b` inkluderar båda gränserna. |
| `TaskID<R>` | En kopierbar resultatnyckel till en [isolerad uppgift](tasks.md). |

Literaler får typ från en annotering, funktionens parameter/returtyp eller
den andra heltalsoperanden. Befintliga värden konverteras aldrig implicit
mellan `i16`, `i32` och `i64`. Konvertering till en smalare typ, exempelvis
`i16(value)`, kontrollerar att värdet ryms. En literal utanför sitt typintervall är ett typfel.
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
För sammansatta och egendefinierade typer skickas `==`/`!=` vidare till traiten
`Eq` (`eq`/`ne`) och `<`/`<=`/`>`/`>=` till traiten `Ord` (`lt`/`le`/`gt`/`ge`)
genom att låna operanderna utan att flytta ägarskap.

Spill i `+`, `-`, `*`, unärt minus, divisionen `MIN/-1` och avsmalnande
konvertering avslutar processen med status 101. Division/rest med noll ger
102. Ogiltiga arrayindex ger 103 och en misslyckad `assert` ger 104.
Konvertering från `i32` till en enum utan payload kontrollerar deklarerade
värden och ger 108 för okända värden; se [enums och match](enums.md).
Reglerna gäller även optimerade byggen. För uttrycklig omslagsaritmetik finns
metoderna `.wrapping_add`, `.wrapping_sub` och `.wrapping_mul` (infixoperatorer
med wraparound saknas).

### Assertioner

På de nativa målen skriver en misslyckad `assert` källfil,
radnummer och originaluttryck till stderr före exit 104. När villkorets
yttersta operation är en jämförelse av skalärvärden skrivs båda värdena.
En enum utan payload, en rune och en råpekare visas med sitt heltalsvärde.

```text
src/main.ddr:7: assertion failed: answer == 42
  left = 41
  right = 42
```

Samma diagnostik gäller booleska testuttryck i `@test`. Villkoret
utvärderas en gång och godkända assertioner skriver ingenting.
WebAssembly använder en trap vid misslyckade assertioner.

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

Tillåtna resultattyper är `string` och `&str`, signerade och osignerade
heltal, `usize`, `bool`, `rune`, `f32` och `f64`. Standardformatet motsvarar
`.str()`; för en sträng delar `.str()` dess oföränderliga text och för en
`&str`-vy skapas en ägd `string`. Aggregat, råpekare och
funktioner kräver att programmet först väljer en textrepresentation.

Heltal kan formateras med `:d` för decimal, `:x` för gemena hexadecimala
siffror och `:X` för versaler. En bredd före formatkoden fyller ut med
blanksteg eller, om den börjar med `0`, med nollor. Om bokstaven utelämnas
används decimalformat. Bredden är ett minimum och räknar med eventuellt
minustecken. Negativa värden visas med minustecken även i hexadecimal form.

```dunder
name := "å 🌍"
color := 0xaf
hex := f"#{color:04X}"      // "#00AF"
position := f"{-7:04d}"    // "-007"
count := f"{3:02}"         // "03"
left := f"{name:<6}"       // vänsterjusterad till bredd 6
right := f"{name:>6}"      // högerjusterad till bredd 6
```

För `f32` och `f64` ger `:.Nf` fast decimalform med exakt N decimaler
(0–1074). `f"{1.25:.3f}"` ger `1.250` och `f"{2.5:.0f}"` ger `2`.
Avrundningen använder flyttalets exakta värde och väljer jämn slutsiffra
vid lika avstånd. Negativ noll behåller sitt tecken. Specialvärden skrivs
`nan`, `inf` och `-inf`.

För strängar stöder formatangivelsen bredd och justering med `:<N`
(vänsterjustering) och `:>N` (högerjustering), som fyller ut med blanksteg upp
till minst `N` kodpunkter om strängen är kortare.

`{{` och `}}` ger bokstavliga klamrar. I exemplet blir `escaped` strängen `"{3}"`.
Övriga escapes fungerar som i vanliga strängar, inklusive Unicode och NUL. I en enradig `f"..."` får textsegmentet inte innehålla råa
radbrytningar, men uttrycken kan sträcka sig över flera rader; flerradiga
`f"""..."""` tillåter råa radbrytningar i textsegmenten. Ett tomt hål,
en ensam `}`, obalanserade delimiters eller en oavslutad interpolation ger
ett kompileringsfel på originalpositionen. Nästlade interpolerade strängar
har en gräns på 64 nivåer. En sträng utan `f` behåller alltid sin vanliga
tolkning, även när den innehåller klamrar.

## Generiska deklarationer

Funktioner på modulnivå, structar och enums kan deklarera en eller flera
typparametrar. Vid anrop till generiska funktioner kan typargumenten
antingen skrivas uttryckligen mellan `<` och `>` eller härledas automatiskt
från argumenten (eller en förväntad returtyp) när alla typparametrar framgår,
till exempel `array.reverse(&mut v)`, `array.contains(&v, 42)` eller
`identity("Dunder")`. Detsamma gäller associerade funktioner på generiska
typer, såsom `Box::new(42)` eller med typannotering på variabel eller returtyp
(`mut m: Map<string, i32> = Map::new()` och `fn mk() -> Map<string, i32> { Map::new() }`).
Explicita `<Typargument>` krävs när typerna inte framgår lokalt — exempelvis
vid en oannoterad `mut m := Map::new()` (`Map<string, i32>::new()`) — eller
när funktionen används som ett funktionsvärde utan anrop.
Ett `impl` för en generisk typ binder typens parametrar i samma ordning.

```dunder
struct Box<T> { value: T }
enum Maybe<T> { Some(T), None }

impl Box<T> {
    fn new(move value: T) -> Self { Self { value: move value } }
}

fn identity<T>(move value: T) -> T { value }

fn main() {
    box := Box::new(identity("Dunder"))
    maybe := Maybe<Box<string>>.Some(move box)
    match maybe {
        .Some(b) => { print(b.value) },
        .None => {},
    }
}
```

Kompilatorn skapar en specialisering för varje kombination av deklaration
och konkreta typargument som programmet använder. Kroppen typkontrolleras
med de konkreta typerna. En metod som anropas på ett värde av typen `T`
måste därför finnas och vara åtkomlig för den valda typen. Flyttar, lån och
resursstädning följer de vanliga reglerna i varje specialisering.
En specialiserad funktion kan skickas vidare som funktionsvärde,
exempelvis `identity<i32>`. Importerade deklarationer används på samma sätt,
exempelvis `lib.Box<[]i32>`. [Körbart exempel med två moduler](../tests/fixtures/generics/main.ddr)

Ett `where` efter typens parameterlista anger krav som gäller så fort typen
används. Krav kan anges som traitgränser (`T: Trait1 + Trait2`), direkt i
parameterlistan (`<T: Trait1 + Trait2>`), som `storable(T)` (motsvarar
`T: Storable`) eller som enskilda metodkrav (`T.size: fn(T) -> i32`).
Ett metodkrav anger namnet och hela funktionstypen inklusive mottagaren;
metoden måste vara publik och säker och kunna anropas utan ytterligare
rättigheter.

På en generisk funktion kan `where` stå efter signaturen. På ett
`impl`-block står det efter mottagartypen, exempelvis
`impl Box<T> where T.size: fn(T) -> i32` eller
`impl Describe for Box<T> where T: Describe`. För vanliga `impl`-block
kontrolleras kraven när en av dess metoder används. För villkorliga
traitimplementationer aktiveras implementationen enbart för de typargument
som uppfyller kraven.

### Traits och `impl Trait for Type`

En `trait`-deklaration namnger ett statiskt kontrakt med noll eller flera
metodsignaturer (säkra `fn` eller `unsafe fn` i en modul med `capability unsafe`)
samt valfria supertraits efter `:`. Metodsignaturer får
innehålla `Self` även inuti generiska typer som `Result<Self, string>`, och en
traitmetod kan ha en standardkropp (`fn ne(&self, other: &Self) -> bool { ... }`)
som syntetiseras automatiskt när en implementation inte ersätter den. En
implementation i `impl Trait for Type` måste matcha säkerheten (`fn` respektive
`unsafe fn`) i traitdeklarationen. Implementering sker
explicit med `impl Trait for Type`:

```dunder
pub trait Describe {
    fn describe(&self) -> string
}

pub trait Ordered: Describe {
    fn rank(&self) -> i32
}

struct Item { name: string, weight: i32 }

impl Describe for Item {
    fn describe(&self) -> string { self.name.clone() }
}

impl Ordered for Item {
    fn rank(&self) -> i32 { self.weight }
}

fn label<T: Ordered>(x: T) -> string {
    x.describe() + "#" + x.rank().str()
}
```

Metoder i `impl Trait for Type` ärver synlighet från traiten och skrivs utan
`pub`. Antingen traiten eller måltypen måste deklareras i samma modul
(lokala traits kan implementeras för primitiva, sammansatta och importerade
typer). Kompilatorn tillhandahåller fyra inbyggda strukturella traits:

- `Sendable` härleds automatiskt för vanliga structar, enums, tupler och arrayer
  vars delar är `Sendable` (inklusive `string`, `Result<T, E>` och sammansatta
  arrayer). För en `opaque struct` deklareras den uttryckligen med
  `impl Sendable for Typ {}` (eller villkorligt som
  `impl Sendable for Map<K, V> where K: Sendable, V: Sendable {}`), varpå
  kompilatorn bevisar att alla privata fält uppfyller regeln. En
  `@resource`-typ kan också deklarera `impl Sendable for Typ {}` uttryckligen
  (som `std.fs.FileReader`), varpå kompilatorn bevisar fälten; annars är
  `@resource`, råpekare, lån, closures och oklassificerade `opaque struct`
  inte `Sendable`.
- `Storable` anger att typen får lagras som arrayelement eller fält.
- `Clone` gäller strukturellt för typer utan `@resource`-fält och utan inbyggda
  lån (`Result<T, E>`, `[]T`, tupler, structar och enums vars delar är `Clone`).
- `Copy` gäller för värden utan heap- eller ägarskapsstädning (`bool`, heltal,
  `f32`, `f64`, `rune`, `ptr`, fältlösa enums samt aggregat/tupler vars delar är
  `Copy`, men inte `string` eller `[]T`). Värden av typen `fn(...)` delar sin
  infångade miljö genom referensräkning i stället för att djupkopiera fångade värden.
- `Clone` och `Copy` är strukturella och kan inte implementeras manuellt.

Förladdade kärntraits från preludium finns alltid tillgängliga utan import:
`Ordering` (`Less`, `Equal`, `Greater`), `Eq`, `Ord: Eq`, `Hash: Eq`,
`Display` och `Default`. De är implementerade för primitiva värden (utom att
`f32` och `f64` avsiktligen inte implementerar `Eq`, `Ord` eller `Hash` — bara
`Display`, `Default`, `Clone`, `Copy` och `Sendable`), `Option<T>`, `Result<T, E>` samt
sammansatta typer `[]T`, `(A, B)` och `(A, B, C)` när deras elementtyper
uppfyller motsvarande krav (`impl Default for []T where T: Storable`).
Structar och enum kan dessutom automatiskt härleda implementationer med
`@derive(Eq, Hash, Display, Default)` samt JSON-serialisering med
`@derive(Encode, Decode)` (kräver `std.json`).
Uttryck som `Result<T, ()>` och `Result<(), E>` konstrueras med bara `.Err`
respektive `.Ok` när payloadtypen är `()`.

```dunder
struct Sized<T> where storable(T), T.size: fn(T) -> i32 { item: T }
struct Sample { n: i32 }
impl Sample { pub fn size(&self) -> i32 { self.n } }

fn main() {
    x := Sized<Sample> { item: Sample { n: 42 } }
    print(x.item.size())
}
```

Två importalias till samma deklaration ger samma instans när argumentens
typer är lika. En metod som behöver ytterligare operationer på `T`
kontrolleras för den konkreta mottagaren när metoden används. [Typidentitet och kontrolltidpunkter](generics-design.md)

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
frisätts efter dess sista lexikala läsning i samma block och loopdjup (eller
gäller till blockets slut vid `defer` eller ändring i en inre loop).
Oberoende syskonfält får ändras; ett elementlån låser hela arrayen.
Flytt, `replace` och motstridiga skrivningar av lånad lagring avvisas.

Temporära uttryck får lånas direkt vid anrop, exempelvis `read(&f())` och
`sum(&[1, 2])`, och hålls levande tills anropet är klart. De får inte bindas
som lokala referenser. Lokala skrivlån skrivs `r := &mut value`. En funktion
kan returnera ett lån från en parameter. Ett returnerat skrivlån kräver en
muterbar källparameter. Finns det flera möjliga källor anger `from namn`
vilken parameter returvärdet lånar från.

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
variantens payload (`v` eller `mut v` när värdet får ägas/muteras) eller
kasta bort oanvända värden med jokertecknet `_` (exempelvis `.Err(_)`).
[Enums och match](enums.md#kontrollflöde-med-match) · [Match som uttryck](enums.md#match-som-uttryck)

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
lista i hakparenteser för att fånga lokala värden. Typerna kan härledas. En closure utan
infångade värden har samma funktionstyp `fn(...)` som en namngiven funktion,
men en closure som fångar värden får funktionstypen `Fn(...)` (med varianterna
`&Fn(...)`, `&mut FnMut(...)`, `FnMut(...)` och `FnOnce(...)`) och kan inte
omvandlas till en rå `fn(...)`-pekare.

```dunder
fn plus(base: i32) -> Fn(i32) -> i32 {
    fn[base: i32](x: i32) -> i32 { x + base }
}

fn main() {
    ten := plus(10)
    print(ten(32))  // 42
}
```

Listan utvärderas när closuren skapas. `fn[x]` följer vanliga äganderegler.
Heltal kopieras och en ägd array flyttas in. `fn[mut x]` låter kroppen
ändra sitt eget infångade värde mellan anrop. `move fn[x]` ger en `FnOnce`
som förbrukas när den anropas. Resurser som miljön fortfarande äger städas
när miljön släpps.

`fn[&x]` och `fn[&mut x]` lånar den ursprungliga lagringen. Den senare
kan ändra anroparens värde. En sådan closure får bindas lokalt eller
skickas till en synkron callback. Lånen gäller så länge closuren används.
Kompilatorn avvisar försök att returnera eller lagra dessa closures i en
container. Parametern `&Fn(...)` tillåter läsanrop. `&mut FnMut(...)`
tillåter även callbacks som skriver till sin miljö.
[Körbara exempel](../tools/test_closures.ddr)

### Flervärdesreturer

En funktion kan returnera flera värden som en typad tupel. Returtypen
anger typerna och ordningen. Bind hela resultatet eller plocka isär det
med ett namn eller `_` för varje komponent.

```dunder
fn read_tag(offset: i64) -> (i64, string) {
    (offset + 4, "DATA")
}

fn main() {
    (next, tag) := read_tag(12)
    print(next)  // 16
    print(tag)   // DATA
    pair := read_tag(20)
    print(pair.0)  // 24
}
```

Komponenterna nås med nollbaserade fält som `.0` och `.1`. En tupel
har minst två komponenter, kan nästlas och kan lagras som ett vanligt
värde. I `pair := (42, "tagg")` härleds typen `(i32, string)` från
komponenterna. En returtyp, parametertyp eller lokal typannotering ger
också komponenterna deras typer, exempelvis för en tom array i
`pair: (i32, []string) = (42, [])`.

Varje komponent måste vara lagringsbar. En tupel som äger en sträng,
array eller resurs flyttas när den binds eller plockas isär. Bindningen
`(value, _) := read()` släpper den överhoppade komponentens ägare direkt.
`mut (value, status) := read()` gör båda bindningarna muterbara.

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
och `continue` stöds. Block ger lexikala scope. `print` tar ett skalärvärde
(`i32`, `i64`, flyttal, `bool`, `rune`, `string`, `&str`), ett heltal eller
enum, eller valfri typ som implementerar `Display` (via `.str()`), följt av
radbrytning.
Stränginterpolering `f"{x}"` använder på samma sätt `Display`/`.str()`.
`assert(condition)` kräver `bool` och avslutar processen vid `false`.

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
import std.Map
import std.json.Doc
```

`import sys` väljer det inbäddade systembiblioteket; se [sys](sys.md).
Det kan aliaseras och dedupliceras precis som andra moduler. En egen lokal
fil med samma namn importeras uttryckligt som `import .sys as local`.

Det sista segmentet kan också peka på en exporterad typ eller konstant med stor
begynnelsebokstav i modulen i stället för att binda hela modulen som ett alias:
`import std.Map` (eller `import std.Map as HashMap`), `import std.json.Doc`,
`import std.alloc.Pool` och `import .geometry.Point`. En sådan direktimport
ger samma användning utan modulprefix, inklusive metodanrop som `Map::new()`,
och följer modulens vanliga `pub`-synlighet och kapabilitetskontroll.
Sökvägar med liten begynnelsebokstav fortsätter att importera modulen.

Den första importen läser `math.ddr` bredvid den importerande filen. Den andra
läser `geometry/vector.ddr` relativt samma fil. Sökningen beror inte på
processens arbetskatalog. Punktnotation översätts till kataloger när filen läses.

| Import | Var modulen finns |
| --- | --- |
| `std.map` | Det inbäddade standardbiblioteket (`import std.Map` direktimporterar `Map`). |
| `std.json.Doc` | Direktimporterar den publika typen `Doc` från `std.json`. |
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
En fast array får använda högst 1 048 575 byte. Tomma arrayliteraler kräver explicit typ.

När kontexten ger en arraytyp används dess elementtyp för heltalsliteralerna.
Annars kan det första typade numeriska elementet ge typen även åt tidigare
heltalsliteraler. `[0, u8(255), 1]` får typen `[u8; 3]`.
En array med enbart heltalsliteraler får `i32` om alla ryms där, annars
`i64`. Samma regel gäller `[value; N]` när `value` är en heltalsliteral.
Literaler måste rymmas i den valda typen och typade element måste ha samma typ.

Arrayer flyttas vid bindning och retur. Vanliga parametrar lånar för
läsning. `mut` krävs för att tilldela ett element. Index har typen `i32`; negativa index och index vid
eller efter längden ger runtimefel. `len(values)` och `values.len()` ger
längden som `i32`. Fasta arrayer kan returneras genom ägaröverföring på samma sätt som
andra resursägande värden.
En lokal variabel eller parameter får heta `len`. Den skuggar det
fristående anropet `len(a)` i sitt block. Medlemsåtkomst som `a.len`
avgörs av mottagarens typ, så ett eget structfält kan också heta `len`.
`values.swap(i, j)` byter två element utan kloning och kräver muterbar lagring.
`values.remove(i)` flyttar ut och returnerar elementet vid index `i` i en
muterbar dynamisk array. Efterföljande element förskjuts åt vänster med
bibehållen ordning.

`middle := &values[start..end]` skapar en lånad `&[]T` utan heapallokering
eller retain (observera skillnaden mot `text[start..end]` på en sträng, som
skapar en ny ägd `string`, medan `&text[start..end]` ger en lånad `&str`-vy).
En explicit annotering, `middle: &[]i32 = &values[1..3]`, går
också bra. Vyn använder originalets adress, och dess `len` och `cap()` är
intervallängden. Fasta arrayer och redan lånade vyer kan skäras på samma sätt.
Gränserna utvärderas en gång från vänster till höger; ett intervall utanför
`0 <= start <= end <= values.len` ger status 103. Vyn är oföränderlig och
lånar hela arrayen fram till sin sista läsning; `values[start..end]` utan `&`
avvisas för arrayer — använd `(&values[start..end]).clone()` när en egen
ägd array behövs.

För iteration är `for` lånande som standard men låser bara fältets egen
ägarskapssökväg (så `for x in record.left` tillåter ändring av `record.right`),
och `for x in move values` märker varje elementbindning som ett färskt ägt
värde vid t.ex. `out.push(x)`.

`start..end` har typen `range`, inkluderar start och utesluter slut;
`start..=end` inkluderar båda gränserna. Gränserna är `i32`, utvärderas en
gång och iterationen går uppåt. Om start är större än eller lika med slut
blir det noll varv. Intervall kan lagras, skickas och returneras som vanliga
värden.

`for` stöder intervall, arrayer, `.iter()`-värden, uppslagstabeller och
mängder (`for (k, v) in &map`, `for v in &set`) samt vilken typ som helst
som implementerar den inbyggda traiten `Iterator` (se nedan). Loopvariabeln
är oföränderlig och lokal till loopen. `break` lämnar närmaste loop;
`continue` går till nästa element. Vanlig iteration lånar samlingen för
läsning och avvisar mutation eller flytt av samma samling under loopen.
`for value in move values` konsumerar ägaren. Med `.enumerate()` fås ett
tupelmönster: `for (i, x) in values.enumerate()`.

`for value in values.iter()` lånar också. En lagrad ägande iterator skapas
med `(move values).iter()` eller från en uttrycklig klon, som i exemplet
ovan. Iteratorn kan läslånas flera gånger; bindning till en ny ägare flyttar
den. En egen typ görs itererbar med `impl Iterator for Typ` och metoden
`fn next(&mut self) -> Option<T>`; `for`-loopar accepterar då värdet direkt
(även med `&mut`-lån för att dra flera element i taget).

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
tabellresultat måste stödja likhet (`==`, inklusive typer som implementerar `Eq`
som `[]T`, tupler, `Result<T, E>` och egna structar/enums med `impl Eq`). Kortformen anropar alltid den annoterade funktionen, även
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
och stderr; vid en misslyckad assertion ingår källrad och operandvärden i den
vidarebefordrade stderr-utskriften.

Standardtidsgränsen är fem sekunder per test, ändringsbar med
`--timeout-ms 1000`. Runnern stoppar och väntar in en process som överskrider
gränsen. `-o directory` väljer baskatalog för tillfälliga testbinärer;
varje körning får en egen underkatalog som städas efteråt. Exitstatus är 0
när alla tester passerar, annars 1. Noll hittade tester rapporteras uttryckligt.

`dunder check` typkontrollerar både funktioner och tester i den valda rotfilen
och i importerade projektmoduler. Det omfattar fristående testfunktioner och tabellfall.
Kommandot kör varken programmet eller testerna. En testrot kan sakna `main`.

Utan fil väljer `dunder check`, `build` och `run` manifestets `root`.
`dunder check --tests` och `dunder test` väljer `test_root` när den anges
och annars `root`. För en uttrycklig testfil används `dunder check fil.ddr`.

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
en Unicode-skalär. `rune` är en egen typ som stödjer jämförelse (`==`, `!=`,
`<`, `<=`, `>`, `>=`), `print`, `.str()` och explicit heltalskonvertering med
`i32(r)`/`i64(r)` (dock inte aritmetik som `+`/`-`/`*`/`/`/`%`).
`rune(number)` kontrollerar att talet är 0–0x10FFFF och inte en surrogate.
Ingen implicit konvertering mellan rune, heltal och string sker.

Escape-sekvenser: `\\`, `\"`, `\'`, `` \` ``, `\n`, `\r`, `\t`, `\0`,
`\xHH`, `\uHHHH` och `\u{H...}` med en till sex hexsiffror.
Byte-escapes valideras tillsammans: `"\xC3\xA5"` är `"å"`, men `"\xFF"`
är ett kompileringsfel. Surrogates och värden över Unicode-intervallet
avvisas. NUL är tillåten i strängdata; längden avgör slutet, ingen nollterminering används.

Råa strängar skrivs `r"..."`, `r'...'`, `r#"..."#` (med godtyckligt antal `#`)
samt flerradiga `"""..."""` och `r#"""..."""#` (och `f"""..."""` med
interpolering). Flerradiga literaler använder automatisk dedent efter
avslutningsradens indentering; oescapade radbrytningar tillåts i flerradiga och
råa literaler. Dessutom finns kompileringstidsinbäddning med
`@embed_file("relativ/sökväg")` (`string`, UTF-8-kontrollerad) och
`@embed_bytes("relativ/sökväg")` (`[]u8` / `[u8; N]`), strikt begränsad till
filer innanför paketets rot och spårad av den inkrementella byggcachen.

`f"Hej {name}, {count + 1}!"` och `f'Hej {name}!'` interpolerar uttryck
från vänster till höger (liksom på `&str`-vyer, med bredd/justering som
`f"{v:>8}"`). Hål utan formatkod använder `.str()`. Uttrycken
lånar sina operander; att formatera en sträng flyttar inte dess ägare.
`{{` och `}}` ger bokstavliga klamrar. Vanliga strängliteraler interpolerar
inte. Fel i uttrycken pekar på deras ursprungliga källpositioner.

| Operation | Betydelse |
| --- | --- |
| `text.len`, `text.len()`, `len(text)` | Antal byte, `i32`, konstant tid (fungerar på både `string` och `&str`). |
| `text[i]`, `view[i]` | Ett byte som `u8` (i linje med `b'a'`-literaler). |
| `text[start..end]` | Ny ägd `string` från halvöppna bytegränser (till skillnad från arrayer som kräver `&arr[start..end]` för en lånad `&[]T` eller `(&arr[start..end]).clone()` för en kopia). Båda gränserna måste vara UTF-8-gränser. |
| `&text[start..end]`, `text.as_str()` | Lånad UTF-8-strängvy av typen `&str`, utan ny sträng eller retain (`view.as_bytes()` ger `&[]u8`). |
| `text.runes()[i]` | En rune efter avkodning från början; linjär tid. |
| `text.runes().len` | Antal kodpunkter; linjär tid. `.len()` och `len(...)` fungerar också. |
| `for r in text.runes()` | Avkodar kodpunkter i ett linjärt pass (på både `string` och `&str`). |
| `for byte in text.bytes()` | Itererar `u8`-byte i ett linjärt pass (på både `string` och `&str`). |
| `a + b` | Bygger en ny sträng av två ägda `string` (ej på `&str`; använd `.clone()` vid behov). |
| `a == b`, `a != b` | Jämför innehåll (även mellan `string` och `&str`), inklusive eventuella NUL. |
| `<`, `<=`, `>`, `>=` | Lexikografisk ordning efter UTF-8-byte (fungerar mellan `string` och `&str`), utan språklig sortering. |
| `value.str()` | Sträng av heltal, flyttal, bool eller rune; på `&str` och `.to_string()` skapas en ägd `string`. |

`.bytes()` och `.runes()` ger självständiga arrayer av typen `[]u8`
respektive `[]rune`. Direkt iteration kan strömma från en behållen
strängsnapshot eller en `&str`-vy utan mellanarray. Om originalbindningen får en ny sträng
behåller konverteringen eller iterationen sina gamla värden. `for` direkt över en string eller `&str` avvisas:
välj `.bytes()` eller `.runes()` uttryckligt. `break` och `continue` fungerar
för båda. Lagrade arrayiteratorer äger sin samling; skapa dem med
`(move values).iter()` eller `values.clone().iter()`.
Intervall i index skrivs direkt i hakparentesen (`text[start..end]`); ett
lagrat eller returnerat `range`-värde accepteras inte som index.
En namngiven strängvy, exempelvis `utf8 := &text[1..7]` för `"xå🌍y"`,
pekar direkt in i originalet med typen `&str` och stöder nollallokerande
hjälpare i [`std.text.view`](standard.md#moduler). Båda gränserna måste vara
UTF-8-gränser även när vyn är tom. Intervallfel ger 103 före
UTF-8-kontrollen som ger 106. `.clone()` på `&str` ger en ägd `string`,
medan `utf8.as_bytes()` ger motsvarande `&[]u8`-bytevy.

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
`capability unsafe` kan använda `unsafe fn` (även i `trait` och `impl Trait for Type`),
`unsafe { ... }`, `bytes_view(&owner, base, length)`, `@extern` och typade
C-funktionspekare. `capability filesystem`, `network`, `process` och `stdio`
styr tillgången till delegerbara OS-handtag. `@resource(dispose)` stöder
städmetoden `fn dispose(&self)` eller `fn dispose(&mut self)`. En
`pub opaque struct` exponerar typen och låter den definierande modulen äga
konstruktion och fältåtkomst. [Språkkontrakt och exempel](ffi.md)
