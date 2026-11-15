# Verifierade loopar och elementmutation, 2026-11-11

Denna etapp breddar den typade funktionsmodellen med räknade `for`-loopar
och elementskrivning genom exklusiva `&mut []i64`-parametrar. Den inför
också obligatorisk IR-verifiering före och efter passet, gemensamma
källspråksregler och konsekvent lexikal namnupplösning i kompilatorns
direkta API. Ingen algoritm väljs genom funktionsnamn eller modul.
[Mekanismer, gränser och nästa steg](../../docs/optimization.md).

Baseline är den självbyggda kompilatorn från `f4a1fac`. Slutkandidatens
kodrevision är `5360693`, SHA-256
`ef3ead075d0e1b1e70826fcb64c3b31c39d2adacbd482b8d98a3d924a9f2b4ba`.
Mätmaskin: Apple M5, 24 GiB RAM, ARM64/macOS 27.2 (26B123).
En ren Git-archive av kodrevisionen byggde samma kompilator byte för byte
från det frysta fröet, med identiska tredje och fjärde byggsteg.

## Runtime mot föregående kompilator

| Arbetsfall | Median före → efter | p95 före → efter | Median C | Dunder/C median |
| --- | ---: | ---: | ---: | ---: |
| Numerik | 9,262 → 9,300 ms | 9,921 → 9,502 ms | 5,204 ms | 1,79× |
| Arrayläsning | 3,455 → 3,404 ms | 3,891 → 4,425 ms | 2,760 ms | 1,23× |
| UTF-8-text | 4,229 → 4,220 ms | 4,510 → 4,545 ms | 2,937 ms | 1,44× |
| Sortering | **8,520 → 5,727 ms** | **8,755 → 5,887 ms** | 2,713 ms | 2,11× |

Sorteringens mediantid minskar **32,8 %**, motsvarande cirka **1,49×**
genomströmning. Dess vanliga Dunder-kropp använder nu den generella
modellen: en fångad range-gräns, registerhållna lokaler och explicit
ordnade elementskrivningar. Index- och overflow-kontroller finns kvar.
De övriga medianerna ligger inom cirka 1,5 % från baseline; där visar
serien ingen tydlig förbättring. Arrayläsningens p95 ökar trots liknande
median. Alla observationer behålls.

Geometriskt medel för de fyra Dunder/C-kvoterna går från **1,78× till
1,61×** i samma serie. Målet 1,10× i medel och högst 1,25× per arbetsfall
är inte uppfyllt. Det finns inget anspråk på generell C-paritet.
C använder Apple Clang 21 med `-std=c11 -O3 -ftrapv` och saknar Dunders
arraygränskontroller. Algoritmer, indata och kontrollsummor är oförändrade.
Tiderna inkluderar processstart, argument och utskrift.

## Bygge och kontroll mot föregående kompilator

| Arbetsfall | Median före → efter | p95 före → efter |
| --- | ---: | ---: |
| Genererat bygge | **315,697 → 362,829 ms** | **320,826 → 370,728 ms** |
| Självbygge | **213,868 → 182,619 ms** | **217,228 → 186,436 ms** |
| Tabell-bygge | 11,512 → 11,259 ms | 12,144 → 11,627 ms |
| Genererat `check` | **202,632 → 227,123 ms** | **203,705 → 228,213 ms** |
| Självhostens `check` | **156,646 → 125,222 ms** | **163,010 → 131,272 ms** |
| Tabells `check` | 6,208 → 6,011 ms | 7,185 → 6,708 ms |

Självbyggets median minskar **14,6 %**, kontrollens **20,1 %**. Tabells
medianer minskar cirka 2–3 %. Jämförelsen omfattar hela kompilatorändringen,
inklusive gemensamma språkregler och lexikal namnupplösning. Den isolerar
inte dessa delars bidrag.

Det genererade stressbygget blir samtidigt **14,9 % långsammare** och dess
kontroll **12,1 % långsammare**. Den nya verifieringen körs före och efter
passet och tar särskilt mycket plats i ett korpus som nästan helt använder
modellen. Kostnaden är kvar; rapporten gör inget löfte om oförändrad
byggtid. Inga obligatoriska kontroller har stängts av för mätningen.

Stresskorpuset är 101 024 tokenbärande rader, 2 216 254 byte,
508 056 token, 21 moduler och 1 001 funktioner. Vid p95 ger det
314 887 → **272 502 rader/s**, fortsatt över målet 100 000 rader/s för
detta prov. Båda varianternas binärer är 2 446 112 byte. Den upprepade
skalära stresskoden representerar inte hela språket. Båda kompilatorerna
bygger samma slutliga källor, även i självprovet.

## Samma binär, modellen av och på

| Arbetsfall | Median av → på | p95 av → på |
| --- | ---: | ---: |
| Numerik | 9,746 → 9,074 ms | 10,751 → 10,266 ms |
| Arrayläsning | 9,687 → 3,460 ms | 11,905 → 4,136 ms |
| UTF-8-text | 4,360 → 4,264 ms | 4,767 → 4,664 ms |
| Sortering | **8,657 → 5,840 ms** | **9,199 → 6,269 ms** |
| Genererat bygge | 409,047 → 367,515 ms | **446,666 → 502,624 ms** |
| Självbygge | 183,126 → 183,501 ms | 194,101 → 195,753 ms |
| Tabell-bygge | 8,369 → 8,377 ms | 10,310 → 10,636 ms |
| Genererat `check` | 288,745 → 219,623 ms | 296,212 → 233,271 ms |
| Självhostens `check` | 128,279 → 128,459 ms | 131,613 → 130,880 ms |
| Tabells `check` | 6,119 → 6,131 ms | 6,462 → 6,533 ms |

`--scalar-opt=off` stänger av hela den aktuella modellen och dess pass,
även optimeringar som redan fanns i baseline. Arrayläsningens av/på-vinst
är därför inte ny för denna etapp. Sorteringens median minskar 32,5 %.
Självbygge och Tabell visar ingen tydlig medianvinst av själva lägesbytet;
andra frontendändringar finns i båda lägena.

Det genererade byggets median är lägre med modellen, men p95 är högre
i denna serie. p95-genomströmningen är fortfarande 200 993 rader/s.
Resultatet stöder inte ett generellt löfte om bättre bygg-p95. Vanlig
skrivbordsaktivitet var inte avstängd; inga observationer har valts bort.

## Varför arbetsminne och literalhantering ändrades

Den första integrerade kandidaten, `e578033`, hade fungerande mutation
och verifiering. Dess 30-varvsserie visade sortering 8,480 → 5,687 ms och
självbygge 222,710 → 186,739 ms, men genererat bygge 328,054 → 383,751 ms
(+17,0 %). Hela serien finns kvar i [initial](initial/summary.csv).

Två konkreta allokeringskostnader togs bort:

- Verifierarens flaggor och scopestack återanvänds mellan funktioner och
  mellan kontrollfaser. Använda delar initieras på nytt, även efter fel.
- Den gemensamma heltalsbreddskontrollen validerar decimalparserns
  befintliga resultat. Den konstruerar ingen andra ägd resultatpost.

Alla form-, typ-, effekt-, scope- och bevisplaceringskontroller finns kvar.
Slutserien visar fortfarande en kostnad på stresskorpuset. Serierna
isolerar inte de två allokeringsändringarnas enskilda effekt. De fyra
Dunder-runtimebinärernas hash och storlek är exakt oförändrade mellan
den första och slutliga kandidaten; korrigeringarna ändrar kompilatorns
arbete, inte dessa programs maskinkod.

## Verifiering och reproduktion

Slutkoden klarade `tools/verify-selfhost.sh`: **83 native-grupper**, **77
Wasm-grupper** och sex uttryckligt native-only grupper, 45 diagnostikfall,
56 kontroll-/byggpar över alla mål, 14 Tabell-grupper och 45 runtimekontrakt
vardera på native och Wasm. CLI/cache/timings, filkapabiliteter,
C-ABI/callbacks, 25 negativa FFI-fall, appsignering, Wasm-skalärer och
WebGPU-ABI passerade också.

Konsumentproven jämför slutkandidaten med både föregående kompilator och
avstängd modell: **360 native/Wasm-körningar**, **360 Linux-korsbyggen**
och **1 764 matchande feldiagnostiker**. Bindningsposter från `check
--agent` matchar. Modellägesräknare visar att de nya generella funktionerna
faktiskt emitteras genom modellen. Fullverifieringens eget av/på-prov
omfattar 240 körningar, 240 Linux-korsbyggen och 1 176 diagnostiker.

Emitterarsviten körde **49 fall på både native och Wasm** och byggde alla
fyra mål. Den täcker bland annat destination före RHS, samma-array-läsning,
flera rötter med olika längder, tomma arrayer, extrema range-gränser,
nästlat kontrollflöde, fångade gränser, skuggning och spill. IR-proven
avvisar felaktiga modeller och jämför färskt respektive återanvänt
arbetsminne över växlande modellstorlekar och felutfall.

Ett isolerat subagentprov med den första kandidatens testrunner rapporterade
en lexergrupp med exit 126. Den separat byggda gruppen godkändes och
slutkodens fullverifiering körde samma grupp med godkänt resultat. Orsaken
till det isolerade start-/runnerutfallet är inte fastställd; det redovisas
inte som ett reparerat lexerfel.

Båda Linuxkompilatorerna korsbyggdes. Linuxkörning och Linuxprestanda
verifierades inte här. x86-64 och Wasm sänker fortfarande den gemensamma
ARM-instruktionsvokabulären; fysisk x86-registerallokering återstår.

```sh
python3 tools/performance.py /path/to/f4a1fac /path/to/5360693 \
  /path/to/empty-report --rounds 30 --include-check --c-compiler /usr/bin/clang

python3 tools/performance.py /path/to/5360693 /path/to/5360693 \
  /path/to/empty-ablation --rounds 30 --include-check \
  --baseline-flag=--scalar-opt=off
```

Varje serie använder 30 observationer per variant och arbetsfall, nya
processer, en arbetare, varm OS-filcache och avstängd inkrementell cache.
Klockan omfattar hela bygget till signerad publicerad fil. Stressbinärer
måste skriva `595500`, varje självbyggd kompilator måste bygga hello och
varje Tabell-binär måste köra CSV-provet. Dessa byggresultat kontrolleras
efter klockstopp. Runtime har oberoende beräknade kontrollsummor. Varje
variants byggresultat är byteidentiskt över samtliga 30 försök.

Två varianter växlar ordning; med C roteras alla sex permutationer.
Medianen använder de två mittersta tiderna; p95 är nearest rank, observation
29 av 30. Slutserien har 720 observationer och 12 runtimeuppvärmningar,
av/på-serien 600 och åtta, den första serien ytterligare 720 och tolv.
Inga observationer har tagits bort. Instrumenterade fastider var av.

Dunder och dess program kör med tom miljö och PATH utan externa verktyg.
Serierna kördes utan andra avsiktligt tunga bygg- eller testjobb. Käll- och
kompilatorhashar kontrollerades som oförändrade efter varje serie.
Toppminne, kall OS-cache, inkrementell latens och instrumenteringens
påslagna overhead mättes inte.

[Slutserie: råtider](final/samples.csv), [sammanfattning](final/summary.csv),
[metadata](final/metadata.json).
[Av/på: råtider](ablation/samples.csv), [sammanfattning](ablation/summary.csv),
[metadata](ablation/metadata.json).
[Första serie: råtider](initial/samples.csv), [sammanfattning](initial/summary.csv),
[metadata](initial/metadata.json).
