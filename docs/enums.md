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
    print(i32(kind))  // 1
}
```

Utelämnade värden börjar på noll och ökar från föregående värde. Explicita
värden är signerade `i32`-heltalsliteraler, inklusive negativa värden.
Tomma enums, dubbla namn, dubbla tal och värden utanför `i32` avvisas.
Ingen implicit konvertering till heltal görs. `i32(kind)` och `i64(kind)`
läser talvärdet; `FileKind(number)` tar en `i32` och kontrollerar att värdet
finns i deklarationen. Ett okänt värde avslutar processen med status 108.

En enum utan data lagras som fyra byte och stöder `==` och `!=` mot samma
typ. Den kan också vara structfält eller element i en fast array.
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

`match` kräver en enum och kontrollerar alla varianter vid kompilering.
Saknade eller dubbla fall, fel enumtyp och fel antal payload-bindningar är
typfel. `_` täcker återstående varianter och måste stå sist:

```dunder
import sys

fn read_or_empty(path: string) -> string {
    match sys.read(path) {
        .Ok(text) => text,
        .Err(error) => {
            match error {
                .NotFound => "",
                _ => { print(sys.io_error_message(error)); return "" },
            }
        },
    }
}
```

Payload-bindningar är oföränderliga lokala värden som bara finns i armen.
Använd `_` för ett oanvänt payloadvärde, exempelvis `.Pair(x, _)`.
Uttrycket efter `match` utvärderas en gång; aggregat och bindningar kopieras
med värdesemantik. Att ändra originalets `var` i en arm ändrar inte kopian.
Nästa fall körs inte automatiskt efter en arm. `break` och `continue`
syftar fortsatt på en omgivande loop.

En arm får innehålla ett uttryck eller ett block. Komma eller radbrytning
skiljer armarna åt. En avslutande `match` kan ge funktionens returvärde
genom armarnas sista uttryck, som i exemplen. Explicit `return` fungerar
också. `match` är i denna version en sats; `x := match ...` återstår.
Nästlade fall skrivs med en inre `match`, inte ett nästlat mönster.

## Lagring och dispatch

En enum med data innehåller en tagg och plats för den största variantens
payload direkt i värdet. Själva taggningen behöver inget extra heapobjekt
eller vtables. Payloads följer sina typers kopieringsregler; en dynamisk
array kopieras till en egen buffert. Oföränderliga strängar använder den befintliga
stränglagringen; en sträng i en variant kopierar stränghandtaget.
Funktionsvärden i payloads använder vanliga typade funktionspekare.

`match` kompileras till taggjämförelser och direkta grenar. Structar, arrayer
och data-enums kopieras vid bindning, tilldelning, parameteröverföring och
retur. Anroparen äger returvärdets lagring, även vid callbacks och rekursion.
Detta är en första implementation utan optimerings- eller prestandalöften;
minneslayouten är ännu inget stabilt externt ABI.

Modellen ligger nära kombinationen av
[Zigs tagged unions](https://ziglang.org/documentation/master/#Tagged-union)
och [Odins explicita enumvärden och kontextuella kortformer](https://odin-lang.org/docs/overview/#enumerations).
Arenaallokering och val av dispatch är separata frågor: slutna varianter
med statiskt kontrollerad `match` behöver ingen dynamisk metodtabell.

## Verifiering och nuvarande gränser

```sh
dunder run dogfood/enums.ddr
# 42
# enums ok
dunder test dogfood/enums.ddr
# 5 passed, 0 failed
python3 tools/test-diagnostics.py "$(command -v dunder)"
```

[Dogfood-programmet](../dogfood/enums.ddr) provar båda formerna och nästlade
`Ok`/`Err`-resultat. Native-integrationstester täcker typfel, modulidentitet,
privata typer, egna taggvärden, arrayer/structfält, blandade payloads,
funktionsvärden, callbacks, rekursiva returer, stackargument och verklig fil-I/O.

Data-enums kan vara structfält och innehålla andra aggregat, inklusive
dynamiska arrayer och andra data-enums. Arrayelement är fortfarande begränsade
till skalärer och enums utan payload. Ett enumvärde får vara högst
64 KiB. För djup rekursiv layoutberäkning avvisas också. Likhet för data-enums,
generiska `Result<T>`/`Option<T>`, `?`, mönsterguards, alternativa/nästlade
mönster och enumutskrift med `.str()` återstår. Använd `match` för att välja
payload eller en textrepresentation. [sys](sys.md) använder redan konkreta
`ReadResult` och `IoError` utan generics.
