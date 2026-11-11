# Verifierade framefria ARM64-funktioner

Numerikkärnans median förbättras ytterligare 9,0 %, från 7,28 till 6,62 ms.
Av/på-serien ger 9,3 %. Lexerfallet förbättras 1,8 %. Byggmedianerna ligger
nära föregående nivå, men flera p95 ökar. C-geomedlet är 1,552× och
sorteringens 2,184× är största gapet; målen 1,10×/1,25× är inte uppnådda.

## Mekanism och identitet

Registerplacerade anropsfria funktioner med `i32`, `i64` och `bool` kan
behålla anroparens FP/LR på ARM64. Eligibility granskar alla använda
registerplatser, inklusive syntetiska loopgränser. Arrayer, adresserbara
värden, spill och okända operationer behåller frame. En verifierad
parallellflytt hanterar överlappande argument; en separat observation
kontrollerar att den faktiskt emitterade kroppen saknar anrop.
[Kontrakt](../../docs/optimization.md#framefria-registerfunktioner-på-arm64).

Policyn gäller ARM64/macOS och ARM64/Linux. x86-64 och Wasm behåller
befintlig emission. `--frame-free=off` isolerar mekanismen. Cache och
berörda testkvitton skiljer lägena åt.

Kompilatorkoden är `e11759f`, jämförd med `6e2331e`. Kandidatens SHA-256 är
`c9af03314377b03b5990b168ac6d6d0dca71a48fc69e3bc4305a9e1b48ddca29`.
Båda bygger samma slutliga källor. Varje serie har 30 färska processer per
variant, växlande ordning, varm OS-cache och ingen Dunder-cache på Apple M5.
Varje resultat kontrolleras. Vanliga binärer utan profileringsflaggor används.

## C-fall och bygglatens

Median / nearest-rank p95 i millisekunder. C använder clang `-O3 -ftrapv`
och saknar Dunders arraygränskontroller.

| Arbete | Före | Efter | C | Efter / C, median |
| --- | --- | --- | --- | --- |
| Numerik | 7,28 / 11,00 | 6,62 / 12,89 | 4,56 / 5,99 | 1,452× |
| Arrayläsning | 3,04 / 6,45 | 3,03 / 5,06 | 2,44 / 4,19 | 1,241× |
| UTF-8-text | 3,74 / 7,16 | 3,75 / 5,84 | 2,55 / 4,15 | 1,473× |
| Sortering | 5,08 / 9,17 | 5,11 / 7,43 | 2,34 / 3,62 | 2,184× |

Bara numerikbilden ändras; de övriga tre är byteidentiska. Numerikens
p95 stiger 17,1 % i före/efter-serien. Av/på-serien ger däremot
7,26 / 8,04 ms av och 6,59 / 7,58 ms på. Båda fördelningarna bevaras.

| Byggarbete | Före median / p95 | Efter median / p95 |
| --- | --- | --- |
| Genererat bygge | 312,41 / 315,51 | 311,45 / 323,27 |
| Genererad kontroll | 182,01 / 185,28 | 179,36 / 182,06 |
| Självbygge | 163,92 / 164,71 | 164,01 / 181,09 |
| Självkontroll | 103,25 / 103,75 | 103,55 / 104,11 |
| Tabellbygge | 10,74 / 13,71 | 10,71 / 15,41 |
| Tabellkontroll | 4,94 / 5,93 | 4,91 / 6,34 |

Självbyggets p95 ökar 9,9 % och Tabellbyggets 12,4 %. Genererad
genomströmning är cirka 312 500 rader/s vid p95. Dess bild minskar från
2 446 112 till 2 413 280 byte; övriga byggbilders storlekar är oförändrade.
Av/på-serien visar 0,72 % medianpåslag för genererat bygge och 0,02 % för
självbygge. Självbyggets av/på-p95 är 169,44 / 166,73 ms.
[Råtider](final/samples.csv), [sammanfattning](final/summary.csv),
[metadata](final/metadata.json), [av/på-råtider](ablation/samples.csv)
och [av/på-metadata](ablation/metadata.json).

## Oberoende konsumenter och korrekthet

Oförändrad arbetsmängdsrevision 2 används. Medianerna före/efter är
POD 48,74 / 48,83 ms, decimal 71,87 / 72,07 ms, lexer 91,64 / 89,98 ms
och symboltabeller 99,23 / 99,21 ms. Lexerns p95 går 92,99 till 90,98 ms;
symboltabellens stiger 99,68 till 100,47 ms. Lexerns separata av/på-vinst
är 1,5 %. Fall utan C-referens ingår inte i C-geomedlet.
[Råtider](dogfood/samples.csv), [metadata](dogfood/metadata.json),
[av/på-resultat](dogfood-ablation/summary.csv).

`tools/verify-selfhost.sh` passerade: 105 nativa grupper, 97 Wasm-grupper,
åtta explicit nativa grupper, 50 diagnostikfall, 46 runtimekontrakt per
native/Wasm och tolv CLI-grupper på macOS. Bootstrap nådde identisk fixpunkt.
Frameprovet bygger 88 bilder på fyra mål och två lägen; 44 host/Wasm-körningar
provar FP/SP, callee-saved register, åtta argument, cykler, okononiska i32-bitar,
spill, loopar och fällordning. Symbolisk tolkning verifierar emitterade MOV-ord
över 40 320 permutationer. Fallback- och övriga målens bilder är byteidentiska.
Linuxbilder har byggts men inte körts här.

Nästa arbete är intervall-/längdbevis för arrayer och explicit runtimeinvariant
för längder. För bredare språkfall finns planer för
[ägartillstånd](../../docs/ownership-ir-plan.md) och
[destinationsbaserade POD-värden](../../docs/pod-values-plan.md).
