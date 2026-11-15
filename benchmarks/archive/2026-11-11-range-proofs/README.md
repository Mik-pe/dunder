# Intervallbevis och explicita arraygränser

Sorteringens median förbättras 10,7 %, från 5,10 till 4,55 ms. Samma
kompilator med intervallpasset av/på ger 9,7 %. C-geomedlet är 1,507×,
med sorteringens 1,933× som största gap. Målen 1,10× och högst 1,25×
per fall är fortfarande inte uppnådda.

## Mekanism och identitet

En begränsad intervallanalys följer i32-intervall och relationer till
exakta arrayrötters längder. Looparnas tillstånd når först en konservativ
fixpunkt; en separat återspelning producerar bevis. Analysen skriver inga
bevis vid budgetfel. Den obligatoriska IR-verifieringen körs före och efter
optimeringen. Endast bevisade arraygränskontroller tas bort; indexutvärdering,
overflow, fällordning och sparad destinationsadress behålls.

Analysen har budget för 131 072 arbetsenheter, åtta överföringar per
loophuvud, djup 64, 32 768 faktaceller och 512 resultatplatser. En återanvänd
arbetsyta med plana buffertar undviker en heapallokering per uttryck.
`--range-opt=off` isolerar passet, med separat cache- och kvittoidentitet.
[Kontrakt](../../docs/optimization.md#budgeterade-intervall-och-stabila-arraylängder).

Runtime upprätthåller nu uttryckligen `0 <= len <= cap <= i32::MAX`.
Tillräcklig befintlig kapacitet är en snabb väg; tillväxt kontrollerar den
begärda längden före allokering eller publicering och begränsar dubbleringen.
Detta ersätter ett tidigare beroende av heapens praktiska storlek.
[Minneskontrakt](../../docs/memory.md).

Kompilatorkoden är `90f303d`, jämförd med `e11759f`. Kandidatens SHA-256 är
`57c860cd4a1f0e56e5e7b99a3f50c15d613bac79adad50e526f20bd266bd975f`.
Varje serie har 30 färska processer per variant, växlande ordning, varm
OS-cache och ingen Dunder-cache på Apple M5. Båda kompilatorerna bygger
samma slutliga källor; varje resultat verifieras. Profilering är avstängd.

## Runtime och bygglatens

Median / nearest-rank p95 i millisekunder. C använder clang `-O3 -ftrapv`
och saknar Dunders arraygränskontroller.

| Arbete | Före | Efter | C | Efter / C, median |
| --- | --- | --- | --- | --- |
| Numerik | 6,60 / 11,20 | 6,60 / 11,32 | 4,58 / 6,23 | 1,443× |
| Arrayläsning | 3,04 / 8,01 | 3,04 / 5,30 | 2,39 / 4,64 | 1,271× |
| UTF-8-text | 3,73 / 7,28 | 3,76 / 5,96 | 2,59 / 4,21 | 1,455× |
| Sortering | 5,10 / 8,87 | 4,55 / 6,56 | 2,35 / 3,74 | 1,933× |

Av/på-serien ger sortering 5,02 / 7,01 ms av och 4,54 / 6,70 ms på.
De generella sorterings- och reverse-funktionerna behöver inga källändringar:
de får fem respektive fyra nya gränsbevis.

| Byggarbete | Före median / p95 | Efter median / p95 |
| --- | --- | --- |
| Genererat bygge | 309,24 / 316,64 | 313,95 / 326,02 |
| Genererad kontroll | 178,18 / 180,41 | 178,88 / 180,79 |
| Självbygge | 170,78 / 173,02 | 174,15 / 175,58 |
| Självkontroll | 108,20 / 108,90 | 109,62 / 110,39 |
| Tabellbygge | 10,76 / 12,51 | 10,90 / 14,17 |
| Tabellkontroll | 4,89 / 5,91 | 4,92 / 6,58 |

Hela ändringen kostar 1,5 % i genererad byggmedian och 2,0 % i självbygge.
Tabellbyggets p95 ökar 13,3 % och kontrollens 11,3 %. Dessa försämringar
ingår i bedömningen. Samma binär med passet av/på ger däremot cirka
0,03 % medianpåslag för genererat bygge och 0,02 % för självbygge.
Byggbildernas storlekar är oförändrade: 2 413 280, 1 526 816 och 49 376 byte.
Genererad genomströmning vid p95 är cirka 309 900 rader/s.
[Råtider](final/samples.csv), [sammanfattning](final/summary.csv),
[metadata](final/metadata.json), [av/på-råtider](ablation/samples.csv)
och [av/på-metadata](ablation/metadata.json).

De första serierna med kapacitetskontrollen före snabbvägen bevaras i
[initial](initial/summary.csv) och [initial-ablation](initial-ablation/summary.csv).
Den slutliga ordningen kontrollerar först om den giltiga befintliga kapaciteten
räcker. Vi tillskriver inte denna omordning någon isolerat uppmätt tidsvinst.

## Dogfood, attribuering och korrekthet

Oförändrad arbetsmängdsrevision 2 används. Medianerna före/efter är POD
47,56 / 47,46 ms, decimal 70,43 / 70,35 ms, lexer 87,68 / 87,48 ms och
symboltabeller 98,99 / 99,02 ms. Av/på-medianerna skiljer mindre än 0,7 %
för samtliga fyra fall. De saknar C-referens och ingår inte i C-geomedlet.
[Råtider](dogfood/samples.csv), [metadata](dogfood/metadata.json),
[av/på-resultat](dogfood-ablation/summary.csv).

Separata femomgångsobservationer med fasur respektive allokeringsräknare
verifierar samma utbilder. Intervallfasens median är 0,069 ms vid bygget av
det verkliga sorteringsfallet. Genererat bygge, självbygge och Tabell saknar
aktiverad intervallanalys i denna revision och redovisar noll där. Självbyggets
fulla frontend är fortfarande den största emitterande fasen, 61,3 ms i denna
instrumenterade observation. Instrumenterade tider används inte som vanlig
bygglatens. [Kompilatorattribuering](compiler-attribution/summary.json),
[sorteringsattribuering](sort-attribution/summary.json), med råloggar och
proveniens i respektive katalog.

Full `tools/verify-selfhost.sh` passerade efter den slutliga snabbvägsändringen:
114 nativa grupper, 106 Wasm-grupper, åtta explicit nativa grupper,
50 diagnostikfall, 46 runtimekontrakt per native/Wasm och 13 CLI-grupper
på macOS. Bootstrap nådde byteidentisk fixpunkt.

Den oberoende modellen omfattar 55 källfall, rå/förenklad IR och budgetarna
0/64/131072: 697 400 fullföljda körningar, 529 430 arrayåtkomster och
124 nådda bevisnoder. Varje nådd bevisad åtkomst kontrolleras mot den verkliga
modellerade lagringen. Inga körningar är ofullständiga; högsta bränsleåtgång
är 143 av 5000. Avsiktligt falska bevis och oändliga loopar provar att oraklet
rapporterar fel. Detta är en fryst ändlig testdomän, inte ett formellt bevis
för alla program. [Exporter och oberoende resultat](proof-qa/independent-models.json),
[full verifieringslogg](proof-qa/integrated-verification.log).

Emitterproven omfattar fyra mål, av/på/budgetläge, verkliga relokeringar,
oförändrade aritmetik- och nollkontroller samt felaktiga apply-livscykler.
Kapacitetsproven täcker maxlängder, breda ogiltiga begäranden, push före
mutation, kloning, konverteringar och återanvända Wasm-instanser.
Linuxbilder är byggda men har inte körts här.

Nästa grundsteg är en gemensam nodarena och uttrycksemission som kan
återanvändas för begränsad inlining utan att kopiera funktionsmodeller.
Intervallanalysen kan senare ge separata bevis för i32-aritmetik.
Kompilatorkostnad och kodtillväxt ska mätas innan nya vägar aktiveras.
