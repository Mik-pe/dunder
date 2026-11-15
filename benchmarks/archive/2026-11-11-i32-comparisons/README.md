# i32-bevis och jämförelser mellan registerhem

Sorteringens median förbättras 23,3 %, från 4,55 till 3,49 ms. C-geomedlet
är nu 1,415× och största enskilda gapet 1,490×. Målen 1,10× och högst
1,25× är fortfarande inte uppnådda. Numerik och text är byteidentiska med
föregående emitter och tillskrivs ingen förbättring.

## Generella mekanismer

Intervallanalysen kan nu bevisa i32-addition, subtraktion, multiplikation
och negation. Den räknar hela råintervallet i i64 före klippning och
producerar bara bevis under den separata återspelningen av slutliga
loopinvarianter. i64, division och rest får inga nya aritmetikbevis här.
Funktioner utan arrayer kan också analyseras; funktioner utan relevanta
kandidater hoppar över själva analysen.

En oberoende granskning hittade ett kompilatorkraschfall före aktivering:
`(2147483647*2147483647)*0`. Barnet saknar normal fortsättning, men dess
tomma intervall kunde nå förälderns matematik. Analysen stoppar nu efter
sådana barn och kontrollerar även nåbarhet före råaritmetik. Programmet
kompilerar och ger overflow först när det körs. Regressionerna inkluderar
negation, båda operandordningarna och flera i32-gränser.

Jämförelseemissionen använder samtidigt lokala registerhem och cachade
längdregister direkt. Den laddar bara saknade/spillda operander och använder
små CMP-immediater där de ryms. Konstant till vänster får korrekt vänd
signerad relation; sammansatta operander behåller fällordningen.
Villkorsgrenar och bool-resultat delar samma val. Inga nya relokeringar eller
målberoende källregler införs. [Kontrakt](../../docs/optimization.md).

Den tidigare gemensamma nodarenan används för båda mekanismerna. En begränsad
verifierad inlining-katalog och dess riktiga emitterprov ingår också i denna
revision, men katalogen används ännu inte av vanliga kompilatorbyggen.

## Runtime och bygglatens

Kompilatorkoden är `f6519a8`, jämförd med `e40f6b3`. Kandidatens SHA-256 är
`b48cd5089b167399ace026198f4a44c0042578949f95ff84faf4a88680f6ac9e`.
Båda bygger samma slutliga källor på Apple M5. Trettio färska processer per
variant, växlande ordning, varm OS-cache och ingen Dunder-cache eller
profilering används. Varje programresultat verifieras.

Median / nearest-rank p95 i millisekunder. C byggs med clang `-O3 -ftrapv`
och saknar Dunders arraygränskontroller.

| Arbete | Före | Efter | C | Efter / C, median |
| --- | --- | --- | --- | --- |
| Numerik | 6,60 / 9,65 | 6,60 / 11,30 | 4,59 / 6,36 | 1,439× |
| Arrayläsning | 3,06 / 6,53 | 3,10 / 6,49 | 2,40 / 4,67 | 1,293× |
| UTF-8-text | 3,75 / 7,28 | 3,77 / 6,02 | 2,61 / 4,35 | 1,445× |
| Sortering | 4,55 / 8,68 | 3,49 / 5,49 | 2,34 / 3,91 | 1,490× |

Arrayläsningens median ökar 1,2 % trots färre jämförelseinstruktioner.
Den försämringen bevaras. Numerikens p95 ökar 17,1 % trots identisk binär.
Binärstorlekarna är 16 544 byte för samtliga fyra Dunder-fall; filformatets
justering döljer mindre skillnader i instruktionsantal.

| Byggarbete | Före median / p95 | Efter median / p95 |
| --- | --- | --- |
| Genererat bygge | 327,75 / 332,15 | 328,18 / 333,45 |
| Genererad kontroll | 184,83 / 185,89 | 185,33 / 186,39 |
| Självbygge | 178,27 / 179,20 | 179,57 / 181,02 |
| Självkontroll | 112,77 / 114,62 | 113,99 / 116,17 |
| Tabellbygge | 10,60 / 11,85 | 10,71 / 13,29 |
| Tabellkontroll | 4,97 / 6,09 | 4,95 / 6,47 |

Byggmedianerna ökar 0,13 / 0,73 / 1,03 %. Tabellbyggets p95 ökar 12,2 %.
Genererad genomströmning vid p95 är cirka 303 000 rader/s. Byggbildernas
storlekar är 2 413 280 / 1 543 232 / 49 376 byte; genererad bild är
byteidentisk medan självhost och Tabell får ändrad kod.
[Råtider](final/samples.csv), [metadata](final/metadata.json).

Med samma kandidatbinär och hela intervallpasset av/på är sorteringen
4,95 / 6,54 ms av och 3,51 / 5,14 ms på: medianvinsten är 29,1 %.
Den serien stänger av både tidigare bounds-bevis och nya i32-bevis;
registerjämförelserna är på i båda lägena. Den är därför ingen isolerad
procentsiffra för enbart den nya aritmetiken. Byggmedianpåslagen är
0,05 / 0,21 / 0,99 %, med Tabell-p95 10,97 / 12,73 ms.
[Av/på-råtider](ablation/samples.csv), [metadata](ablation/metadata.json).

## Oberoende fall, fasarbete och korrekthet

Dogfood-revision 2 är oförändrad. Medianerna före/efter är POD
48,71 / 48,70 ms, decimal 71,95 / 71,90 ms, lexer 89,64 / 89,78 ms
och symboltabeller 99,19 / 99,17 ms. Samtliga medianförändringar är under
0,2 % i serien. I intervallpassets av/på-serie är de under 0,4 %.
Dessa fall saknar C-referens och ingår inte i C-geomedlet.
[Råtider](dogfood/samples.csv), [av/på-resultat](dogfood-ablation/summary.csv).

Fem separata observationer med fasur respektive allokeringsräknare
verifierar samma utbilder. Intervallfasens median är 0,082 ms för det
verkliga sorteringsbygget och 0,047 ms för självbygget; genererat bygge
och Tabell redovisar noll. Självbyggets fulla frontend är fortsatt störst,
64,0 ms i den instrumenterade observationen. Instrumenterade tider används
inte som vanlig bygglatens. [Kompilator](compiler-attribution/summary.json),
[sortering](sort-attribution/summary.json), med råloggar och proveniens.

Full `tools/verify-selfhost.sh` passerade: 129 nativa grupper, 121 Wasm-grupper,
åtta explicit nativa grupper, 50 diagnostikfall, 46 runtimekontrakt per
native/Wasm och 13 CLI-grupper på macOS. Bootstrap nådde identisk fixpunkt.
[Verifieringslogg](verification.log).

Den oberoende interpretern fullföljde 844 688 körningar och 248 712 dynamiska
kontroller av nya aritmetikbevis. Den räknar med obegränsade heltal innan
vanlig overflow-hantering. Ursprungliga 55 fall/697 400 körningar bevaras;
en separat kohort med 34 fall täcker i32-gränser och tomma fortsättningar.
Tio konstanta child-overflowfall ger 43 320 exakta overflowutfall utan nått
föräldrabevis. Inga körningar är ofullständiga; maxbränsle 143/5000.
Ett separat för litet bränsle ger uttryckligen exit 2 och `complete=false`.
Detta är ändliga domänprov, inte ett formellt sundhetsbevis för alla program.
[Modeller/resultat](proof-qa/independent-models.json), [bränsleprov](proof-qa/incomplete.json).

287 jämförelsefall granskar faktiska instruktioner och signerad semantik.
21 range-emitterfall kontrollerar exakt borttagning av bounds-/i32-kontroller,
oförändrade nollkontroller, budgetens byteidentiska återfall och fällordning.
Inlining-katalogens prov bygger 88 bilder med rebasing, frameplatser 37/620,
normalisering, caller-ABI och kodström som förblir orörd vid avslag.
Alla fyra mål byggs; host och Wasm körs. Linuxbilder är inte körda här.

Nästa arbete gäller begränsad inlining med full cacheidentitet och
beroendemetadata, samt delad emission av rune-avkodning. Båda ska använda
de generella kontrakten och mätas före aktivering.
