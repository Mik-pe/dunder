# Gemensam nodarena och uttrycksemission

Den typade IR:ens elva nodkolumner ägs nu av `Nodes`, separat från
funktionssignaturer och lokaler. Samma uttrycksemission kan låna arenan i en
befintlig anroparframe. Detta förbereder begränsad inlining; ingen inlining
är aktiverad i denna revision.

Kompilatorn återanvänder fortfarande modellen och registerplanen mellan
funktioner. En tom `Plan.frame_slots` betyder tidigare identitetsplacering;
en komplett karta kan ange andra frameplatser för uttrycksemission.
Helfunktions-API:erna kräver identitetsplacering. `emit_expression` lämnar
resultatet i x0 och skapar ingen frame, entry eller RET. Argument, canonical
i32-värden och eventuella arraycacher måste förberedas av anroparen.
[Kontrakt](../../docs/optimization.md#typad-funktionsmodell-och-lokalt-optimeringspass).

## Uppmätt byggkostnad

Kompilatorkoden är `e40f6b3`, testintegrationen `c2afa11`. Baseline är
`90f303d`. Kandidatens SHA-256 är
`178a31167707b46717482809ad59e11e613a946d78d32387e5ed6101d6219fc9`.
Samma slutliga källor byggs av båda. Trettio färska processer per variant,
växlande ordning, varm OS-cache, ingen Dunder-cache och ingen profilering
används på Apple M5. Alla utdata verifieras.

Median / nearest-rank p95 i millisekunder:

| Arbete | Före | Efter | Medianförändring |
| --- | --- | --- | --- |
| Genererat bygge | 316,82 / 319,87 | 323,24 / 338,96 | +2,02 % |
| Genererad kontroll | 180,99 / 182,58 | 183,84 / 184,95 | +1,58 % |
| Självbygge | 176,65 / 182,29 | 176,05 / 183,45 | −0,34 % |
| Självkontroll | 112,05 / 114,53 | 111,12 / 112,51 | −0,82 % |
| Tabellbygge | 10,55 / 10,80 | 10,39 / 10,86 | −1,59 % |
| Tabellkontroll | 4,86 / 6,13 | 4,93 / 6,23 | +1,38 % |

Genererat bygges p95 ökar 5,97 % och genomströmningen där är cirka
298 000 rader/s. Arenan är alltså inte kostnadsfri. Självbygge och Tabell
visar ingen motsvarande medianregression i serien. Ingen runtimevinst
tillskrivs denna förändring.

Alla tre byggbilder är byteidentiska mellan kompilatorerna: genererat
2 413 280 byte, självbygge 1 543 232 och Tabell 49 376. Självbyggets källa
innehåller den nya infrastrukturen; storleken ska därför inte jämföras som
emittereffekt med föregående rapports andra självhostkällor.
[Råtider](samples.csv), [sammanfattning](summary.csv), [metadata](metadata.json).

## Verifiering

Full `tools/verify-selfhost.sh` passerade: 116 nativa grupper, 108 Wasm-grupper,
åtta explicit nativa grupper, 50 diagnostikfall, 46 runtimekontrakt per
native/Wasm och 13 CLI-grupper på macOS. Bootstrap nådde identisk fixpunkt.
[Logg](verification.log).

Sex källfall över fyra mål och två intervallägen ger 48 exakta jämförelser
av utbilder samt bindnings-/diagnostikdata mot föregående emitter.
De täcker aritmetik, arraymutation, spill, åtta argument, ägande och platser.
[Jämförelseprogram](identity.py), [resultathashar](identity.json).

Det nya uttrycksprovet bygger 72 bilder på fyra mål och kontrollerar
frameplatserna 37/620, kortslutning, fällordning, FP/SP och callee-saved
register. Felaktiga kartor får inte publicera någon bild. Host och Wasm
har körts; Linuxbilder har byggts men inte körts här.

Nästa steg är en begränsad katalog med verifierade retur-uttryck,
utvärderade argument och full cacheidentitet för inlinade kroppar. Den
mekanismen måste mätas separat före aktivering.
