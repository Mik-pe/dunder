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
layout och funktionens tokenström. På macOS används SHA-256 av den laddade
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

Klockan väljs efter kompilatorns värdplattform, även vid korsbyggen. Ett
klockfel ger `clock_ok: false` och otillgängliga intervall som `null`.
Lyckade byggen och kontroller ger rapporter; misslyckade eller enbart
listande kommandon ger ingen avslutad tidsrapport. Vanlig stdout och
`check --agent` behåller sina format. Utan flaggan tas inga tidsstämplar.

## Kör berörda tester

`test --affected` kör alla grupper första gången. Därefter återanvänder den
godkända resultat för grupper vars transitivt nåbara funktionskroppar och
typkontext matchar. Även funktioner som används som värden ingår. En ändrad
tidsgräns ger en ny körning. Tester med nåbara systemanrop, råpekare eller
externa anrop körs varje gång eftersom omvärlden kan ha ändrats.
Misslyckade grupper behöver en ny godkänd körning. Vanlig `dunder test` kör
alltid hela sviten.

Resultatkvitton och kodfragment finns i den valda testutdatakatalogen
(`-o`). Sammanfattningen anger hur många grupper som hoppades över och hur
många som gick igenom eller misslyckades.
