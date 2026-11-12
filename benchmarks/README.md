# Lokala prestandamätningar

Den aktuella självhostade kompilatorn mäts med
[`tools/performance.py`](../tools/performance.py), som använder Python-standardbiblioteket.
Den jämför två Dunder-kompilatorer med fyra runtimeprogram, ett genererat
byggkorpus, kompilatorns egen källkod och Tabell. Varje binärs resultat verifieras.
Verktyget sparar råtider, median, p95, binärstorlek och SHA-256.

```sh
python3 tools/performance.py /path/to/baseline /path/to/candidate \
  /path/to/empty-report-directory --rounds 30 --c-compiler /usr/bin/clang
```

`--suite runtime` och `--suite compiler` väljer en delmängd. `--include-check`
mäter även `check` på samtliga tre byggkällor. C-referensen
är valfri och byggs med `-O3 -ftrapv`. Kompilatorerna och runtimeprogrammen
körs med tom miljö och en PATH utan externa verktyg. Kör mätningarna utan
andra avsiktligt tunga arbeten på värddatorn.

`--baseline-flag=--scalar-opt=off` med samma nya binär som baseline och
kandidat jämför optimeringslägen utan att byta kompilatorbinär.

[SSA-grund och ABI-kontrakt, 2026-11-12](2026-11-12-ssa-foundation/README.md)
prövar den valbara cell-SSA:n och en separat SSA-optimerad producentbinär.
SSA på ökade kompilatorkorpusets byggmedian 70,92 procent och Tabells
42,34 procent utan samlad runtimevinst mot C. Producentprofilen gav ingen
meningsfull byggvinst med oförändrad användarpolicy. Standard behåller fyra
workers och SSA av. Rapporten innehåller båda 30-rundorsserierna och
korrekthetsunderlaget för SSA samt separata minnes-/utgångs-/ABI-premisser.

[Parallell kompilering, 2026-11-12](2026-11-12-parallel-compiler/README.md)
motiverar valet av fyra workers på ARM64/macOS. Samma binär med fyra workers
ger 36,13 procent kortare genererad byggmedian och 18,89 procent kortare
median för kompilatorkorpuset `selfhost/main.ddr` än med en worker. Den nya
standarden i den mätningen når 163 003 genererade rader/s vid p95. Hela bootstrapkedjans tid
är inte mätt. Runtimebilderna är identiska; C-geomedlet 1,178× lämnar C-målet
öppet. Rapporten innehåller råserier, allokeringsattribuering och verifiering
med den parallella kompilatorn som standard.

[Isolerade uppgifter och seriell byggkostnad, 2026-11-12](2026-11-12-isolated-tasks/README.md)
mäter språkets taskstöd: fyra aktiva jobb ger 3,59×/3,78× parad förbättring
i två rena arbetsfall. Den då seriella kompilatorn behöll
105 975 genererade rader/s vid p95; byggmedianerna ökar 0,01–0,67 procent.
Alla sju vanliga programbilder är identiska i den jämförelsen.

[Helfunktionsoptimering och fristående funktionsobjekt, 2026-11-11](2026-11-11-whole-function-optimization/README.md)
är den tidigare seriella standardprofilen. Den når 107 909 genererade källrader/s
vid p95 med en arbetare, men byggmedianerna kostar fortfarande 2,89×/3,13×
på genererat korpus/självbygge jämfört med före IR. Runtime/C-geomedlet är
1,168×; C-målet återstår. Rapporten behåller mellanresultat och råvärden.

[Gemensam helfunktions-IR, 2026-11-11](2026-11-11-whole-function-mir/README.md)
flyttar hela källspråket genom verifierad IR och sänkning. Första kandidaten
ger identiska bilder men ökar byggmedianerna 55–86 procent; det genererade
provets p95 ger fortfarande 178 000 rader/s. Ett generellt registerpass är
testat separat och ännu inte aktiverat i denna mätning.

[i32-bevis och registerjämförelser, 2026-11-11](2026-11-11-i32-comparisons/README.md)
förbättrar sorteringen ytterligare cirka 23 procent. C-geomedlet är 1,415×;
rapporten redovisar byggkostnad och separat kontroll av varje nytt bevis.

[Gemensam nodarena, 2026-11-11](2026-11-11-typed-nodes/README.md)
förbereder delad uttrycksemission och inlining. Byggkostnaden mäts separat;
utbilderna är byteidentiska och genererat bygges median ökar cirka två procent.

[Intervallbevis och arrayinvarianter, 2026-11-11](2026-11-11-range-proofs/README.md)
förbättrar generell sortering med cirka elva procent. Rapporten visar även
byggkostnad, sämre p95, oberoende modellprov och separata fasobservationer.

[Framefria ARM64-funktioner, 2026-11-11](2026-11-11-frame-free/README.md)
ger ytterligare numerikvinst med verifierade registerflyttar. Högre p95
i vissa fall redovisas tillsammans med median, råtider och av/på-serie.

[Signerad konstantdivision, 2026-11-11](2026-11-11-constant-division/README.md)
förbättrar numerikkärnan med ungefär tolv procent och mäter byggkostnaden
separat. C-målet för hela sviten är fortfarande inte uppnått.

[Verifierade strängägare, 2026-11-11](2026-11-11-ownership-leases/README.md)
visar snabbare självbygge, självkontroll och lexer med validerade
funktionsomskrivningar. Samma kompilator med passet av/på mäter dess kostnad.

[Kompilatordogfood och attribuering, 2026-11-11](2026-11-11-compiler-dogfood/README.md)
är nästa grund: separata fasur och allokeringsräknare, oberoende konsumentfall
och mätning av infrastrukturen med profileringen avstängd.

[Verifierade loopar och elementmutation, 2026-11-11](2026-11-11-verified-mutation/README.md)
är föregående optimeringsetapp: räknade loopar, exklusiv elementmutation, obligatorisk
IR-verifiering och gemensamma språkregler. Sortering och självbygge blir
snabbare; kostnaden på genererad stresskod redovisas uttryckligen.

[Lokalt skalärpass, 2026-11-11](2026-11-11-scalar-optimizer/README.md)
är föregående etapp: typad funktionsmodell, begränsad förenkling,
loopbevis och registerallokering. Rapporten innehåller både jämförelse mot
föregående kompilator och en separat av/på-serie.

[Resultat för självhosten, 2026-11-10](2026-11-10-selfhost-performance/README.md)
innehåller de aktuella mätvillkoren och kvarvarande prestandagap.

[Optimeringsgrund, 2026-11-11](2026-11-11-optimizer-foundation/README.md)
mäter nästa etapp med fasrapportering, kontroll utan lagrad maskinkod,
överförda temporärer, strömmande iteration och konstanta rune-literaler.

## Historiska byggmätningar

Historisk rapport. Kommandon och verktygssökvägar i rapporten avser [deb48dd](https://github.com/Mik-pe/dunder/tree/deb48dd).

Runtime har en separat [före/efter-svit](runtime/README.md) för numerik,
arrayläsning, UTF-8-text och sortering. Den mäter optimerad maskinkod med
samma källor och säkerhetskontroller, med 30 varv per variant.

Mätverktyget är [tools/compile-bench.rs](https://github.com/Mik-pe/dunder/blob/deb48dd/tools/compile-bench.rs), ett separat
Rust/std-program för mätning utanför kompilatorprocessen. Det ingår inte i
Dunders byggväg. Enligt [agentregeln](../AGENTS.md) körs ingen GitHub-CI.

```sh
cargo build --release
rustc --edition=2021 -O tools/compile-bench.rs -o build/compile-bench
./build/compile-bench /path/to/dunder compiler.csv 30
```

Verktyget skapar ett genererat stresskorpus med 21 moduler, 1 001 funktioner,
101 024 fysiska rader med språktoken och 2 216 254 källbyte. Varje hjälpfunktion
anropas av main och bidrar till resultatet. Blankrader och kommentarer ingår
inte. Korpuset har många upprepade heltalsoperationer och representerar inte
hela språket eller kompilatorns egen källkod.

Varje försök startar en ny `dunder build`-process, läser alla moduler, gör
typkontroll, kodgenerering och signering samt publicerar en ny utdatafil.
Klockan stannar när processen har avslutats. Binären körs därefter med tom
miljö och måste skriva `595500`; felaktigt resultat underkänner försöket.
Körningen ingår inte i byggtiden. Kompilatorns PATH saknar externa verktyg.
Det finns ingen Dunder-cache, OS-filcachen är varm och en arbetare används.

Rapportera råvärden, median, nearest-rank p95, compiler/source revision,
binärchecksumma, maskin, OS och byggflaggor tillsammans. Även första försöket
ingår i fördelningen. Detta prov visar genomströmning för genererad stresskod.
Det uppfyller inte ensamt [hela prestandakontraktet](../docs/prestanda.md):
representativa moduler med samlingar/lån, C/C++-jämförelse, kall OS-cache,
inkrementella arbetsfall och ett verkligt självhostat kompilatorbygge återstår.

## Resultat 2026-11-10

| Mått | Resultat |
| --- | --- |
| Maskin | Apple M5, 24 GiB, arm64, macOS 27.2 (26B123). |
| Kompilator | `90d232ab810367d235ed3d0c6252770d96adaf54`, Rust-bootstrap byggd med `cargo build --release`. |
| Indata | 101 024 kodrader, 2 216 254 byte, 508 056 token, 21 moduler, 1 001 funktioner. |
| Försök | 30 nya processer och utdatafiler; en arbetare, ingen Dunder-cache, varm OS-cache. |
| Median | 55,166 ms, utifrån de två mittersta observationerna. |
| p95 | 59,026 ms, nearest rank. |
| Genomströmning vid p95 | Cirka 1,71 miljoner genererade kodrader/s. |
| Binärstorlek | 3 201 248 byte. |
| Körverifiering | Samtliga 30 binärer skrev exakt `595500` och avslutades korrekt. |

[Råvärden för alla försök](2026-11-10-compiler.csv) och
[metadata, revisioner och checksummor](2026-11-10-compiler.json) följer med.
Toppminne mättes inte. Den ursprungliga backendens korta hopp kunde inte
bygga detta korpus; långa hopp/adresser fungerar nu. Optimeringarna omfattar
kompaktare stackåtkomster, ASCII-parsning utan teckenindextabell och borttagning
av onåbar kod. Detta prov isolerar inte varje optimerings enskilda bidrag.

Som separat verkligt program byggdes `selfhost/main.ddr` med tolv moduler.
En enskild observation från kompilatorns interna timer gav 1,867 ms
läsning/parsning, 1,261 ms typkontroll, 0,870 ms kodgenerering/signering och
8,818 ms totalt inklusive publicering. Den siffran inkluderar inte
processstart och är inte en 30-försöksmätning eller ett självhostat bygge.
Den byggda Dunder-skrivna kompilatorn byggde därefter hello och fibonacci med
PATH utan externa verktyg; båda körresultaten och macOS-signaturerna godkändes.
