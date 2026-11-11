# Generell signerad konstantdivision

Numerikkärnans median minskar 12,2 %, från 8,27 till 7,26 ms. Samma
kompilator med sänkningen av/på ger 12,6 %. Byggmedianerna är i stort sett
oförändrade. C-svitens geomedel är nu 1,586×; 1,10×-målet är fortfarande
inte uppnått och sorteringens 2,160× är det största gapet.

## Mekanism och provad semantik

Verifierade konstanta `i32`-/`i64`-delare sänks med en återanvänd
multiplikatorplan och exakt teckenkorrigering. Noll, ±1, tvåpotenser och
minimidelaren har separata fall. Täljaren utvärderas en gång och först,
även vid `% 1`. Nollfällan samt overflow för både `MIN / -1` och
`MIN % -1` bevaras. Planeringen har högst 128 steg; återfall använder den
redan beräknade täljaren i den gamla vägen. Ingen benchmarkkonstant eller
funktionsidentitet känns igen.

ARM64 och x86-64 använder den generella sänkningen. Wasm behåller vanlig
division för de generella multiplikatorfallen och använder bara de enkla
specialfallen. Endast scratchregistren x0–x3 påverkas.
[Implementation och kontrakt](../../docs/optimization.md#signerad-division-och-rest-med-konstant-delare).

Kompilatorkoden är `6e2331e`, jämförd med föregående `a1b8369`. Kandidatens
SHA-256 är `87d0070524a5cbe1a095325e0190736695ab267bca4d1ef124158befcf42c496`.
Samma slutliga källor byggs av båda kompilatorerna. Mätningen använder
30 färska processer per variant, växlande ordning, ingen Dunder-cache och
varm OS-filcache på Apple M5/macOS. Resultatet verifieras efter varje bygge
och körning. Profileringsflaggor används inte.

## Fyra frysta C-fall

Median / nearest-rank p95 i millisekunder; C byggs med clang `-O3 -ftrapv`.
C saknar Dunders arraygränskontroller.

| Arbete | Före | Efter | C | Efter / C, median |
| --- | --- | --- | --- | --- |
| Numerik | 8,27 / 13,26 | 7,26 / 12,27 | 4,60 / 5,68 | 1,579× |
| Arrayläsning | 3,02 / 5,67 | 3,06 / 7,59 | 2,40 / 4,20 | 1,278× |
| UTF-8-text | 3,75 / 6,00 | 3,76 / 5,32 | 2,59 / 3,90 | 1,451× |
| Sortering | 5,07 / 8,71 | 5,11 / 7,45 | 2,36 / 3,42 | 2,160× |

Bara numerikbilden ändras; de tre övriga Dunderbilderna är byteidentiska
före/efter. Deras observerade variation, inklusive arrayläsningens 33,7 %
högre p95, tillskrivs därför inte sänkningen. Numerikens separata
[av/på-serie](ablation/summary.csv) ger 8,26 / 10,92 ms av och
7,22 / 7,98 ms på. Ingen bred C-paritet följer av en förbättrad kärna.

## Byggtid

| Arbete | Före median / p95, ms | Efter median / p95, ms |
| --- | --- | --- |
| Genererat bygge | 318,06 / 322,34 | 318,03 / 321,78 |
| Genererad kontroll | 184,74 / 185,76 | 183,98 / 184,99 |
| Självbygge | 163,15 / 163,98 | 162,99 / 165,13 |
| Självkontroll | 102,96 / 105,60 | 102,23 / 103,13 |
| Tabellbygge | 10,58 / 11,00 | 10,61 / 10,94 |
| Tabellkontroll | 4,95 / 6,03 | 4,96 / 6,42 |

Genererat bygge når cirka 314 000 rader/s vid p95. Bildstorlekarna är
oförändrade för dessa källor. Självbyggets p95 ökar 0,7 % och
Tabellkontrollens 6,6 %. Den separata av/på-serien ger 0,08 % medianpåslag
för genererat bygge; ingen märkbar byggkostnad kan urskiljas här.

[Alla råtider](final/samples.csv), [sammanfattning](final/summary.csv),
[metadata](final/metadata.json) samt
[av/på-råtider](ablation/samples.csv), [av/på-metadata](ablation/metadata.json).

## Oberoende konsumentfall

De fyra konsumenterna använder oförändrad arbetsmängdsrevision 2.
Medianerna före/efter är POD 47,60 / 47,66 ms, decimal 70,35 / 70,60 ms,
lexer 89,50 / 89,53 ms och symboltabeller 99,69 / 99,69 ms. Alla fyra
programbilder är byteidentiska. Denna mekanism ger alltså ingen
kodförändring eller belagd runtimevinst i dessa fall.
[Råtider](dogfood/samples.csv), [sammanfattning](dogfood/summary.csv),
[metadata](dogfood/metadata.json) och separat
[av/på-serie](dogfood-ablation/summary.csv) sparas. Fall utan C-referens
ingår inte i C-geomedlet.

## Korrekthet och nästa grund

`tools/verify-selfhost.sh` passerade med 101 nativa grupper, 93 Wasm-grupper
och åtta explicit nativa grupper, 50 diagnostikfall, 46 runtimekontrakt
per native/Wasm och elva CLI-grupper på macOS. Bootstrap nådde byteidentisk
fixpunkt. Cache och berörda testkvitton skiljer konstantdivision av/på åt.

Det oberoende konstantdivisionsprovet validerar 2 360 verkliga Dunder-planer
med 1 425 530 exakta numeratörfall, inklusive uttömmande signerade bredder
3–10 och matematiska certifikat för breda planer. En separat körning mot
föregående kompilator och nya av/på-lägen provar 77 program, 422 host/Wasm-
exekveringar, 422 ytterligare korsbyggen, 325 800 exakta resultatrader och
216 identiska felrapporter. Nativa fällkoder är exakta; Wasm visar
`unreachable` med kontrollerad stdout före fällan. Linuxbilder är byggda,
men Linuxkörning ingår inte i denna etapps evidens.

Nästa avgränsade mekanismer är framekostnad i registerplacerade
anropsfria funktioner och intervall-/längdbevis för muterbara arrayer.
Ägar-IR och destinationsbaserade POD-värden behövs också för större
språkfall; de ersätts inte av denna aritmetiska sänkning.
