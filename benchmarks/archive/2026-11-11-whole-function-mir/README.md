# Helfunktions-IR: första arkitekturmilstolpen

Standardkompilatorn går nu genom en gemensam IR för alla nyemitterade
källfunktioner, även med anrop, ägda värden och loopar. Första integrationen
behåller byteidentisk kod. Den mäter därför arkitekturens byggkostnad;
ingen runtimevinst tillskrivs denna etapp och C-målet är fortfarande öppet.

## Vad som är levererat

Kedjan är full funktion → kanoniska operationer/CFG/minnesplatser →
obligatorisk verifiering → atomisk sänkning. Arenor, dekoder, verifiering
och sänkningsminne återanvänds. Kontroll utan emission skapar inga
backendarenor, och cacheträffar återställer slutlig kod utan en ny IR-runda.

Verifieringen kontrollerar både exakta kvarhållna kodord och syntetiska
operationer. Operander, maskinbredder, NZCV, anrops- och retur-ABI,
bytealias, framebevis, scratchregister, kontrollkanter, imports och etiketter
har explicita kontrakt. Ändrad semantik måste sänkas kanoniskt. Okända
effekter ger en barriär; trasiga metadata blir ett internt fel.

Ett separat testat, budgeterat pass för CFG-liveness, SCC och promotion av
oexponerade åttabytesplatser finns också på main. Det är ännu inte
inkopplat i denna kandidat. Den fortsatta
[arkitekturplanen](../../docs/whole-function-ir-plan.md) beskriver register
över hela funktioner, SSA, loopplacering och direkta målbackends.

## Byggkostnad

Trettio nya processer per variant och arbetsfall, alternerad ordning,
Apple M5/24 GiB, macOS 27.2, varm OS-filcache och ingen Dunder-cache.
Profilering är avstängd och varje utbild kontrolleras. Båda kompilatorerna
bygger samma källor från `4c43288`. Baseline är den frysta kompilatorn
`4b73d04b90e252741bc39e926a4c03b775710a6cfdaece1dc29dd413a39cf105`;
kandidatens SHA-256 är
`ce6f37fbcaa4d860b367817eb229e574de0aa691a33eaaa94c4a61ee76241fdd`.

Median / nearest-rank p95 i millisekunder:

| Arbete | Före | Helfunktionskedja | Medianförändring |
| --- | --- | --- | --- |
| Genererat bygge | 329,43 / 332,83 | 561,46 / 567,46 | +70,4 % |
| Genererad kontroll | 188,58 / 189,60 | 186,96 / 188,16 | −0,9 % |
| Självbygge | 214,30 / 215,81 | 398,96 / 401,29 | +86,2 % |
| Självkontroll | 139,19 / 142,56 | 139,70 / 140,59 | +0,4 % |
| Tabellbygge | 9,86 / 10,39 | 15,28 / 17,35 | +55,0 % |
| Tabellkontroll | 5,14 / 5,71 | 5,15 / 6,18 | +0,1 % |

Detta är ett stort påslag. P95 på det genererade 101 024-radersprovet ger
ändå cirka **178 000 rader/s**, över målet 100 000 rader/s. Det genererade
provet representerar inte hela språkets genomströmning. Både den nya
arkitekturens kostnad och nästa optimeringsstegs nytta måste redovisas;
en snabbare isolerad loop räcker inte som acceptans.

Samtliga tre byggbilder är byteidentiska mellan varianterna. Självbyggets
utbild är också identisk med kandidatkompilatorn. Filstorlekarna är
2 413 280 / 1 822 304 / 49 376 byte. Ingen C-serie eller inkrementell
latensserie kördes i denna etapp.
[Råtider](infrastructure/samples.csv), [sammanställning](infrastructure/summary.csv),
[identiteter och källhashar](infrastructure/metadata.json).

```sh
python3 tools/performance.py /path/to/baseline /path/to/whole-function-mir \
  /path/to/empty-report --suite compiler --include-check --rounds 30
```

## Korrekthet och oberoende kontroller

Full `tools/verify-selfhost.sh` passerade med självhostfixpunkt, 137 native-
och 124 Wasm-grupper, 13 uttryckligen nativa grupper, 50 diagnostikfall,
56 check/build-kontrakt, 15 CLI-grupper samt 46 runtimekontrakt per
native/Wasm. Även FFI, scalar/array/ownership/inlining, caches och de nya
IR-testerna ingår. Linuxbilder byggs här; Linuxkörning påstås inte.
[Full verifieringslogg](verification.log).

Den oberoende IR-tolken använder semantiska operationer, inte ARM-ord.
Den fullföljde 1 092 frysta heltals-/frame-/CFG-fall utan oväntat
bränslestopp. Hela källprogram med anrop, ägare, flyttal och fällordning
provas separat genom native/Wasm-körning: 192 bilder, 160 exakta
diagnostikjämförelser och identiska bindningsposter. Tolken gör inget eget
anspråk på att tolka anrop, ARC eller hela flyttalssemantiken.

Striktare verifiering avvisade två gamla testfixturer med fel
minneseffekter/framebevis; fixturerna rättades. Verifieraren försvagades
inte. Det separata registerpassets 96 målprogram kontrollerar bland annat
gamla anrop som förstör nya registerhem, alla returvägar, ägarinitiering
efter `RET`, looputgångar, checked overflow och divisionsfällor.
