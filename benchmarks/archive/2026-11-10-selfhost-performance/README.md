# Självhostens runtime och byggtider, 2026-11-10

Mätt på Apple M5 med 24 GiB RAM, ARM64/macOS 27.2. Baseline är den
självbyggda kompilatorn från `c2eb946`. Kandidaten innehåller ändringarna
i samma commit som denna rapport. Båda är byggda av Dunder själv från det
frysta fröet och har verifierade byteidentiska efterföljande självbyggen.
Källhashar och binärernas SHA-256 finns i [metadata.json](metadata.json).
Metadatafältet `checkout` anger basrevisionen före incheckning;
`sources_sha256` identifierar kandidatens faktiska källor.

Ändringarna omfattar direkta typade läsningar och skalära tilldelningar
till lokaler, adresser som beräknas först när de behövs, register i stället
för stackspill för enkla högeroperander och looparnas interna operander,
ordvis buffertkopiering samt nyckelord och vanliga literaler i lexern.
Overflow-, index-, UTF-8- och ägarskapskontroller behålls. Mätningen visar
ändringarnas samlade effekt, inte varje deländrings enskilda bidrag.

## Kompilering

| Arbetsfall | Median före → efter | Kortare mediantid | p95 före → efter | Binärstorlek före → efter |
| --- | ---: | ---: | ---: | ---: |
| Genererat korpus | 757,229 → 418,184 ms | 44,8 % | 989,823 → 446,995 ms | 7 354 496 → 3 217 664 byte |
| Kompilatorns egen källa | 216,166 → 171,023 ms | 20,9 % | 218,074 → 179,522 ms | 1 346 240 → 1 198 496 byte |

Det genererade korpuset har 101 024 tokenbärande rader, 2 216 254 byte,
508 056 token, 21 moduler och 1 001 funktioner. Varje hjälpfunktion
anropas och bidrar till resultatet `595500`. Genomströmningen vid p95 är
102 063 → **226 007 rader/s**. Korpuset består av upprepad skalär kod och
är ett stressprov; det styrker inte hela språkets prestandakontrakt.

Varje prov startar en ny kompilatorprocess och skriver en ny färdig,
signerad Mach-O-fil. Inkrementell cache är avstängd, OS-filcachen är varm
och en arbetare används. Inläsning, typkontroll, kodgenerering, signering
och publicering ingår. Efter klockstopp körs varje stressbinär och dess
utskrift kontrolleras. Varje självbyggd kompilator måste bygga hello, vars
binär sedan måste skriva `42` och `true`. Alla 30 resultat per variant
måste också vara byteidentiska inom respektive byggarbetsfall.

Båda kompilatorerna bygger exakt samma slutliga källor, även i självprovet.
Tabellens självbyggda binärer skiljer sig därför från mätkörningens
baselinekompilator, som byggdes från den ursprungliga källrevisionen.

## Runtime

| Arbetsfall | Median före → efter | Kortare mediantid | p95 före → efter | Median C `-O3 -ftrapv` | Efter / C |
| --- | ---: | ---: | ---: | ---: | ---: |
| Numerik | 14,447 → 9,087 ms | 37,1 % | 15,946 → 11,279 ms | 4,705 ms | 1,93× |
| Arrayläsning | 15,491 → 10,826 ms | 30,1 % | 20,472 → 11,411 ms | 2,582 ms | 4,19× |
| UTF-8-text | 32,889 → 29,869 ms | 9,2 % | 33,669 → 30,637 ms | 2,824 ms | 10,58× |
| Sortering | 13,808 → 8,145 ms | 41,0 % | 16,095 → 8,742 ms | 2,586 ms | 3,15× |

De fyra frysta källorna och C-referensen ligger i
[`benchmarks/runtime/`](../runtime/). Varvantal, frö och UTF-8-text kommer
från programargument. Python beräknar oberoende förväntade resultat för
hela talföljden, arraysummeringen, Unicode-kodpunkter/ord och sorteringen.
Samtliga körningar måste ge exakt samma kontrollsumma som referensen.

C byggs med Apple Clang 21 och `-std=c11 -O3 -ftrapv`. C-koden har inte
Dunders arraygränskontroller. Väggtiden inkluderar processstart,
argumentläsning och utskrift. Tabellen beskriver dessa fyra arbetsfall på
denna värd; **C/C++-paritet är inte uppnådd**. Särskilt UTF-8 och
arrayloopar behöver fortsatt arbete med kodgenerering och materialisering
av mellanvärden. Linuxprestanda är inte mätt i denna rapport.

## Reproduktion och verifiering

```sh
python3 tools/performance.py /path/to/baseline /path/to/candidate \
  /path/to/empty-report-directory --rounds 30 --c-compiler /usr/bin/clang
```

Verktyget använder Python-standardbiblioteket och ingår inte i Dunders
byggväg. Dunder och de färdiga programmen körs med tom miljö och PATH
utan externa verktyg. Runtime kör en uppvärmning per binär. Mätningen
omfattar 30 försök per variant och arbetsfall: 480 tidsobservationer och
12 runtimeuppvärmningar, samtliga med godkända resultat. Medianen är
medelvärdet av de två mittersta tiderna; p95 är nearest rank, prov 29 av
30. Inga observationer tas bort. Två varianter växlar ordning; med C
roterar alla sex körordningar. Serien kördes utan andra avsiktligt tunga
bygg- eller testjobb. Vanlig skrivbordsaktivitet var inte avstängd.

[samples.csv](samples.csv) innehåller råtider och faktisk arbetsordning.
[summary.csv](summary.csv) innehåller sammanställningen och
[metadata.json](metadata.json) maskinuppgifter, indata, flaggor och
SHA-256. Verktyget kontrollerar vid slutet att kompilatorerna och samtliga
mätta källor är oförändrade. Toppminne, kall OS-cache, inkrementell latens
och tid per kompilatorfas ingår inte i denna mätning.

Korrekthet verifieras separat med `tools/verify-selfhost.sh`: native- och
Wasm-testgrupper, förväntade diagnostikfel, Tabell, CLI/cache,
filsystemkapabiliteter, C-ABI/callbacks, signerade appar och runtimekontrakt.
Nya kontrakt provar kopieringsgränser och ägarskap, typade lokaler och
platser samt operatorprioritet och sidoeffekters ordning. Båda
Linuxkompilatorerna korsbyggdes; de kördes inte på Linux i denna session.
