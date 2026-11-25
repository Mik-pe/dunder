# Iteration över tolkad JSON

Apple M5, 24 GiB, macOS 27.2.1 och Linux ARM64 i Kiri på samma dator.
Standardprofilen bygger [mätprogrammet](iterate.ddr). Det läser varje värde
i ett redan tolkat dokument från början till slut. För objekt räknar det
också nyckelns UTF-8-längd. Varje storlek körs tre gånger för uppvärmning
och därefter 30 gånger med kontroll av summan efter varje tidtagning.

| Samling | Antal barn | Kontrollsumma | macOS median / p95, µs | Linux median / p95, µs |
| --- | ---: | ---: | ---: | ---: |
| Array | 1 024 | 523 776 | 35 / 36 | 35,6 / 41,7 |
| Array | 2 048 | 2 096 128 | 71 / 73 | 71,1 / 78,0 |
| Array | 4 096 | 8 386 560 | 143 / 147 | 144,1 / 149,7 |
| Objekt | 1 024 | 527 786 | 66 / 68 | 63,6 / 69,3 |
| Objekt | 2 048 | 2 105 258 | 140,5 / 149 | 129,9 / 130,9 |
| Objekt | 4 096 | 8 405 930 | 274 / 286 | 268,0 / 276,4 |

Medianen är medelvärdet av observation 15 och 16 och p95 är observation 29
i den sorterade serien. [Rådata från macOS](macos-arm64.tsv) · [Rådata från Linux](linux-arm64.tsv)
