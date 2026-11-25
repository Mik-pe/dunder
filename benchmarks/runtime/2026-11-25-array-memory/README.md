# Arrayliteraler och minnesrutiner på Apple M5

En dynamisk arrayliteral med ett heltal reserverade en plats. För 16 384
rader sjönk den sammanlagda kapaciteten från 131 072 till 16 384. Efter
1 024 `push` på en klon hade originalet kvar sitt enda värde.

| macOS ARM64 | Före | Efter |
| --- | ---: | ---: |
| Högsta RSS för 16 384 korta heltalsrader | 4 240 KiB | 3 984 KiB |
| Högsta RSS för 16 384 textdokument med ett spann | 10 992 KiB | 9 440 KiB |
| Tid att lägga till 1 024 heltal efter det första | 1 096 ns | 1 101 ns |

RSS är medianen av tre nya processers högsta RSS enligt `getrusage`.
Pushprovet kontrollerar sista värdet för varje array. Tiden är medianen
av tre processmedianer, med 100 uppmätta omgångar om 32 arrayer per process.
[RSS-prover](rss.tsv) · [Pushprov](push.ddr)

| Mål | Nolla två plan, ns | Jämför två plan radvis, ns | Jämför ett heltalsplan radvis, ns |
| --- | ---: | ---: | ---: |
| macOS ARM64 | 838 → 835 | 1 059 → 1 054 | 698 → 704 |
| Linux ARM64, Kiri | 846 → 846 | 1 145 → 1 041 | 701 → 701 |
| Linux x86-64, översatt i Kiri | 26 015 → 276 | 40 862 → 3 195 | 28 235 → 1 503 |

Tabellen visar medianen av tre processmedianer för 24 rader med bredd 96.
För det separata heltalsplanet på macOS användes 15 processer per variant.
Provet med båda planen körde även bredderna 95 och 97 och kontrollerade
skillnader i sista cellen i vartdera planet. Kontrollsummorna var 45 120
respektive 330 880 för alla körningar. TSV-filen innehåller median och p95
från varje process, inklusive kontrollfallen med typad loop och systemets
`memcmp`. Nativa Linux och Windows x64 återstår för prestandaverifiering.
[Samtliga tider](samples.tsv)

Värdena mättes med Apple M5, 24 GiB och macOS 27.2.1 (26B131). Linux kördes
med kärna 6.18.58 i Kiri. Förekompilatorn byggdes från `f93b8a1`. Kandidaten
byggdes från samma revision med ändringarna för dynamiska arrayliteraler
och x86-minnesrutiner. Byggena använde standardoptimering. Först kördes
förekompilatorns program, sedan kandidatens. Ordningen växlade för varje
omgång. Arbetslasterna kommer från en fast källrevision.
[Korta arrayer](https://github.com/Mik-pe/dunder-tui/blob/e281b84f6943a96e004a2160c4ea2835c8ce08f8/benchmarks/language/short_arrays.ddr) ·
[Textlagring](https://github.com/Mik-pe/dunder-tui/blob/e281b84f6943a96e004a2160c4ea2835c8ce08f8/benchmarks/text_storage.ddr) ·
[Cellplan](https://github.com/Mik-pe/dunder-tui/blob/e281b84f6943a96e004a2160c4ea2835c8ce08f8/benchmarks/language/cell_planes.ddr) ·
[Radlikhet](https://github.com/Mik-pe/dunder-tui/blob/e281b84f6943a96e004a2160c4ea2835c8ce08f8/benchmarks/language/row_equality.ddr)
