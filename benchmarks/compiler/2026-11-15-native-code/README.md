# Nativ maskinkod och självbygge på Apple M5

Samma kompilatorkällkod byggde sig själv på 477,19 ms med den nya maskinkoden och på 548,22 ms med den gamla. De två kompilatorerna skrev byteidentiska körbara filer i samtliga fem byggfall. Därmed går det att tillskriva de 13,0 procenten kortare självbyggtid till hur den befintliga kompilatorn körs. Dess ARM64-kodsektion krympte från 2 900 116 till 2 822 412 byte.

## Byggtid

Båda mätserierna använder låsta indata från Dunder vid `9bab837` och samma CSV-program. Tabellen visar medianen av tolv byggen per variant med fyra workers. Hela processtiden ingår, inklusive Mach-O-signering. I kontrollserien fick den nya kodgeneratorn först kompilera den gamla kompilatorkällan. I den andra serien används den fullt ombyggda nya kompilatorn.

| Källa | Kontroll före (ms) | Kontroll efter (ms) | Fullt bygge före (ms) | Fullt bygge efter (ms) | Kortare byggtid | Rader/s efter |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Identiska funktionskroppar | 91,42 | 80,59 | 92,11 | 80,87 | 12,2 % | 1 249 269 |
| Unik konstant per funktion | 166,51 | 147,16 | 167,70 | 148,74 | 11,3 % | 679 219 |
| Unika kroppar med blandade uttryck | 262,79 | 220,88 | 264,35 | 221,86 | 16,1 % | 455 348 |
| Dunders kompilator | 548,22 | 477,19 | 547,51 | 458,91 | 16,2 % | — |
| Tabell (CSV) | 12,78 | 11,25 | 12,65 | 11,11 | 12,1 % | — |

Varje syntetiskt fall omfattar 101 024 rader och 1 000 hjälpfunktioner. Identiska kroppar drar nytta av kompilatorns återanvändning av funktionsmodeller. Varje bygge skriver en ny fil med inkrementell kompilering avstängd och varm filsystemscache. Ordningen växlar efter varje par. Programmens utdata kontrolleras och självbygget får kompilera och köra `examples/hello.ddr`. [Rådata för kontrollen](control.json) och [för den nya kompilatorn](builds.json).

## Programmens körtid

Trettio parade körningar per variant, med en uppvärmning och tio gånger arbetsmängden i ordinarie runtimesvit. Tiderna inkluderar processstart. Varje körnings resultat jämförs med en oberoende referensberäkning.

| Program | Före (ms) | Efter (ms) | Förändring i median | P95 före (ms) | P95 efter (ms) |
| --- | ---: | ---: | ---: | ---: | ---: |
| Heltalsberäkning | 35,43 | 35,48 | +0,1 % | 35,91 | 38,58 |
| Upprepad summering av en array | 9,58 | 9,04 | −5,6 % | 16,89 | 12,46 |
| UTF-8-text | 20,46 | 20,36 | −0,5 % | 20,85 | 20,77 |
| Sortering | 13,11 | 9,66 | −26,3 % | 14,19 | 11,34 |

Sorteringens median minskade med 26,3 procent. För heltalsberäkningen och text låg medianerna inom en procent av utgångsläget. Heltalsberäkningens p95 steg från 35,91 till 38,58 ms. [Enskilda körningar](runtime.csv) och [sammanställning](runtime-summary.csv).

## Kodgenerering

Uttryckskompilatorn och den optimerande skalära vägen använder `integer_emit.ddr` för kontrollerad addition, subtraktion och jämförelsevillkor. Anroparen bestämmer evalueringsordning och register, medan emittern bestämmer instruktionernas bredd, när overflow måste testas och hur resultatet utvidgas. Konstanta bitoperationer kan använda mottagarens register direkt. Arrayers elementbredd väljs på den typade anropsplatsen. Alla bredder delar samma tillväxtväg och kapacitetskontroll.

ARM64 kan läsa och skriva via bas och skalat index i en instruktion där registerlivslängden tillåter det. MIR beskriver bas, index, värde och minneseffekt i sina befintliga kolumner. Räknade loopar kan testa sitt villkor vid bakåthoppet. Konstantladdning väljer mellan MOVZ och MOVN följt av de MOVK som behövs. MIR lagrar konstantens färdiga värde, oberoende av vilken sekvens som bildade det.

## Reproducera

Kör på ARM64/macOS från den nya källkatalogen. `OLD_SRC` är en separat utcheckning av `9bab837`. `OLD` och `NEW` är absoluta sökvägar till respektive självbyggd kompilator, `CONTROL` namnet på en ny körbar fil och `TABELL` den externa Tabell-katalogen. Byggmätningarnas JSON-filer innehåller SHA-256 för kompilatorbinärer, korpus och Tabells källfiler. Mätmaskinen körde macOS 27.2.

```sh
(cd "$OLD_SRC" && "$NEW" build selfhost/main_parallel.ddr -o "$CONTROL")
python3 tools/performance.py "$OLD" "$NEW" runtime-results --suite runtime --runtime-scale 10 --rounds 30
measure=benchmarks/compiler/2026-11-15-worker-lift/measure.ddr
dunder run "$measure" -- "$OLD" "$CONTROL" "$OLD_SRC" "$TABELL" control-results.json --rounds 12
dunder run "$measure" -- "$OLD" "$NEW" "$OLD_SRC" "$TABELL" build-results.json --rounds 12
./tools/verify-selfhost.sh --jobs 4
```

Den frysta bootstrapkompilatorn byggde verktygskedjan till en byteidentisk fixpunkt. En och fyra workers producerade samma kompilator. Alla 80 verifieringssviter passerade.
