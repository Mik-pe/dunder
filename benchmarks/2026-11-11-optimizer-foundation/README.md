# Optimeringsgrund, 2026-11-11

Första etappen lägger in monotona fastider, kontroll utan lagrad maskinkod,
överföring av ägda temporärer, direkt strömmande byte-/runeiteration och
rune-literaler som skalära konstanter. Standardprofilens overflow-, index-,
UTF-8-, snapshot- och ägarskapsregler behålls.

Mätt på Apple M5, 24 GiB RAM, ARM64/macOS 27.2. Baseline är den självbyggda
kompilatorn från `2f462d1`. Kandidatens kodrevision är `f91b34b`; efterföljande
rapport- och dokumentationsändringar ändrar inte mätta kompilatorkällor.
De exakta käll- och binärhasharna finns i [metadata.json](metadata.json).
Kandidaten nådde byteidentiska självbyggen från det frysta fröet. En ren
Git-archive av kodrevisionen byggde samma kompilator byte för byte.

## Runtime

| Arbetsfall | Median före → efter | p95 före → efter | Median C `-O3 -ftrapv` |
| --- | ---: | ---: | ---: |
| Numerik | 9,148 → 9,160 ms | 12,074 → 10,857 ms | 4,738 ms |
| Arrayläsning | 10,543 → 10,584 ms | 12,022 → 12,725 ms | 2,484 ms |
| UTF-8-text | **29,091 → 3,788 ms** | **32,252 → 4,108 ms** | 2,664 ms |
| Sortering | 7,845 → 7,838 ms | 11,264 → 9,519 ms | 2,409 ms |

UTF-8-provets mediantid minskade **87,0 %**, motsvarande cirka 7,68× genomströmning
för samma arbete. Kandidatens median är 1,42× C-referensens tid. De övriga
runtimeprovens medianer ändrades mindre än en procent; där visar serien
ingen tydlig förbättring. Den styrker inte generell C/C++-paritet.

De fyra källorna och [C-referensen](../runtime/reference.c) är oförändrade.
Varvantal, frö och text kommer från programargument. Python beräknar
oberoende förväntade resultat; varje körning måste ge exakt rätt kontrollsumma.
C använder Apple Clang 21 med `-std=c11 -O3 -ftrapv` och saknar Dunders
arraygränskontroller. Väggtiderna inkluderar processstart, argument och utskrift.

## Bygge och kontroll

| Arbetsfall | Median före → efter | p95 före → efter |
| --- | ---: | ---: |
| Genererat bygge | 402,930 → 399,417 ms | 408,156 → 403,280 ms |
| Självbygge | 170,945 → 170,639 ms | 173,010 → 173,555 ms |
| Tabell-bygge | 11,069 → 11,143 ms | 12,500 → 12,303 ms |
| Genererat `check` | 289,278 → 287,172 ms | 291,531 → 289,247 ms |
| Självhostens `check` | 122,913 → 122,390 ms | 124,683 → 123,394 ms |
| Tabells `check` | 6,106 → 5,998 ms | 6,904 → 7,499 ms |

Medianerna ligger nära baseline. De tre byggproven ändrades mindre än en
procent; de tre kontrollproven mindre än två procent. Vi gör inget anspråk
på en tydlig generell bygg- eller kontrollvinst från dessa skillnader.
Tabells korta kontrollprov har högre p95 trots lägre median; alla observationer
är med och korta processer påverkas tydligt av start och schemaläggning.

Stresskorpuset är samma 101 024 tokenbärande rader, 2 216 254 byte,
508 056 token, 21 moduler och 1 001 funktioner som tidigare. Genomströmningen
vid p95 är 247 513 → 250 506 rader/s. Det är upprepad skalär stresskod och
styrker inte ensamt hela språkets prestandakontrakt.

Varje bygge startar en ny kompilatorprocess, använder ingen inkrementell
cache och skriver en ny färdig signerad Mach-O-fil. OS-filcachen är varm;
en arbetare används. Varje stressbinär måste skriva `595500`. Varje
självbyggd kompilator måste bygga en fungerande hello-binär. Varje Tabell-binär
kör CSV-provet och jämförs med projektets förväntade fil. Resultatkontrollen
sker efter klockstopp. Samma variant måste ge byteidentiska byggresultat
vid samtliga 30 försök. Båda kompilatorerna bygger samma slutliga källor,
även i självprovet. `check` verifieras genom lyckad process och förväntad
slutrapport; semantiska felutfall provas i den separata korrekthetssviten.

## Varför rune-literaler blev nästa optimering

Den första integrerade versionen, kodrevision `b02f027`, hade fungerande
strömmande iteration men visade ingen tydlig runtimevinst: UTF-8-medianen
var 28,632 → 28,856 ms. Dessa 30-varvsdata är bevarade i
[initial-samples.csv](initial-samples.csv), [initial-summary.csv](initial-summary.csv)
och [initial-metadata.json](initial-metadata.json).

Inspektion visade att varje rune-literal i loopkroppens jämförelser gick
genom strängkonvertering och en temporär rune-array under körning.
Strömning av själva loopkällan tog inte bort den kostnaden. Kandidaten
avkodar nu den lexikalt validerade runen vid kompilering och emitterar ett
skalärt immediate-värde. En strukturell regression jämför de genererade
byten med motsvarande heltalsjämförelser på samtliga fyra mål.

Slutseriens runtimevinst gäller den samlade etappen. Den jämför inte
strömning och literaländring var för sig och isolerar inte varje delpass.

## Reproduktion, underlag och gränser

```sh
python3 tools/performance.py /path/to/baseline /path/to/candidate \
  /path/to/empty-report-directory --rounds 30 --include-check \
  --c-compiler /usr/bin/clang
```

Slutserien omfattar 720 tidsobservationer och 12 runtimeuppvärmningar.
Alla resultatkontroller godkändes. Varje variant och arbetsfall har 30
observationer. Medianen är medelvärdet av de två mittersta tiderna; p95
är nearest rank, observation 29 av 30. Två varianter växlar ordning; med
C roterar alla sex ordningar. Första bygg-/kontrollprovet ingår; inga
observationer tas bort. Den preliminära serien omfattar ytterligare
720 observationer och 12 uppvärmningar, också med godkända resultat.

Dunder och dess program körs med tom miljö och PATH utan externa verktyg.
Mätserierna kördes utan andra avsiktligt tunga bygg- eller testjobb.
Vanlig skrivbordsaktivitet var inte avstängd. Källor och kompilatorbinärer
kontrollerades som oförändrade efter varje serie. [samples.csv](samples.csv),
[summary.csv](summary.csv) och [metadata.json](metadata.json) innehåller
råtider, faktisk körordning, maskin, indata, flaggor och SHA-256.

Slutkoden klarade `tools/verify-selfhost.sh`: 63 native-grupper, 57 Wasm-grupper
och sex uttryckligt native-only kompilator-/filsystemgrupper, 45 diagnostikfall,
56 kontroll-/byggpar över alla mål, 14 Tabell-grupper och 44 runtimekontrakt
vardera på native och Wasm. Även CLI/cache/timings, filkapabiliteter,
C-ABI/callbacks, 25 negativa FFI-fall, appsignering, Wasm-skalärer och
WebGPU-ABI passerade. Rune-literalernas egna kontrakt körs även på Wasm.
Båda Linuxkompilatorerna korsbyggdes; Linuxkörning och Linuxprestanda
verifierades inte i denna etapp.

`--timings[=json]` är valbar separat instrumentering och användes inte i
väggtidsserien. `check` behåller inga maskinord/relokeringar, men går ännu
genom sänkningshjälparna. En typad funktionsmodell, full registerallokering,
direkta x86-/Wasm-backends, finare beroendecache och mer avancerade
loopoptimeringar återstår. Toppminne för kompilatorn, kall OS-cache,
inkrementell latens och instrumenteringens påslagna overhead mättes inte här.
