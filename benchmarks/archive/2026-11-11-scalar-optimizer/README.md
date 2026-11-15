# Lokalt skalärpass, 2026-11-11

Standardkompilatorn använder nu en begränsad typad funktionsmodell,
konstantförenkling, konservativa loopbevis och registerallokering.
Anropsfria skalära funktioner och läsning av delade `[]i64`-parametrar
omfattas. Anrop, muterbara arrayer, ägda värden och flyttal använder
fortsatt den allmänna kodgeneratorn. [Implementation och nästa etapper](../../docs/optimization.md).

Två serier med 30 försök per variant och arbetsfall kördes separat:

- **Före/efter:** självbyggd baseline `c6d3b15` mot kodrevision
  `8aa924f`. Fyra runtimeprov jämförs även med C `-O3 -ftrapv`.
- **Av/på:** samma nya kompilatorbinär med `--scalar-opt=off` respektive
  standardläget. Denna serie isolerar den nya modellens samlade effekt,
  inklusive parser, pass, registerallokering och emission.

Mätmaskinen är Apple M5, 24 GiB RAM, ARM64/macOS 27.2.
Kandidatens SHA-256 är
`c7c7b230d867e37ec7c4a7491204094c2ed51e63a5b593e60498fa1112d18df6`.
En ren Git-archive av `8aa924f` byggde samma kompilator byte för byte
från det frysta fröet, med byteidentiska tredje och fjärde byggsteg.
Kompilatorkällor och binärer ändrades inte under mätserierna.

## Runtime mot föregående kompilator

| Arbetsfall | Median före → efter | p95 före → efter | Median C | Dunder/C median |
| --- | ---: | ---: | ---: | ---: |
| Numerik | 9,039 → 8,478 ms | 13,243 → 12,382 ms | 4,702 ms | 1,80× |
| Arrayläsning | **10,643 → 3,070 ms** | **15,807 → 4,040 ms** | 2,493 ms | **1,23×** |
| UTF-8-text | 3,787 → 3,778 ms | 7,483 → 5,924 ms | 2,638 ms | 1,43× |
| Sortering | 7,968 → 7,982 ms | 10,424 → 9,268 ms | 2,385 ms | 3,35× |

Arrayprovets mediantid minskar 71,2 %, motsvarande 3,47× genomströmning.
Den generella regeln optimerar `array.sum` genom dess kropp: ackumulator,
index och arraydata/längd behålls i register; index och ökning bevisas
säkra. Summeringens overflow-kontroll finns kvar. Ingen funktion väljs
genom namn eller modul.

Numerikens median minskar 6,2 %. Den lilla anropsfria `step` omfattas,
medan dess anropande huvudloop använder den allmänna vägen. UTF-8 och
sortering visar ingen tydlig medianvinst från denna etapp. Den tidigare
UTF-8-optimeringen finns redan i baseline.

Arrayprovet når den föreslagna enskilda gränsen 1,25× C i denna serie.
Geometriskt medel för de fyra proven är fortfarande **1,81× C**, över
målet 1,10×. Detta är inte generell C-paritet. C saknar Dunders
arraygränskontroller; referensen använder Apple Clang 21,
`-std=c11 -O3 -ftrapv`. Algoritmer, indata och kontrollsummor är oförändrade.
Tiderna inkluderar processstart, argument och utskrift.

## Bygge och kontroll mot föregående kompilator

| Arbetsfall | Median före → efter | p95 före → efter |
| --- | ---: | ---: |
| Genererat bygge | **403,799 → 305,756 ms** | **408,672 → 313,609 ms** |
| Självbygge | 199,884 → 199,948 ms | 202,702 → 204,083 ms |
| Tabell-bygge | 10,691 → 10,825 ms | 11,569 → 14,783 ms |
| Genererat `check` | **289,240 → 191,789 ms** | **298,016 → 199,175 ms** |
| Självhostens `check` | 146,049 → 145,696 ms | 149,589 → 149,463 ms |
| Tabells `check` | 5,873 → 5,919 ms | 6,797 → 7,137 ms |

Det genererade byggets median minskar 24,3 % och kontrollens 33,7 %.
Stresskorpuset är 101 024 tokenbärande rader, 2 216 254 byte,
508 056 token, 21 moduler och 1 001 funktioner. Genomströmning vid p95
ökar från 247 201 till 322 134 rader/s. Binären minskar från
3 217 664 till 2 446 112 byte. Den upprepade skalära stresskoden passar
den nya modellen och representerar inte hela språket.

Självbyggets och Tabells medianer ligger inom cirka 1,3 % från baseline;
vi gör inget anspråk på tydlig förbättring där. Tabells p95-bygge ökar
27,8 % i denna serie och dess kontroll-p95 5,0 %. Motsvarande ökning i
bygg-p95 återkommer inte i av/på-serien nedan. Råvärdena behålls i båda
serierna; resultatet motiverar inget generellt löfte om oförändrad p95.

Båda kompilatorerna bygger samma slutliga källor, även självhosten.
Självbyggtider från äldre rapporter avser mindre källversioner och kan
inte jämföras direkt med dessa tider.

## Samma binär, optimering av och på

| Arbetsfall | Median av → på | p95 av → på |
| --- | ---: | ---: |
| Numerik | 9,054 → 8,360 ms | 11,094 → 10,087 ms |
| Arrayläsning | **10,767 → 3,115 ms** | **12,618 → 3,431 ms** |
| UTF-8-text | 3,834 → 3,813 ms | 4,239 → 4,204 ms |
| Sortering | 8,026 → 8,074 ms | 8,846 → 8,321 ms |
| Genererat bygge | **407,785 → 306,758 ms** | **414,685 → 313,818 ms** |
| Självbygge | 201,304 → 200,286 ms | 207,402 → 205,460 ms |
| Tabell-bygge | 10,795 → 10,886 ms | 12,003 → 11,459 ms |
| Genererat `check` | **293,405 → 193,772 ms** | **297,798 → 197,062 ms** |
| Självhostens `check` | 146,810 → 147,406 ms | 152,104 → 152,058 ms |
| Tabells `check` | 6,042 → 6,046 ms | 7,109 → 7,359 ms |

Av/på-provet bekräftar den samlade optimeringsvägens vinst: 3,46×
genomströmning för arrayprovet, 24,8 % kortare genererat bygge och
34,0 % kortare genererad kontroll. Den isolerar inte varje internt pass.
Tidigare kodgenereringsoptimeringar är aktiva även med skalärpasset av.

## Metod, korrekthet och reproduktion

Före/efter-serien omfattar 720 tidsobservationer och 12
runtimeuppvärmningar; av/på-serien 600 observationer och åtta uppvärmningar.
Varje arbetsfall och variant har 30 observationer. Två varianter växlar
ordning; med C roteras alla sex permutationer. Medianen använder de två
mittersta tiderna och p95 är nearest rank, observation 29 av 30.
Inga observationer tas bort.

Varje bygge använder en ny process och en ny färdig signerad fil, med
en arbetare, avstängd inkrementell cache och varm OS-filcache. Varje
runtimekörning måste ge en oberoende beräknad kontrollsumma. Stressbinärer
måste skriva `595500`; varje självbyggd kompilator bygger hello och varje
Tabell-binär kör projektets CSV-prov. Dessa byggresultatkontroller sker
efter klockstopp. Varje variants byggresultat är byteidentiskt över samtliga
30 försök. `check` måste lyckas och ge rätt slutrapport.

Dunder och dess program kör med tom miljö och PATH utan externa verktyg.
Serierna kördes utan andra avsiktligt tunga bygg- eller testjobb; vanlig
skrivbordsaktivitet var inte avstängd. Valbar tidsinstrumentering var av.

```sh
python3 tools/performance.py /path/to/c6d3b15 /path/to/8aa924f \
  /path/to/empty-report --rounds 30 --include-check --c-compiler /usr/bin/clang

python3 tools/performance.py /path/to/8aa924f /path/to/8aa924f \
  /path/to/empty-ablation --rounds 30 --include-check \
  --baseline-flag=--scalar-opt=off
```

Slutkoden klarade `tools/verify-selfhost.sh`: 72 native-grupper,
66 Wasm-grupper och sex uttryckligt native-only grupper, 45 diagnostikfall,
56 kontroll-/byggpar över alla mål, 14 Tabell-grupper och 45 runtimekontrakt
vardera på native och Wasm. CLI/cache/timings, filkapabiliteter,
C-ABI/callbacks, 25 negativa FFI-fall, appsignering, Wasm-skalärer och
WebGPU-ABI passerade också.

Optimerarens konsumentprov passerade separat mot den äldre baseline och
avstängt pass: 156 native/Wasm-körningar, 156 Linux-korsbyggen och
1 008 matchande feldiagnostiker. Fullverifieringens av/på-prov omfattar
104 körningar, 104 Linux-korsbyggen och 672 matchande feldiagnostiker.
Den separata emitterarsviten körde 24 fall på både native och Wasm och
byggde alla fyra mål, inklusive fällor, tomma arrayer, argumentöverlappning,
kortslutning och 40 samtidigt levande lokaler som tvingar spill.

Linuxkörning och Linuxprestanda verifierades inte här. x86-64 och Wasm
sänker fortfarande ARM-instruktionsvokabulären; de nya registerhemmen
innebär inte fysisk x86-registerallokering. Toppminne, kall OS-cache,
inkrementell latens och instrumenteringens påslagna overhead mättes inte.

[Före/efter: råtider](samples.csv), [sammanfattning](summary.csv),
[metadata och källhashar](metadata.json).
[Av/på: råtider](ablation/samples.csv), [sammanfattning](ablation/summary.csv),
[metadata och flaggor](ablation/metadata.json).
