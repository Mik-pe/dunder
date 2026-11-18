# Parallell byggtid på Apple M5

Båda kompilatorerna byggdes från det frysta fröet med `tools/bootstrap.sh`.
Baslinjen och självbyggets källor är låsta vid `c6773583e4fa`.
Tabell är låst vid `d0c9dbc8903b` i dunder-hundmat.
Mätningen kördes på Apple M5 med 10 CPU-kärnor.

| Program | Mål | Jobbudget | Före, median ms | Efter, median ms | Kortare byggtid |
| --- | --- | --- | ---: | ---: | ---: |
| Låst självbygge | `arm64-macos` | 4 | 1039,52 | 598,16 | 42,46 % |
| Låst självbygge | `arm64-macos` | auto | 675,42 | 510,08 | 24,48 % |
| Låst självbygge | `arm64-macos` | 2 | 1448,62 | 876,57 | 39,49 % |
| Låst självbygge | `x86_64-linux` | auto | 705,61 | 564,40 | 20,01 % |
| Tabell | `arm64-macos` | auto | 18,07 | 17,65 | 2,35 % |
| Genererat program med blandade uttryck | `arm64-macos` | 4 | 321,56 | 249,45 | 22,43 % |
| Genererat program med blandade uttryck | `arm64-macos` | auto | 308,33 | 237,20 | 23,07 % |

Med automatisk jobbudget gick modulladdning, lexning och namnupplösning från 188,0 till 152,5 ms. Kompileringsfasen gick från 436,7 till 341,8 ms och Mach-O-formatering inklusive signering från 44,6 till 9,0 ms.

Varje värde bygger på tolv alternerande mätpar efter ett uppvärmningspar.
Varje prov startade en ny process med varm filsystemscache och avstängd
inkrementell cache. Körkommandot var `dunder build <källa> --target <mål> --timings=json -o <ny fil>`,
med `--jobs 2` eller `--jobs 4` för serierna med explicit jobbudget.
Tiden omfattar hela processen. Alla bilder var byteidentiska mellan
kompilatorerna och mellan prov i samma serie. De självbyggda macOS-kompilatorerna
byggde och körde hello. Tabell och det genererade programmet kördes med
kontrollerad standardutdata. ELF-bildernas format och arkitektur kontrollerades.
[Råvärden](samples.csv) · [Kompilatorer, källrevisioner och SHA-256](metadata.json)

Det genererade programmet bygger på `corpus()` i `tools/performance.py`
och `make_unique(..., True)` i det tidigare mätverktyget för funktionslokalt MIR.
[Generator för blandade uttryck](../2026-11-15-worker-lift/corpus.ddr)
