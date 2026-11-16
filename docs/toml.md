# TOML-konfiguration

`std.toml` läser konfiguration i Dunder och returnerar JSON för samma typade
läsare som övriga dataformat. Ingen extern parser eller process körs.

```dunder
import std.toml as toml
import std.json as json

fn main() {
    result := toml.parse("version = 1\n[worker]\nconcurrency = 4\n")
    if !result.ok { print(result.error); return }
    doc := json.Doc::parse_text(result.json)
    worker := doc.field(doc.root, "worker")
    count := doc.as_u64(doc.field(worker, "concurrency"))
    assert(count.ok && count.value == 4)
}
```

## API och fel

```dunder
pub struct ParseResult { ok: bool, error: string, json: string }
pub fn parse(input: string) -> ParseResult
```

Ett lyckat resultat har `ok = true`, tom `error` och ett JSON-objekt i
`json`. Ett fel har `ok = false`, en diagnostik och tom `json`; ett delvis
tolkat dokument publiceras aldrig. Syntax- och tabellfel anger rad och
bytekolumn, båda räknade från ett. Storleksfel kan avvisas innan någon
källposition har lästs.

Anroparen läser filer och validerar sitt eget schema: tillåtna fältnamn,
timeoutgränser, sökvägar, obligatoriska fält och liknande. En giltig
TOML-konfiguration behöver alltså inte vara giltig för en viss applikation.

## Syntax och tabellidentitet

Grammatiken utgår från [TOML 1.0](https://toml.io/en/v1.0.0). Parsern stöder
nakna och citerade nycklar, punktnotation, tabeller, arrays av tabeller,
inline-tabeller, nästlade arrayer, kommentarer och de fyra strängformerna.
Unicode-escapes avkodas innan nycklar jämförs. Tomma citerade nycklar och
nycklar som innehåller punkt, kolon eller escaped NUL behåller sin identitet.
LF och CRLF stöds; CRLF normaliseras till LF i flerradiga strängar.

Dubbletter och typkonflikter avvisas. Implicita överordnade tabeller kan
deklareras senare, medan tabeller som skapats genom punktnotation inte kan
omdeklareras med en tabellrubrik. Inline-tabeller stängs rekursivt efter
värdet. En efterföljande punktnyckel eller tabellrubrik får inte utöka dem.
Nästlade arrays av tabeller använder det senaste elementet på varje nivå.
Elementordning och fältens insättningsordning bevaras.

Projektets `dunder.toml` använder fortfarande den separat dokumenterade,
avgränsade [manifestgrammatiken](dependencies.md). `std.toml` ersätter inte
manifestets schema eller ändrar beroendeupplösningen.

## JSON-projektion och exakta tal

Strängar, booleans, arrayer och tabeller blir motsvarande JSON-värden.
Heltal går aldrig genom flyttal. Decimalers plustecken och understreck tas
bort; binära, oktala och hexadecimala heltal skrivs som exakta decimaltal.
Heltalen `0`, `+0` och `-0` ger alla JSON-heltalet `0`; flyttalet `-0.0`
behåller sitt tecken och sin flyttalstyp.
Negativa heltal stöder hela `i64`-intervallet. Som uttrycklig utökning stöds
positiva heltal till och med `18446744073709551615`, hela `u64`-intervallet.
Överskridanden ger fel.

Flyttal behåller en giltig JSON-stavning med decimalpunkt eller exponent.
`1.0` och `1e0` blir alltså inte heltalet `1`. Inga binära flyttalskonverteringar
eller avrundningar görs; stora exponenter förblir text i JSON-talet.
Använd `json.Doc.raw_span` när den exakta talstavningen behövs.
`as_i64` och `as_u64` avvisar flyttalsstavningar även när talvärdet är helt.

JSON saknar vissa TOML-typer. Dessa får märkta objekt i stället för att
förväxlas med vanliga strängar, null eller heltal:

| TOML-värde | JSON-projektion |
| --- | --- |
| `inf`, `+inf` | `{"$toml_float":"inf"}` |
| `-inf` | `{"$toml_float":"-inf"}` |
| `nan`, `+nan`, `-nan` | `{"$toml_float":"nan"}` |
| Lokalt datum | `{"$toml_date":"1979-05-27"}` |
| Lokal tid | `{"$toml_time":"07:32:00.123456789"}` |
| Lokal datumtid | `{"$toml_datetime":"1979-05-27T07:32:00"}` |
| Datumtid med offset | `{"$toml_datetime":"1979-05-27T07:32:00Z"}` |

Datum och tider valideras, inklusive månadslängd, skottår och offset.
Datumtidens mellanslag eller `t` normaliseras till `T`, och `z` till `Z`.
Alla bråkdelssiffror behålls. År `0000` behålls lexikalt; skottsekund `:60`
avvisas uttryckligen. Märkningarna är en projektion för konfigurationsläsning,
inte en återställbar serialisering av TOML-källtexten. Ett vanligt TOML-objekt
kan ha samma fältnamn; en framtida generell writer behöver ett rikare
typkontrakt för att återskapa sådana skillnader.

## Gränser och implementation

- Högst 4 MiB indata.
- Högst 100 000 noder inklusive rot och implicita tabeller.
- Högst 64 nivåer under roten; både en array av tabeller och dess element
  räknas som nivåer.
- Högst 64 MiB genererad JSON, kontrollerat före utdataallokeringen.

TOML 1.1-utökningar som `\e`, `\xNN` och tider utan sekunder ingår inte.
Strängparametern följer Dunders UTF-8-kontrakt; `sys.read_text` avvisar en
fil med ogiltig UTF-8 före parseranropet.

`toml.ddr` samordnar källposition och statementgrammatik. `toml/values.ddr`
läser strängar och skalärer. Den opaque tabellarenan i `toml/tree.ddr` äger
nyckelidentitet, tabellernas definitionstillstånd, syskonlänkar och JSON-
skrivning. Fält indexeras per förälder och avkodad nyckel; rendering följer
syskonlänkar och behöver inte söka igenom hela arenan för varje nod.

`python3 tools/test-toml.py KOMPILATOR` kontrollerar publika resultat mot
oberoende dataorakel. `--runner TARGET=COMMAND` kan lägga till lokala
Linuxkörningar, och `--toml-test DIR` använder ett separat uppackat
`toml-test`-korpus. Ingen Pythonkod används av biblioteket eller programmen
som byggs med det.
