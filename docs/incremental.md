# Snabba återbyggen och agentutdata

```sh
dunder check examples/functions.ddr --agent
dunder build examples/functions.ddr --incremental --timings
dunder check examples/functions.ddr --agent --timings=json
dunder test --affected
```

## Läs kontrollresultat som JSON

`check --agent` skriver ett JSON-objekt per rad på stdout. En lyckad
kontroll rapporterar namn och typ för parametrar och lokala bindningar,
sedan en post med `kind: "ok"` och antal moduler. Ett källfel ger en post
med `kind: "error"`, felkod, sökväg, meddelande och spann samt exitkod 1.
`start` och `end` är nollbaserade UTF-8-byteoffset i den angivna filen.
Slutpositionen är exklusiv. `--target wasm32` väljer Wasms pekarbredd även
vid typkontrollen.

## Återanvänd maskinkod

`--incremental` aktiverar funktionscachen för nativebyggen och `run`.
`--timings` rapporterar hur många nåbara funktioner som återanvändes och hur
många som emitterades. Cachen finns i `build/.dunder-cache/selfhost` under
aktuell arbetskatalog. Man kan radera hela katalogen mellan två byggen.

Nyckeln innehåller kompilatorns kryptografiska identitet, målet, deklarationernas
layout, skaläroptimeringens läge och funktionens tokenström. CLI:t behåller
separata cachekataloger för `--scalar-opt=on` och `--scalar-opt=off`.
På macOS används SHA-256 av den laddade
bildens kodsignatur och på Linux SHA-256 av den körande ELF-filen genom
`/proc/self/exe`. Identiteten följer därmed processen även om ett nytt bygge
ersätter dess sökväg. Symboliska hopp placeras om vid varje
bygge. Funktioner som skapar nya typer, strängliteraler, importplatser eller
statisk lagring emitteras på nytt. Program med C-callbacks emitteras på nytt.
Det håller återanvändningen konservativ tills dessa resurser kan spelas upp
från cacheformatet. `--timings` visar återanvända och emitterade funktioner.

## Mät byggkedjans faser

`--timings` skriver också monotona väggtider på stderr. `--timings=json`
skriver en JSON-post på stderr med `version: 1`, `unit: "ns"`, målet,
modul- och funktionsantal samt följande faser:

- `module_load_lex_resolve`: modulladdning, lexning och namnupplösning.
- `build_setup`: förberedelser, inklusive källskydd och cacheidentitet.
- `compiler`: det samlade kompilatoranropet med semantisk kontroll,
  kodgenerering, målsänkning och symboliska korrigeringar.
- `image_format_sign`: bildformat och eventuell binärsignering.
- `publication`: skrivning och atomisk publicering; för appar även
  resursförsegling och appsignering.

`total` är summan av faserna. CLI-parsning, själva tidsrapporten och körning
av det byggda programmet eller testerna ingår inte. `check` markerar bild
och publicering som överhoppade. `functions.emitted` är då noll: kontrollen
behåller inga maskinord eller relokeringar. Typ-, låne- och layoutkontroller
körs fortfarande, även för oanropade funktioner.

Fältet `optimizer` visar `enabled`, `modeled`, `emitted`, `rewrites`,
`proven_bounds` och `fallbacks` för det lokala skalärpasset.
`modeled` omfattar också validerade oanropade kroppar; `emitted` räknar
bara modellens kodgenerering. `fallbacks` räknar försök som parsern
avvisar, medan signaturer utanför delmängden hoppas över före försöket.
Cacheträffar modelleras inte på nytt. `proven_bounds` räknar
indexläsningar och elementskrivningar med bevisad gränskontroll.

Klockan väljs efter kompilatorns värdplattform, även vid korsbyggen. Ett
klockfel ger `clock_ok: false` och otillgängliga intervall som `null`.
Lyckade byggen och kontroller ger rapporter; misslyckade eller enbart
listande kommandon ger ingen avslutad tidsrapport. Vanlig stdout och
`check --agent` behåller sina format. Utan flaggan tas inga tidsstämplar.

### Fördela kompilatorns eget arbete

```sh
dunder build selfhost/main.ddr --profile-compiler --timings=json -o build/measured
```

`--profile-compiler` aktiverar tidsrapporten och lägger till `compiler_detail`.
Fältet delar kompilatorintervallet i elva icke överlappande faser:
`declarations`, `model_parse`, `verify_pre`, `simplify`, `verify_post`,
`allocation`, `scalar_emit`, `legacy_body`, `fragment_cache`, `finalize`
och `range_analysis`. Den sista omfattar analysens preflight, fixpunkt,
replay och applicering; arbetsenheten är abstrakta operationer inklusive
kopiering och sammanfogning av tillstånd.
`covered` summerar dessa intervall; `other` är återstoden av `compiler`.
`calls` och `work` anger antal besök och arbetsenhet för varje fas. Samtliga
klockor använder värdplattformen även när målet är ett annat.

Klockfel ger `null` för tidsfördelningen. En fas som inte körs har noll besök;
den är inte ett uppmätt snabbt arbete. Kontroll utan emission besöker inte
intervallanalys, registerallokering eller skaläremission. Profilering ändrar inte den byggda
bilden. Utan flaggan tas inga inre tidsstämplar eller arbetsräknaruppdateringar.
Profileringen har själv en kostnad, så vanliga byggen används i jämförande
prestandaserier.

### Räkna allokeringar i det byggda programmet

```sh
dunder build examples/hello.ddr --runtime-profile -o build/profiled-hello
./build/profiled-hello
```

Efter en normal retur från `main` skriver programmet en `runtime_profile`
v1-JSON-post på stderr. Den räknar utförda allokeringar, begärda och
storleksklassade payloadbyte, nya och återanvända block, retain/release-anrop,
arraykloner, strängsnitt, postkonstruktion, postkloner och kopierade byte.
Heltalsräknarna är modulo 2^64. Retain/release räknar även anrop med noll och
odödliga strängar. Dessa trafikmått mäter inte levande minne eller toppminne.
Rapportformatet allokerar inget och dess poolinitiering räknas inte som en
vanlig allokering. Vanlig stdout och programmets exitstatus bevaras, även om
stderr är stängd. Fällor och explicita processavslut har ingen slutrapport.

Flaggan stöds för de tre nativa målen. `check` accepterar inte flaggan.
Cachefragment och `test --affected` skiljer profilerade och vanliga
byggen åt. Kör allokering och kompilatorns fastider i separata observationer:
klockans egna temporära värden skulle annars ingå i allokeringstrafiken.
[Oberoende dogfood-fall](../benchmarks/dogfood/README.md) har ett verktyg för
validerad allokeringsattribuering och en separat vanlig tidsserie.

## Kör berörda tester

`test --affected` kör alla grupper första gången. Därefter återanvänder den
godkända resultat för grupper vars transitivt nåbara funktionskroppar och
typkontext matchar. Även funktioner som används som värden ingår. En ändrad
tidsgräns eller ett ändrat skaläroptimeringsläge ger en ny körning.
Tester med nåbara systemanrop, råpekare eller
externa anrop körs varje gång eftersom omvärlden kan ha ändrats.
Misslyckade grupper behöver en ny godkänd körning. Vanlig `dunder test` kör
alltid hela sviten.

Resultatkvitton och kodfragment finns i den valda testutdatakatalogen
(`-o`). Sammanfattningen anger hur många grupper som hoppades över och hur
många som gick igenom eller misslyckades.
