# Byggtid på Apple M5

| Program | Före | Efter | Kortare byggtid |
| --- | ---: | ---: | ---: |
| Dunders parallella kompilator | 677,5 ms | 625,8 ms | 7,6 % |
| Genererat program med 101 024 rader | 526,3 ms | 501,7 ms | 4,7 % |
| Tabell | 14,91 ms | 14,03 ms | 5,9 % |

Tabellen visar medianen av tolv byggen per kompilator och program på samma Mac.
Varje bygge startade en ny kompilatorprocess med `build <källa> -o <binär> --jobs 4`.
Mätningen omfattade hela processen. Filsystemets cache var varm och den inkrementella
byggcachen var avstängd. Varje kompilator fick ett uppvärmningsbygge per program.
Körordningen växlade mellan mätparen. [Råtider, maskin och SHA-256](samples.json)

Båda kompilatorerna byggde samma källor. Självbygget använde
`selfhost/main_parallel.ddr` från `9011d86`. Det genererade programmet kom från
`corpus()` i samma versions `tools/performance.py`. Tabell kom från
`dunder-hundmat` och de fyra källfilernas hashvärden finns bland mätvärdena.

Binärerna för varje program var byteidentiska i samtliga körningar.
Självbygget återskapade referenskompilatorn. Det genererade programmet skrev
`595500` och Tabells resultat för `expenses.csv team cents` matchade `expected.csv`.
