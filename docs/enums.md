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

En enum utan data lagras som fyra byte och stöder `==` och `!=` mot samma
typ. Den kan också vara structfält eller element i en fast eller dynamisk array.
Heltalsaritmetik och ordningsjämförelser kräver en explicit konvertering.

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
Importerade konstruktorer skrivs exempelvis `sys.ReadResult.Ok("hej")`.

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

fn beskriv(resultat: sys.ReadResult) -> string {
    match resultat {
        .Ok(text) if text.len > 0 => text.clone(),
        .Ok(_) => "tom fil",
        .Err(.NotFound) => "filen saknas",
        .Err(.Other(kod)) => f"systemfel {kod}",
        .Err(fel) => sys.io_error_message(fel),
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
och måste stå sist.

Kompilatorn kräver att armarna utan villkor tillsammans täcker alla möjliga
kombinationer av varianter. Detta gäller även när ett villkor är `if true`.
Om en variant har två enumpayloads måste
även kombinationerna av deras värden täckas. Armar får överlappa när den
senare armen fortfarande kan träffa ett nytt fall. Flyttas `.Err(fel)`
före `.Err(.NotFound)` blir den specifika armen onåbar och bygget avvisas.
För mycket stora mönster kan kompilatorn ge `match pattern coverage limit exceeded`.
Dela då upp matchningen i flera `match` så att varje kontroll blir mindre.

En payload kan bindas till ett namn eller hoppas över med `_` oavsett
typ. För att fortsätta mönstret med en variant måste payloaden själv ha
enumtyp. Kompilatorn kontrollerar variantnamn och antal payloadmönster på
varje nivå. Vanlig matchning lånar resursägande payloads. `match move value` och
matchning av ett färskt ägt uttryck överför payloadens ägarskap till den
valda armen efter att dess villkor lyckats. Komma
skiljer armarna åt.

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

## Lagring och dispatch

En enum med data innehåller en tagg och plats för den största variantens
payload direkt i värdet. Själva taggningen behöver inget extra heapobjekt
eller vtables. Payloads följer sina typers ägarskapsregler. Flytt överför befintlig lagring.
En uttrycklig `.clone()` klonar ägda arraybarn till oberoende buffertar;
oföränderlig strängtext kan delas med referensräkning.
Funktionsvärden i payloads kan vara namngivna funktioner eller closures.

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
# I syskonrepot dunder-hundmat:
dunder run dogfood/enums.ddr
# 42
# enums ok
dunder test dogfood/enums.ddr
# 5 passed, 0 failed
dunder run tools/test_diagnostics.ddr -- "$(command -v dunder)"
```

Programmet `dogfood/enums.ddr` i `dunder-hundmat` provar båda formerna och nästlade
`Ok`/`Err`-resultat. Native-integrationstester täcker typfel, modulidentitet,
privata typer, egna taggvärden, arrayer/structfält, blandade payloads,
funktionsvärden, callbacks, rekursiva returer, stackargument och verklig fil-I/O.

Fasta och dynamiska arrayer kan innehålla enums med payload. Ett element
kan i sin tur äga en array, en struct eller en annan enum. Ägarskap och
kloning följer payloadens typ. [Körbart exempel](../tests/fixtures/aggregate_arrays/enums.ddr)

Ett enumvärde får vara högst 64 KiB. För djup rekursiv layoutberäkning avvisas också. Likhet för data-enums,
generiska `Result<T>`/`Option<T>`, `?`, alternativa
mönster och enumutskrift med `.str()` återstår. Använd `match` för att välja
payload eller en textrepresentation. [sys](sys.md) använder redan konkreta
`ReadResult` och `IoError` utan generics.
