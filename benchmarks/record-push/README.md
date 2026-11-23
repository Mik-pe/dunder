# Små poster i en array

Fyra varv fyller 4 096 poster med ett heltal och ett flyttal och läser tillbaka båda fälten. Samtliga sex varianter ger summan 33 595 386. I de två första behåller den ägda arrayen sin kapacitet mellan varven. De andra fyra använder samma förallokerade region med 4 096 platser. På den lagringen jämförs tilldelning av en hel post med att konstruera värdet genom att skriva dess två fält direkt.

| Representation och skrivning | Allokeringsanrop | Färska | Poolåterbruk | Median µs | p95 µs |
| --- | ---: | ---: | ---: | ---: | ---: |
| Vanlig post, push | 16 387 | 4 099 | 12 288 | 99,4 | 101,8 |
| C-layout, push | 16 387 | 4 | 16 383 | 148,1 | 173,2 |
| Vanlig post, region och literal | 2 | 2 | 0 | 82,9 | 100,1 |
| Vanlig post, region och fält | 2 | 2 | 0 | 82,7 | 96,5 |
| C-layout, region och literal | 16 386 | 3 | 16 383 | 242,3 | 257,9 |
| C-layout, region och fält | 2 | 2 | 0 | 90,9 | 94,8 |

En vanlig post med dessa två skalärfält konstrueras på stacken. Vid `push` kopieras den till ett separat ägt objekt som arrayen tar över. Profilen registrerar 16 384 postkopior. En C-layoutliteral får i stället en egen temporär buffert. `push` kopierar dess 16 byte till arrayelementet och frigör temporären. Profilen registrerar 16 384 konstruktioner och 262 144 kopierade postbyte.

På samma regionlagring minskade direkta fältskrivningar mediantiden för C-layout med 62,5 procent och tog bort en allokering per post. För vanliga poster ligger de två medianerna inom 0,2 µs. Regionens längd är känd vid inträdet. Ett framtida försök med direkt konstruktion i en växande array måste även bevara evalueringsordningen om ett fältuttryck ändrar arrayen.

Mätningen kördes på Apple M5 med macOS 27.2.1. Profilräknarna ovan gäller en separat process per variant. Tiderna gäller samma arbete i ett bygge utan profilräknare. Efter ett uppvärmningsvarv roterar verktyget ordningen i 30 omgångar om 32 körningar per variant. Tabellen visar tid per körning. p95 är det 29:e sorterade provet. [Arbetslast](records.ddr) · [Råprover](arm64-macos.csv)

```sh
d=$(tools/bootstrap.sh)
"$d" build --runtime-profile benchmarks/record-push/records.ddr -o build/record-profile
for variant in plain flat plain-literal plain-fields flat-literal flat-fields; do build/record-profile "$variant"; done
"$d" build benchmarks/record-push/records.ddr -o build/record-timings
build/record-timings timings
```
