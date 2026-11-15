# Verifierade strängägare och kompilatordogfood

Självbyggets median minskar 15,0 % och självkontrollens 22,1 %. Den verkliga
lexern blir 8,7 % snabbare i det frysta konsumentfallet. Den oförändrade
fyrfallssviten missar fortfarande C-målet: geomedel 1,635× och största kvot
2,157×. Detta är en förbättring av kompilatorns genererade program och
byggtid, inte ett uppnått anspråk om bred C-paritet.

## Mekanism och identitet

En extra strängoperandägare utelämnas när hela högerledet bevisligen saknar
relevanta effekter. Ursprungliga ägare och andra temporärer lever kvar.
Ett validerat maskin-IR korrigerar instruktioner, etiketter, relokeringar
och importernas byteadresser atomiskt; det verifierar redigeringsplanens
form. Effektreglernas semantiska sundhet granskas och provas separat.
Det finns ingen regel för ett visst funktionsnamn eller en viss benchmark.
[Omfattning och säkerhetsregler](../../docs/optimization.md).

Samtidigt rättas blockgränser för `if` med struct-konstruktorer i villkoret.
Det är ett vanligt språkfel som dogfoodingen exponerade. Den ursprungliga
giltiga tail-returen kunde tidigare avvisas som oanvänt värde.

Kompilatorkoden är `a1b8369`; tidigare kompilator är `2ac8b6a`. Båda bygger
samma slutliga källor. Kandidatens SHA-256 är
`6479e827363bf0bfc93977d9f89d18fd7c13e65a6e20c6ed85c5f528cbfade1a`.
Alla jämförelser använder vanliga binärer, 30 färska processer per variant,
växlande ordning, varm OS-cache och ingen Dunder-cache på Apple M5/macOS.
Varje resultat kontrolleras. Kompletta flaggor, källhashar och bildidentiteter
finns i varje metadatafil.

## Vanlig bygglatens

Median / nearest-rank p95 i millisekunder:

| Arbete | Före | Efter | Medianändring |
| --- | --- | --- | --- |
| Genererat bygge | 353,23 / 356,60 | 320,69 / 326,37 | −9,2 % |
| Genererad kontroll | 218,28 / 220,15 | 184,95 / 185,62 | −15,3 % |
| Självbygge | 189,94 / 191,17 | 161,40 / 166,27 | −15,0 % |
| Självkontroll | 131,28 / 135,98 | 102,30 / 105,79 | −22,1 % |
| Tabellbygge | 11,00 / 11,65 | 10,32 / 12,01 | −6,2 % |
| Tabellkontroll | 5,53 / 6,63 | 4,98 / 6,14 | −10,0 % |

Tabells bygg-p95 ökar 3,1 % trots bättre median. Genererad bygggenomströmning
är cirka 309 500 rader/s vid p95. Självbyggets bild minskar från 1 510 400
till 1 428 320 byte; övriga byggbilders storlekar är oförändrade.
[Råtider](final/samples.csv), [sammanfattning](final/summary.csv),
[metadata](final/metadata.json).

Med samma nya kompilator och enbart `--ownership-opt=off` i baseline
isolerar [av/på-serien](ablation/summary.csv) passets byggkostnad. Medianpåslaget
är 0,10 % för genererat bygge och 0,12 % för självbygge. Denna serie mäter
passet i en redan självoptimerad kompilator; före/efter-serien fångar också
att kompilatorns egna strängoperationer nu kör snabbare.
[Råtider](ablation/samples.csv) och [metadata](ablation/metadata.json).

## Oförändrad C-svit

| Arbete | Dunder median / p95, ms | C median / p95, ms | Mediankvot |
| --- | --- | --- | --- |
| Numerik | 8,32 / 15,28 | 4,56 / 5,72 | 1,826× |
| Arrayläsning | 3,06 / 5,32 | 2,45 / 4,32 | 1,252× |
| UTF-8-text | 3,74 / 5,70 | 2,58 / 4,45 | 1,451× |
| Sortering | 5,08 / 7,27 | 2,36 / 3,65 | 2,157× |

C byggs med clang `-O3 -ftrapv`; C har inte Dunders arraygränskontroller.
Samtliga fyra Dunderbilder är byteidentiska före/efter. Deras p95 är högt
i denna serie; både råfördelningarna och den separata av/på-serien bevaras.
Ingen runtimevinst tillskrivs denna mekanism för dessa fyra program.
Acceptansgränserna 1,10× geomedel och högst 1,25× per fall är fortsatt mål.

## Oberoende konsumenter, arbetsmängdsrevision 2

| Fall | Före median / p95, ms | Efter median / p95, ms | Medianändring |
| --- | --- | --- | --- |
| POD-poster | 48,14 / 48,79 | 48,06 / 48,73 | −0,2 % |
| Decimalparsning | 71,54 / 72,05 | 71,38 / 73,28 | −0,2 % |
| Verklig lexer | 99,00 / 100,31 | 90,43 / 91,28 | −8,7 % |
| Symboltabeller | 100,93 / 101,81 | 99,16 / 103,19 | −1,8 % |

Symboltabellens och decimalfallets p95 ökar 1,4 respektive 1,7 %. Dessa nya
fall har oberoende Pythonreferenser och ingår inte i C-geomedlet.
[Råtider](dogfood/samples.csv), [sammanfattning](dogfood/summary.csv),
[metadata](dogfood/metadata.json). Den separata
[av/på-serien](dogfood-ablation/summary.csv) visar lexervinst 8,5 % och
symboltabellsvinst 1,7 % med samma kompilatorbinär.

Revision 1 hade en protokollskillnad: kalibreringen räknade första starten
av en ny binär, medan jämförelsen använde uppvärmning. De små arbetsmängderna
gav då processstarter för stor vikt. [Gamla före/efter-resultat](dogfood-revision1/summary.csv)
och [gamla av/på-resultat](dogfood-ablation-revision1/summary.csv) sparas.
Revision 2 använder en kontrollerad uppvärmning även vid kalibrering.
De nya mängderna frystes mot en enda tidigare baseline innan de nya serierna:
593 041, 14 479, 284 och 445. De anpassas aldrig efter variant eller försök.
[Kalibrering](calibration-revision2/) och [manifest](../dogfood/workloads.json).

## Utförd trafik och nästa grund

[Fem separata par](compiler-attribution/metadata.json) av fasur och
allokeringsobservationer gav byteidentiska programbilder och verifierade
resultat. Självbyggets fulla frontendkroppar har median 55,65 ms i den
profilerade kompilatorn. Trafiken omfattar cirka 222 022 allokeringar,
76,91 MB begärda byte, 6,56 miljoner retain och 19,24 miljoner release.
Fasurens egna värden ingår inte i allokeringsobservationerna. Dessa tider
är attribuering, inte vanlig bygglatens.

[Allokering med passet av](allocations-off/metadata.json) och
[på](allocations-on/metadata.json) använder exakt samma konsumenter och
arbetsmängder. Lexerns release-anrop minskar från 30 328 947 till
18 261 787; symboltabellens från 9 862 306 till 8 313 261.
Antal allokeringar är identiskt i varje fall. POD och decimal har samma
retain/release-trafik i båda lägena. Räknarna mäter hjälparanrop inklusive
noll och odödliga värden, inte frigöringar, läckor eller toppminne.

`tools/verify-selfhost.sh` passerade: 97 nativa grupper, 89 Wasm-grupper
och åtta explicit nativa grupper, 50 diagnostikfall, 46 runtimekontrakt på
vardera native/Wasm, 14 Tabelltester, tio CLI-grupper och hela tidigare
optimerar-/emissionssviten. Nya MIR-, lease- och villkorsprov bygger alla
fyra mål och kör host/Wasm; lease-provet jämför dessutom diagnostik mot
den tidigare kompilatorn. Linuxkörning har inte gjorts i denna etapp.

Nästa numeriska mekanism är generell konstantdivision/rest med exakt
signerad semantik. För större compilerkostnader finns en
[konkret plan för ägar-IR och CFG](../../docs/ownership-ir-plan.md): först
verifierad analys, därefter borttagning av bevisat tom cleanup. Båda ska
bedömas med samma byggbudget och säkerhetsregler.
