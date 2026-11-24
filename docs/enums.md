# Enums och match

Dunder har både namngivna heltalsvärden och varianter som bär data.
De är slutna, nominella typer: två deklarationer är olika typer även om
varianterna råkar heta samma sak. `enum` är privat; `pub enum` exporterar
typen och dess varianter. Importer har fortsatt inget suffix.

## Namngivna värden

```dunder
enum FileKind {
    Other = 0,
    File = 1,
    Directory = 2,
}

fn main() {
    kind := FileKind.File
    expected: FileKind = .File
    assert(kind == expected)
    assert(i32(kind) == 1)
}
```

Utelämnade värden börjar på noll och ökar från föregående värde. Explicita
värden är signerade `i32`-heltalsliteraler, inklusive negativa värden.
Tomma enums, dubbla namn, dubbla tal och värden utanför `i32` avvisas.
Ingen implicit konvertering till heltal görs. `i32(kind)` och `i64(kind)`
läser talvärdet; `FileKind(number)` tar en `i32` och kontrollerar att värdet
finns i deklarationen. Ett okänt värde avslutar processen med status 108.

En enum utan data lagras som fyra byte och skickas direkt som värde i register
över funktionsanrop, funktionspekare och `task`-gränser. Den stöder `==` och
`!=` mot samma typ och kan vara structfält eller element i en fast eller
dynamisk array. Heltalsaritmetik och ordningsjämförelser kräver en explicit
konvertering.

## Varianter med data

```dunder
struct Point { x: f64, y: f64 }
enum Command {
    Stop,
    Print(string),
    Move(Point),
    Pair(i32, i32),
}

@test {
    (.Pair(20, 22)) -> 42
    (.Print("Dunder")) -> 6
    (.Stop) -> 0
}
fn evaluate(command: Command) -> i32 {
    match command {
        .Stop => 0,
        .Print(text) => text.len,
        .Move(point) => i32(point.x + point.y),
        .Pair(x, y) => x + y,
    }
}

fn main() {
    command := Command.Pair(20, 22)
    print(evaluate(command))
}
```

Konstruktorer skrivs `Command.Pair(20, 22)`. Kortformen `.Pair(20, 22)`
fungerar när typen framgår av en annotering, parameter, returtyp eller
jämförelse. `value := .Pair(20, 22)` saknar däremot typkontext och avvisas.
Varianter utan data kan skrivas med eller utan tomma parenteser.
Importerade konstruktorer skrivs exempelvis `sys.IoError.Other(2)`.

Payloads kan innehålla skalära typer, funktionsvärden, structar, fasta
arrayer, dynamiska arrayer och andra enums. Direkt rekursiva värden som `enum Node { Next(Node) }`
har ingen ändlig storlek och avvisas. Ett träd kan i dagens arena-dogfood
i stället använda typade värden med indexhandtag till en separat buffert.
Enums ger ingen ny allokerare eller ägar-/lånemodell.

## Kontrollflöde med match

`match` utvärderar sitt argument en gång och prövar armarna uppifrån
och ned. Ett mönster kan följas av `if` och ett villkor av typen `bool`.
Villkoret prövas en gång när hela mönstret passar och kan använda armens
payload-bindningar. Vid `true` körs armen. Vid `false` fortsätter matchningen
med nästa arm.
En variant som innehåller en annan enum kan matchas på båda nivåerna i
samma mönster.

```dunder
import sys

fn beskriv(resultat: Result<string, sys.IoError>) -> string {
    match resultat {
        .Ok(text) if text.len > 0 => text.clone(),
        .Ok(_) => "tom fil",
        .Err(.NotFound) => "filen saknas",
        .Err(.Other(kod)) => f"systemfel {kod}",
        .Err(fel) => fel.message(),
    }
}

fn main() {
    assert(beskriv(.Ok("hej")) == "hej")
    assert(beskriv(.Ok("")) == "tom fil")
    assert(beskriv(.Err(.NotFound)) == "filen saknas")
    assert(beskriv(.Err(.Other(123))) == "systemfel 123")
}
```

I `.Err(.Other(kod))` bestämmer payloadtypen `sys.IoError` vad
`.Other` betyder. Namnet `kod` binder ett oföränderligt `i32` i armen.
`.Err(fel)` binder hela felvärdet för de fel som återstår. `.Err(_)`
täcker samma fall och kastar bort payloaden. Även nästlade mönster
som `.Err(.Other(kod)) if kod > 0`, varianter utan payload och `_`
kan ha villkor. Ett ensamt `_` utan villkor täcker alla återstående fall
och måste stå sist. Flera mönster kan också slås samman med `|` i samma arm,
exempelvis `.Stop | .Reset => 0`; i payloadlistor täcker ett `..` alla
återstående payloads (högst ett `..` per mönster).

Kompilatorn kräver att armarna utan villkor tillsammans täcker alla möjliga
kombinationer av varianter. Detta gäller även när ett villkor är `if true`.
Om en variant har två enumpayloads måste
även kombinationerna av deras värden täckas. Armar får överlappa när den
senare armen fortfarande kan träffa ett nytt fall. Flyttas `.Err(fel)`
före `.Err(.NotFound)` blir den specifika armen onåbar och bygget avvisas.
För mycket stora mönster kan kompilatorn ge `match pattern coverage limit exceeded`.
Dela då upp matchningen i flera `match` så att varje kontroll blir mindre.

En payload kan bindas till ett namn (`v` eller `mut v`) eller hoppas över med `_`
oavsett typ. För att fortsätta mönstret med en variant måste payloaden själv ha
enumtyp. Kompilatorn kontrollerar variantnamn och antal payloadmönster på
varje nivå.

Vid `match` på en lokal ägd variabel eller ett ägt structfält (`match värde`,
`match s.fält`) lånar mönsterbindningar som standard när armen bara läser dem.
Om en arm flyttar ut en ägd payload (t.ex. `failure = e`, `ägd := v` eller
`lista.push(move v)`) överförs ägarskapet ur det matchade värdet längs just den
armen på motsvarande sätt som en fältflytt; icke-flyttande armar lämnar värdet
levande. `match move value` och matchning av ett färskt ägt uttryck flyttar
däremot in ägarskapet direkt i armarna efter att ett eventuellt villkor har
lyckats och tillåter också `mut`-mönsterbindningar som `.Ok(mut x)` direkt.
Se gränserna mellan fältflytt, mönsterbindning och lånade källor i
[Minne och ägarskap](memory.md#fält-element-och-begränsningar). Komma skiljer armarna åt.

Som sats kan `match` ha block med flera satser.
En avslutande `match` kan ge funktionens returvärde genom armarnas sista
uttryck. Explicit `return` fungerar också. `break` och `continue` syftar på
en omgivande loop.

### Match som uttryck

```dunder
enum Leverans { Hamta, Skicka(i32) }

fn totalpris(pris: i32, leverans: Leverans) -> i32 {
    frakt := match leverans {
        .Hamta => 0,
        .Skicka(avgift) => {
            emballage := 5
            avgift + emballage
        },
    }
    pris + frakt
}

fn main() {
    assert(totalpris(100, .Skicka(29)) == 134)
}
```

Efter `=>` kan du skriva ett uttryck eller ett block med lokala bindningar
och satser. Blockets sista uttryck utan semikolon ger värdet. De armar som
når slutet av matchningen måste ge samma typ. En arm som lämnar funktionen
med `return` eller hoppar med `break` eller `continue` behöver inget slutvärde.
Typen kan komma från en annotering eller en funktionsparameter. När en enum
förväntas fungerar kortformen `.Variant` även sist i ett block. Med `:=`
härleder kompilatorn typen från den första armen som ger ett värde.

## Typade fel med Result

`Result<T, E>` är en generisk enum i språkets fördefinierade bibliotek med varianterna `.Ok(T)` och `.Err(E)`.
Använd `.Ok` eller `.Err` utan parenteser när den variantens typ är `()`.
Skriv `Result<i32, string>.Ok(42)` för att välja typen utan förväntad
kontext. Varje par av konkreta typargument får en egen statisk typ.

```dunder
fn parse_port(text: string) -> Result<i32, string> {
    if text == "443" { return .Ok(443) }
    .Err("ogiltig port")
}

fn label(text: string) -> Result<string, string> {
    port := parse_port(text)?
    .Ok("port " + port.str())
}
```

Postfixoperatorn `?` utvärderar sitt Result-uttryck en gång. Vid `.Ok`
ger den variantens värde. Vid `.Err` returnerar den felet från den
omgivande funktionen genom dess vanliga returstädning. Funktionens
feltyp måste vara densamma som operandens. Funktionens framgångstyp
kan väljas oberoende.

`Result` följer samma ägarskapsregler som andra enums. När ett Result
innehåller en resurs flyttar `?` ut den valda varianten. Vid vanlig `match res`
på en ägd lokal eller ett ägt fält kan en arm också flytta ut `.Ok(v)` eller
`.Err(e)` direkt utan `match move` (och lämnar `res` förbrukat efter den armen),
medan `.Ok` eller `.Err` utan parenteser används när payloadtypen är `()`.
Metoder på värdet anropas med parenteser: `.is_ok()`, `.is_err()`,
`.unwrap_or(...)`, `.unwrap_or_else(...)`, `.ok()`, `.err()` och
`.unwrap_or_default()` (den senare kräver `T: Default`). Samma metoder finns
på preludiets `Option<T>` (`Some`/`None`): `.is_some()`, `.is_none()`,
`.unwrap_or(...)`, `.unwrap_or_else(...)` och `.unwrap_or_default()`.

`map`, `and_then`, `map_err`, `ok_or_else` och `unwrap_or_else` lånar
callbacken som `&mut FnMut`. De accepterar namngivna funktioner och
closures som fångar egna eller lånade värden. Ett valt anrop sker en gång,
innan metoden returnerar. Övriga varianter hoppar över callbacken. En
callback som får variantens innehåll tar över det med en `move`-parameter.
[Körbart exempel med automatisk resursstängning](../tests/fixtures/results.ddr)

## Lagring och dispatch

En enum med data innehåller en tagg och plats för den största variantens
payload direkt i värdet. Alla fältlösa varianter (oavsett taggnummer, inklusive
`.None`, `.Err`, `?`-propagering och `.clone()` av tomma varianter) återanvänder
statiska skrivskyddade enkelinstanser utan heapallokering. Själva taggningen
behöver inget extra heapobjekt eller vtables. Payloads följer sina typers
ägarskapsregler. Flytt överför befintlig lagring. En uttrycklig `.clone()`
klonar ägda arraybarn till oberoende buffertar; oföränderlig strängtext kan
delas med referensräkning. Funktionsvärden i payloads kan vara namngivna
funktioner eller closures.

`match` kompileras till taggjämförelser och direkta grenar. Resursägande structar, arrayer och data-enums flyttas vid bindning,
tilldelning och retur. Vanliga parametrar lånar dem för läsning. Anroparen äger returvärdets lagring, även vid callbacks och rekursion.
Detta är en första implementation utan optimerings- eller prestandalöften;
minneslayouten är ännu inget stabilt externt ABI.

Modellen ligger nära kombinationen av
[Zigs tagged unions](https://ziglang.org/documentation/master/#Tagged-union)
och [Odins explicita enumvärden och kontextuella kortformer](https://odin-lang.org/docs/overview/#enumerations).
Arenaallokering och val av dispatch är separata frågor: slutna varianter
med statiskt kontrollerad `match` behöver ingen dynamisk metodtabell.

## Verifiering och nuvarande gränser

```sh
dunder run tools/test_results.ddr -- "$(command -v dunder)"
dunder run tools/test_aggregate_arrays.ddr -- "$(command -v dunder)"
dunder run tools/test_diagnostics.ddr -- "$(command -v dunder)"
```

`tools/test_results.ddr` provar båda enumformerna, nästlade `Ok`/`Err`-resultat,
`?`-operatorn och feldiagnostik. Native-integrationstester täcker typfel,
modulidentitet, privata typer, egna taggvärden, arrayer/structfält, blandade
payloads, funktionsvärden, callbacks, rekursiva returer, stackargument och
verklig fil-I/O.

Fasta och dynamiska arrayer kan innehålla enums med payload. Ett element
kan i sin tur äga en array, en struct eller en annan enum. Ägarskap och
kloning följer payloadens typ. [Körbart exempel](../tests/fixtures/aggregate_arrays/enums.ddr)

Ett enumvärde får vara högst 64 KiB. För djup rekursiv layoutberäkning avvisas också.
Alternativa mönster skrivs `.A | .B`. Likhet, hash, utskrift och standardvärde
för structar och data-enums härleds med `@derive(Eq)`, `@derive(Hash)`,
`@derive(Display)` och `@derive(Default)` (och med `std.json` även
`@derive(Encode, Decode)`); utan härledning används `match` för att välja
payload eller en textrepresentation. [sys](sys.md) använder konkreta
`Result<...>` och `IoError`. `Option<T>` (`Some`/`None`) finns i preludiet.
