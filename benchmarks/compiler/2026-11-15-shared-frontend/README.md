# Byggtid och programkörtid för Dunders funktionspipeline

Jämförelsen gäller kompilatorerna vid `4585bc4fc` och `c355c2ad8` på Apple M5 (macOS 27.2). Båda byggde samma låsta programkällor. Tabell kom från dunder-hundmat vid `e3b7310f6`.

Teamet pausade övriga Dunder-byggen och verifieringsjobb på värden under mätningen. macOS fortsatte sina bakgrundstjänster. Enminutsbelastningen låg mellan 6,99 och 12,46 under de 73 sekunder som huvudserien och kontrollerna tog. [Värdlogg](host-load.csv)

## Kompilering

Median och p95 bygger på 12 provpar per program efter ett uppvärmningsbygge per variant. Varje bygge startade en ny process med fyra workers, varm filsystemscache och avstängd inkrementell cache. Tiden omfattar start, källinläsning, maskinkod, Mach-O, signering och publicering. Varianterna växlade startordning mellan paren. Kolumnerna för RSS visar medianen av 4 separata mätningar av processens maximala residenta minne.

| Program | Före, ms | Efter, ms | Förändring | p95 före, ms | p95 efter, ms | RSS före, MiB | RSS efter, MiB |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Identiska kroppar | 82,13 | 76,14 | −7,29 % | 92,29 | 77,78 | 54,69 | 48,03 |
| Unika konstanter | 146,57 | 135,61 | −7,48 % | 147,77 | 136,30 | 54,70 | 49,70 |
| Blandade uttryck | 220,13 | 202,61 | −7,96 % | 232,53 | 205,62 | 73,69 | 55,45 |
| Låst självbygge | 468,17 | 359,79 | −23,15 % | 481,94 | 364,07 | 79,16 | 51,72 |
| Tabell | 10,91 | 10,05 | −7,84 % | 18,35 | 13,30 | 7,14 | 7,22 |
| Stor enskild funktion | 87,95 | 66,26 | −24,66 % | 99,49 | 67,42 | 59,42 | 52,25 |
| Anrop med varierade typer | 53,09 | 49,71 | −6,37 % | 64,97 | 50,41 | 17,63 | 13,43 |

De tre första korpusarna har vardera 101 024 rader fördelade över 1 000 hjälpfunktioner. Den stora funktionen gör 4 096 beroende heltalsuppdateringar. Korpuset med varierade anrop innehåller 256 grupper som skapar records med strängar och arrayer, muterar genom lån och matchar en enum. Varje självbyggesprov mäter ett bygge av basrevisionens `selfhost/main_parallel.ddr` från en ny process. Efter uppvärmningsbygget fick varje ny kompilator bygga och köra hello. Den fullständiga bootstrapkedjan kördes separat.

För hela bygget motsvarade kandidatens median 498 606 rader/s för blandade uttryck och 1 326 793 rader/s för identiska kroppar. Den signerade bilden gick från 1 264 160 till 1 247 744 byte för blandade uttryck, från 2 905 760 till 2 380 448 byte för det låsta självbygget och från 65 792 till 49 376 byte för Tabell.

För samtliga sju byggfall kontrollerades utdata och identiska binärer mellan en och fyra workers inom respektive revision. Varje uppmätt bild hade samma SHA-256 som sin uppvärmningsbild. [Byggprover, minnesprover och kontrollsummor](builds.json)

En separat uppbyggnad av kandidatens egna källor från det frysta bootstrapfröet nådde fixpunkt, och en och fyra workers gav exakt den binär som användes i mätningen. SHA-256 var `3e4337a770eb7e3acd9d55049b2ccd697f6c75f9a9c066f08ceafaa64ead9382`. Hundmats standardsviter passerade med denna binär, inklusive Tabells CSV-kontroller, HTTP-integration och grafiska korsbyggen.

## Körprestanda

Varje variant kördes en gång för uppvärmning och därefter i 30 alternerande provpar per program. Körtiden inkluderar processstart. Heltalsberäkning, arrayskanning, UTF-8-text och sortering kördes med tio gånger ordinarie arbetsmängd. Tabell grupperade och summerade 32 768 genererade CSV-rader med 402 grupper, UTF-8 och citerade fält. Varje programs standardutdata jämfördes med en separat referensberäkning.

| Program | Före, ms | Efter, ms | Förändring | p95 före, ms | p95 efter, ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Heltalsberäkning | 37,50 | 37,38 | −0,32 % | 39,86 | 41,23 |
| Arrayskanning | 9,36 | 9,37 | +0,01 % | 11,01 | 12,49 |
| UTF-8-text | 20,67 | 20,57 | −0,47 % | 21,03 | 21,07 |
| Sortering | 9,76 | 9,50 | −2,64 % | 11,56 | 12,07 |
| Tabell, 32 768 CSV-rader | 25,03 | 22,72 | −9,25 % | 25,49 | 23,16 |

De fyra syntetiska programmens p95 ökade trots små förändringar i medianen. Tabells median och p95 sjönk i denna serie. [Körtidsprover](runtime.csv), [sammanställning](runtime-summary.csv) och [indata, binärer och förväntade resultat](runtime-metadata.json).

## Kontroll av schemaläggningen

Tre separata kontroller jämförde hela kompilatorrevisioner på de låsta källorna i fyra växelvis ordnade provpar efter uppvärmning. Tabellen visar medianen av de parvisa tidskvoterna för varje arbetslast. Ett värde över ett anger längre byggtid i den senare revisionen.

| Jämförelse | Unika konstanter | Blandade uttryck | Låst självbygge | Tabell |
| --- | ---: | ---: | ---: | ---: |
| Två pooler → gemensam gräns | 1,68 | 1,40 | 1,12 | 1,05 |
| Första gemensamma poolen → slutrevision | 0,63 | 0,72 | 1,05 | 1,08 |
| Seriell nåbar frontend → slutrevision | 0,92 | 0,92 | 1,22 | 1,11 |

- Från `2853f52` till `52fa187` gick kompilatorn från separata pooler för frontend och backend till en gemensam pool med sammanlagt fyra utestående jobb. [Rådata](control-two-pools.json)
- Från `52fa187` till `c355c2a` började koordinatorn kontrollera den första tillgängliga kroppen medan poolen kunde kontrollera andra funktionskroppar och sänka färdiga kroppar. [Rådata](control-bounded-pool.json)
- Från `3603267` till `c355c2a` flyttades typning av nåbara kroppar till workers med ordnad publicering hos koordinatorn. [Rådata](control-pre-frontier.json)

De tre kontrollserierna går att upprepa med samma byggkommando som huvudserien och de angivna kompilatorrevisionerna. Använd fyra rundor och utelämna flaggorna för de två extra korpusarna och RSS.

## Reproducera

Bygg kompilatorerna från de två revisionerna med `tools/bootstrap.sh`. `OLD_SRC` är en ren utcheckning av `4585bc4fc`, `OLD` och `NEW` deras absoluta binärsökvägar och `TABELL` sökvägen till Tabells projektkatalog.

```sh
python3 benchmarks/compiler/2026-11-15-worker-lift/measure.py \
  "$OLD" "$NEW" "$OLD_SRC" "$TABELL" builds.json --rounds 12 --frontend-stress --rss-rounds 4
python3 tools/performance.py "$OLD" "$NEW" runtime-out \
  --suite runtime --rounds 30 --runtime-scale 10 --tabell "$TABELL"
```
