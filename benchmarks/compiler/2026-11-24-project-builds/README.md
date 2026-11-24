# Byggtid för Dunderprojekt

Median i millisekunder för hela `dunder build` från processstart till färdig binär.
Den självbyggda kompilatorn från `731db56` jämförs med 0.12.0. Apple M5 hade 10 jobb i autoläge och Intel Core i5-1035G7 hade 8.
Efter ett uppvärmningspar kördes sju mätpar på Macen och fem på Linux, med växlande körordning och varm filsystemscache. [Källrevision](https://github.com/Mik-pe/dunder/commit/731db56)

| Värd | Projekt | Jobb | Bygge | 0.12.0, ms | 731db56, ms |
| --- | --- | --- | --- | ---: | ---: |
| arm64-macos | Tabell | auto | Rent | 51,80 | 50,83 |
| arm64-macos | Tabell | auto | Oförändrat | 52,22 | 12,16 |
| arm64-macos | Tabell | 4 | Rent | 56,75 | 54,64 |
| arm64-macos | Tabell | 4 | Oförändrat | 56,76 | 12,17 |
| arm64-macos | Kompilatorn | auto | Rent | 2192,19 | 1897,75 |
| arm64-macos | Kompilatorn | 4 | Rent | 2519,28 | 2253,32 |
| x86_64-linux | Tabell | auto | Rent | 888,17 | 865,42 |
| x86_64-linux | Tabell | auto | Oförändrat | 887,49 | 163,12 |
| x86_64-linux | Tabell | 4 | Rent | 928,68 | 914,99 |
| x86_64-linux | Tabell | 4 | Oförändrat | 927,04 | 161,27 |

Tabell byggdes med `dunder build -o <fil>` från projektkatalogen. Vid rena byggen togs utfilen bort före varje körning. Vid oförändrade byggen kördes samma kommando igen. Den gamla och nya kompilatorn hade varsin utfil. På båda värdarna gav den byggda Tabell-binären exakt innehållet i `expected.csv` för provet med `expenses.csv`. [Tabells låsta källor](https://github.com/Mik-pe/dunder-hundmat/tree/b08de81602a5a70bd5cebd19f88b6a7763946cb8/tabell)

Självbygget använde `dunder build selfhost/main.ddr -o <fil>` från samma källträd i båda fallen. Med fyra jobb tillkom `--jobs 4`. De färdiga binärerna jämfördes byte för byte. [Kompilatorns låsta källor](https://github.com/Mik-pe/dunder/tree/4721c31) · [Alla mätpar](samples.csv)

Ett projekt med bara `fn main() { print("hello") }` tog 14,98 ms för ett rent bygge på Linux med 0.12.0 och 14,53 ms med `731db56` (elva mätpar). Projekt under 512 upplösta token kompileras direkt.
