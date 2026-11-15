# Kompilatordogfood och attribuering

Denna etapp ger mätunderlag för nästa generella optimeringsmekanism.
Kompilatorkoden är `2ac8b6a`, jämförd med `162fb65`. Båda binärerna är
självhostade på ARM64/macOS. Den nya vanliga kompilatorns SHA-256 är
`51b12d3d36370a20dad5db2a6fc5a0b7b0ee52eb55932098be215493d9254c91`.
Maskinen är Apple M5 med 24 GiB; fulla villkor och identiteter finns i
metadata för respektive serie.

## Kostnad med profileringen avstängd

30 färska processer per variant och arbetsfall, växlande ordning, ingen
Dunder-cache och varm OS-filcache. Varje byggd bild körverifieras.
Profileringsflaggorna används inte i denna serie. Tabellen visar median
och nearest-rank p95 i millisekunder.

| Arbete | Före median / p95 | Efter median / p95 | Medianändring |
| --- | --- | --- | --- |
| Genererat bygge | 362,44 / 377,63 | 368,81 / 382,64 | +1,8 % |
| Genererad kontroll | 217,56 / 224,43 | 222,30 / 227,08 | +2,2 % |
| Självbygge | 188,41 / 190,17 | 189,17 / 192,11 | +0,4 % |
| Självkontroll | 129,45 / 132,32 | 128,47 / 129,79 | −0,8 % |
| Tabellbygge | 10,85 / 11,78 | 10,93 / 13,22 | +0,8 % |
| Tabellkontroll | 5,63 / 6,63 | 5,67 / 6,87 | +0,6 % |

Tabells bygg-p95 ökar 12,2 % trots liten medianändring. Serien visar inte
att alla latensmål är uppfyllda. Genererat bygge når cirka 264 000 rader/s
vid p95. Samma nya källor används för båda kompilatorerna; detta jämför
kompilatorimplementationerna och håller den byggda arbetsmängden fast.
Bildstorlekarna är identiska inom varje arbetsfall.
[Råtider](observer-overhead/samples.csv), [sammanfattning](observer-overhead/summary.csv),
[metadata](observer-overhead/metadata.json).

```sh
python3 tools/performance.py BASELINE CANDIDATE REPORT \
  --suite compiler --include-check --rounds 30
```

## Kompilatorns eget arbete

Fem separata par per arbetsfall med
[`profile-compiler.py`](../../tools/profile-compiler.py). Varje par använder
en vanlig kompilator med fasur och en separat allokeringsinstrumenterad
kompilator utan ur. Bilderna som de producerar är byteidentiska genom hela
insamlingen. Självbygget är dessutom samma fixpunkt som indatakompilatorn.
Alla genererade program, självbyggenas hello-program och Tabellresultat
körverifierades. Profilerade tider är attribuering, inte vanlig bygglatens.

Medianen för självbyggets fulla frontendkroppar är 76,40 ms. Modellparsning
är 0,185 ms och verifiering före/efter 0,054 / 0,035 ms. Det genererade
korpuset använder modellen mycket mer: modellparsning 72,72 ms och de två
verifieringarna 21,61 / 20,06 ms. Modellens låga kostnad för självbygget
beror på liten täckning och får inte tolkas som låg kostnad för alla program.

Självbygget registrerar cirka 217 477 allokeringar, 76,32 MB begärda
payloadbyte, 19,86 miljoner retain och 55,14 miljoner release. Arraykloner
står för 21,57 MB av 46,21 MB registrerad kopiering. Postkonstruktion och
postkloning begär tillsammans 0,33 MB. Retain/release räknar hjälparanrop
även för noll och odödliga värden; talen är varken frigöringar eller läckor.

[Sammanfattning](compiler-attribution/summary.json),
[kommandon, identiteter och alla observationer](compiler-attribution/metadata.json)
och [rå stdout/stderr](compiler-attribution/raw/) bevaras. Stora genererade
bilder och korpusfiler återgenereras av verktyget och checkas inte in.
[Kontrakt och begränsningar](../../docs/compiler-attribution.md).

## Oberoende konsumentfall

Fyra [dogfood-program](../dogfood/README.md) provar POD-poster,
decimalparsning, den verkliga lexern och symboltabeller. Resultaten jämförs
med oberoende Pythonreferenser. Arbetsmängderna kalibrerades en gång mot
`162fb65` och frystes före jämförelser: 16 135, 550, 20 respektive 30.
[Rå kalibrering](calibration/) är enstaka storleksobservationer, inte
prestandafördelningar.

Den första fulla [allokeringsattribueringen](consumer-attribution/metadata.json)
verifierar alla fyra resultat och räknar faktisk trafik. De nya fallen
har ännu inga C-referenser. Den befintliga fyrkärniga C-sviten och dess
acceptansgränser är oförändrade; föregående etapps C-geomedel är fortfarande
cirka 1,61× och målet är inte uppnått.

## Verifiering och nästa mekanism

`tools/verify-selfhost.sh` passerade: 84 nativa grupper, 78 Wasm-grupper,
45 diagnostikfall, optimerarens 240 körjämförelser på native/Wasm,
1 176 identiska felmeddelanden, 49 emissionsfall för alla fyra mål,
14 Tabelltester och 45 runtimekontrakt per native/Wasm. Därutöver
passerade modulresolversvitens 15 grupper och resolverns externa
korsmålsprov. Defaultbilder är byteidentiska mot baslinjen i nio
profilertestbilder för de tre nativa målen. Linuxbilder korsbyggdes;
Linuxkörning ingår inte i denna etapps evidens.

Profilerna motiverar nu en verifierad representation för funktionskod och
ägarlivslängder i den fulla frontendens lowering. Först prövas borttagning
av extra strängoperandägare där hela högerledet bevisligen saknar anrop
och skrivningar. Därefter behövs explicita ägartillstånd och överföring
av sammansatta temporärer med bevarad värdesemantik. Ingen kontroll ska
tas bort enbart för att en benchmark blir snabbare.
