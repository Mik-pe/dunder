# Dunders funktionspipeline på en belastad byggmaskin

Jämförelsen gäller kompilatorerna vid `4585bc4fc` och `cd3ad46ef` på Apple M5 (macOS 27.2). Båda byggde samma låsta programkällor. Tabell kom från dunder-hundmat vid `e3b7310f6`.

Under mätningen körde andra Dunder-arbeten verifieringssviter på samma dator. Filindexering och filsynkronisering pågick också. En stickprovsmätning under det blandade korpuset visade belastning 50,25 i enminutssnitt på tio kärnor. För det korpuset varierade baslinjens prover mellan 286 och 2 285 ms och kandidatens mellan 267 och 3 118 ms. Kvoten mellan medianerna blev 0,81 och medianen av de tolv parvisa kvoterna 0,94. Tiderna beskriver den här belastade körningen.

## Kompilering

Median och p95 bygger på 12 provpar per program efter ett uppvärmningsbygge per variant. Varje bygge startade en ny process med fyra workers, varm filsystemscache och avstängd inkrementell cache. Tiden omfattar start, källinläsning, maskinkod, Mach-O, signering och publicering. Varianterna växlade startordning mellan paren. Kolumnerna för RSS visar medianen av 4 separata mätningar av processens maximala residenta minne.

| Program | Före, ms | Efter, ms | Förändring | p95 före, ms | p95 efter, ms | RSS före, MiB | RSS efter, MiB |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Identiska kroppar | 93,81 | 82,52 | −12,04 % | 117,80 | 117,05 | 54,70 | 47,72 |
| Unika konstanter | 223,24 | 225,61 | +1,06 % | 259,32 | 291,34 | 54,70 | 47,78 |
| Blandade uttryck | 1281,76 | 1041,79 | −18,72 % | 2285,06 | 3118,19 | 73,69 | 56,45 |
| Låst självbygge | 524,17 | 313,99 | −40,10 % | 1156,98 | 820,72 | 79,16 | 56,52 |
| Tabell | 11,28 | 9,06 | −19,64 % | 20,06 | 12,20 | 7,14 | 6,75 |
| Stor enskild funktion | 88,59 | 63,56 | −28,25 % | 102,81 | 64,32 | 59,42 | 53,09 |
| Anrop med varierade typer | 56,35 | 39,58 | −29,77 % | 64,55 | 40,93 | 17,62 | 13,97 |

De tre första korpusarna har vardera 101 024 rader fördelade över 1 000 hjälpfunktioner. Den stora funktionen gör 4 096 beroende heltalsuppdateringar. Anropskorpusen innehåller 256 grupper som skapar records med strängar och arrayer, muterar genom lån och matchar en enum. Självbygget kompilerar basrevisionens `selfhost/main_parallel.ddr` och bygger och kör sedan hello med den nya binären.

För samtliga sju byggfall kontrollerades utdata och identiska binärer mellan en och fyra workers inom respektive revision. Varje uppmätt bild hade samma SHA-256 som sin uppvärmningsbild. Den signerade filen för blandade uttryck gick från 1 264 160 till 1 247 744 byte, det låsta självbygget från 2 905 760 till 2 396 864 byte och Tabell från 65 792 till 49 376 byte. [Byggprover, minnesprover och kontrollsummor](builds.json)

Kandidaten byggde dessutom sig själv från en ren kopia av sin egen revision till exakt den uppmätta binären med SHA-256 `232a1bf024ed4a7363bc9282ec132122bc6e6c2be753e00cc37af4c079ed9373`. Hundmats standardsvit körde 53 språk-/dogfoodtester, 14 Tabelltester, 10 HTTP-tester och programmens CSV- och HTTP-integrationer samt kontrollerade genererade Vulkan-, Metal- och WebGPU-artefakter.

## Native program

Varje variant kördes en gång för uppvärmning och därefter i 30 alternerande provpar per program. Körtiden inkluderar processstart. Heltalsberäkning, arrayskanning, UTF-8-text och sortering kördes med tio gånger ordinarie arbetsmängd. Tabell grupperade och summerade 32 768 genererade CSV-rader med 402 grupper, UTF-8 och citerade fält. Varje programs standardutdata jämfördes med en separat referensberäkning.

| Program | Före, ms | Efter, ms | Förändring | p95 före, ms | p95 efter, ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Heltalsberäkning | 37,64 | 37,60 | −0,10 % | 39,08 | 40,70 |
| Arrayskanning | 9,44 | 9,49 | +0,55 % | 10,03 | 10,09 |
| UTF-8-text | 20,81 | 20,69 | −0,59 % | 21,43 | 21,14 |
| Sortering | 9,78 | 9,49 | −2,91 % | 10,80 | 11,27 |
| Tabell, 32 768 CSV-rader | 25,32 | 23,25 | −8,18 % | 26,29 | 23,74 |

[Körtidsprover](runtime.csv), [sammanställning](runtime-summary.csv) och [indata, binärer och förväntade resultat](runtime-metadata.json).

## Reproducera

Bygg kompilatorerna från de två revisionerna med `tools/bootstrap.sh`. `OLD_SRC` är en ren utcheckning av `4585bc4fc`, `OLD` och `NEW` deras absoluta binärsökvägar och `TABELL` sökvägen till Tabells projektkatalog.

```sh
dunder run benchmarks/compiler/2026-11-15-worker-lift/measure.ddr -- \
  "$OLD" "$NEW" "$OLD_SRC" "$TABELL" builds.json --rounds 12 --frontend-stress --rss-rounds 4
python3 tools/performance.py "$OLD" "$NEW" runtime-out \
  --suite runtime --rounds 30 --runtime-scale 10 --tabell "$TABELL"
```
