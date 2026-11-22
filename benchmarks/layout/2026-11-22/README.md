# Rörelsesimulering 2026-11-22

Varje variant byggdes och kördes 30 gånger med samma Dunderrevision
`030ff7d3f704e99fc1acccc22347364b1083ba1b`. Samtliga 120 körningar gav
`16774246` för 101 steg och startvärde 1729. Tiderna är millisekunder.
Medianen är medelvärdet av de två mittproverna och p95 är det 29:e sorterade
provet i varje grupp. [Arbetslast och mätmetod](../README.md)

| Värd och mål | Representation | Bygge median | Bygge p95 | Körning median | Körning p95 |
| --- | --- | ---: | ---: | ---: | ---: |
| Apple M5, macOS ARM64 | Array med poster | 27,03 | 28,12 | 37,04 | 40,17 |
| Apple M5, macOS ARM64 | En array per fält | 24,57 | 26,34 | 35,90 | 39,18 |
| Översatt Linux x86-64 | Array med poster | 492,87 | 835,66 | 17,93 | 28,85 |
| Översatt Linux x86-64 | En array per fält | 449,37 | 778,77 | 17,95 | 27,86 |

ARM64-processerna kördes direkt på macOS 27.2.1. Linux-processerna kördes i en
lokal Docker-gäst på samma M5. Både kompilatorn och programmen var x86-64 ELF.
Gästens tider inkluderar maskinkodsöversättning. [Råprover macOS](arm64-macos.csv)
· [Råprover översatt Linux](x86_64-linux-translated.csv)

De självbyggda kompilatorernas SHA-256 var
`98ba5b1194b8e3a02229354816dc260c67db1f53a3f796350caffa65fd99c870` på macOS
och `b79ec9a4d85db7e1814522d6d7879b999f980c23a3566c4da0b7110b09734e4b` i Linux.

Kolumnprogrammets uppdateringsloop använde skalär `fmul` och `fadd` med
S-register på ARM64, respektive `mulss` och `addss` på x86-64. För samma
punktloop i C-referensen rapporterade Apple Clang 21 fyra element per vektor
och GCC 12.2 både 16- och 8-bytesvektorer med `-O3 -ffp-contract=off`.
Även C-referensen gav `16774246` på båda målen.
