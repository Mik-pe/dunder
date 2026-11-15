# Regional Frame/Expr-grund och sänkning i workers

Den nya integrerade versionen är **0,64–1,56 % långsammare i byggmedian** än den
föregående standardkompilatorn med fyra arbetare. Peak RSS sjunker i de större
byggfallen. Detta är paketets uppmätta kostnad, inte en ny total latensvinst.
**SSA förblir OFF som standard och C-målet är öppet.** Ingen ny runtimevinst för
Frame/Expr-placeringen har mätts.

Samma nya binär med fyra i stället för en arbetare ger 1,57× genomströmning för
den genererade korpusen, 1,24× för serial-entryn och 1,22× för parallel-entryn.
Fyra arbetare var redan en fungerande språk- och kompilatorfunktion. Den
jämförelsen visar nuvarande kapacitetsvinst; den är inte den nya revisionens
vinst över föregående fyrarbetarstandard.

## Fryst identitet och omfattning

- Källa: `073856c3a8d5a54644d6855bdaae25e184a0be34`, ren under hela mätningen.
- Tidigare kompilator: `9e5b774adff87835bbdf1cee07f19c2801facd6d4f08a1148d5a79c5bbe04de1`.
- Ny parallell standard: `7d94803b76b85729987746b0efb4538bb553bd0c079e6fe2eb0d694145bc9fea`.
- Ny seriell byggprodukt: `c133a697713cf69089dbe83b3dc6e03c290028be005e83f21e1ba3ccfc1088fb`.
- Matrixverktyg: `3a98b84d2278602baf303c24e08d4530aeb8cab17a1395cae54ff5c7ab572ee7`.
- Maskin: Apple M5, 24 GiB RAM, ARM64 macOS; full miljö finns i råmetadata.

Båda kompilatorerna bygger exakt samma slutliga källor. A jämför tidigare och ny
standard med SSA av och standardens fyra arbetare: sex build- och sex
check-grupper per variant, 720 prover. B jämför samma nya binär med `--jobs=1`,
`--jobs=2` och `--jobs=4`: sex byggfall, 540 prover. Varje fall/variant har 30
nya processer. A alternerar variantordning; B kör alla sex permutationer exakt
fem gånger. **1 260 prover i 42 grupper och 2 612 kommandokvitton** är bevarade.
Alla kommandon avslutades enligt sina förväntade framgångsvillkor. Ingen
mätserie kördes om och inga prover eller extremvärden togs bort.

Tiden täcker hela den nya kompilatorprocessen till `wait4`-avslut: källarbete,
kloning, trådstart/join, länkning och publicering. Bildhashning och
konsumentkörningar ligger utanför tidsintervallet. OS-filcache är varm;
incremental-cache är avstängd. Inga profiler eller fasklockor användes. RSS är
OS-rapporterad peak `wait4.ru_maxrss` för processen inklusive dess workertrådar,
omräknad till byte. Det är inte heapallokering, kvarhållet minne eller
minneskostnad per funktion. MiB betyder 1 048 576 byte. p95 är observation 29 av
30; parvisa kvoter använder samma rundnummer och redovisas separat från
kvoten mellan medianer.

`selfhost/main.ddr` och `selfhost/main_parallel.ddr` är **två separata byggfall**.
Inget av dem är tiden för hela bootstrapkedjan. Deras byggda kompilatorbilder
bygger och kör hello utanför mätintervallet. Generated innehåller 101 024
kodrader, 508 056 token, 21 moduler och 1 001 funktioner. Resultatet 595500
beräknas oberoende. Tabell kontrolleras med Python-CSV-summering och fryst
facit; owned med oberoende original-/mutationsvärden 21, 5 och 8.

## A: tidigare → ny standard, båda fyra arbetare

Tider är median / p95 i millisekunder. RSS är median / p95 i MiB.

| Fall | Tidigare tid | Ny tid | Byggmedian | Tidigare RSS | Ny RSS |
|---|---:|---:|---:|---:|---:|
| Generated 100k | 615,366 / 621,225 | 620,344 / 626,787 | 0,81 % | 77,20 / 77,20 | 75,17 / 75,19 |
| selfhost/main.ddr | 809,697 / 813,078 | 822,310 / 824,979 | 1,56 % | 66,72 / 66,72 | 63,48 / 63,48 |
| selfhost/main_parallel.ddr | 957,866 / 963,539 | 971,303 / 974,699 | 1,40 % | 71,05 / 71,05 | 67,20 / 67,20 |
| Tabell | 17,892 / 18,187 | 18,151 / 18,686 | 1,45 % | 8,06 / 8,06 | 7,53 / 7,53 |
| hello | 5,239 / 5,662 | 5,319 / 5,652 | 1,53 % | 3,97 / 3,97 | 4,00 / 4,00 |
| owned | 5,566 / 6,062 | 5,601 / 5,989 | 0,64 % | 4,22 / 4,22 | 4,25 / 4,27 |

Check går genom den gemensamma seriella vägen och är en negativ kontroll för
schemaläggningen, inte ett påstående om parallell typkontroll.

| Check-fall | Tidigare median / p95 ms | Ny median / p95 ms | Medianändring |
|---|---:|---:|---:|
| Generated 100k | 187,539 / 188,966 | 187,233 / 188,036 | -0,16 % |
| selfhost/main.ddr | 229,982 / 230,980 | 231,350 / 232,909 | 0,59 % |
| selfhost/main_parallel.ddr | 384,798 / 389,932 | 387,112 / 388,178 | 0,60 % |
| Tabell | 5,214 / 5,618 | 5,195 / 5,598 | -0,37 % |
| hello | 2,030 / 3,026 | 2,019 / 3,175 | -0,54 % |
| owned | 2,094 / 3,190 | 2,063 / 3,121 | -1,45 % |

## B: samma nya binär, 1 / 2 / 4 arbetare

| Fall | jobs1 median / p95 ms | jobs2 median / p95 ms | jobs4 median / p95 ms | Medianhastighet 1→4 |
|---|---:|---:|---:|---:|
| Generated 100k | 982,428 / 987,013 | 778,834 / 783,550 | 623,820 / 625,539 | 1,5749× |
| selfhost/main.ddr | 1023,754 / 1025,300 | 961,848 / 964,762 | 823,461 / 826,993 | 1,2432× |
| selfhost/main_parallel.ddr | 1186,908 / 1189,227 | 1126,950 / 1132,113 | 971,125 / 975,632 | 1,2222× |
| Tabell | 20,488 / 21,590 | 20,321 / 20,626 | 18,153 / 19,042 | 1,1286× |
| hello | 5,343 / 5,875 | 5,313 / 5,740 | 5,390 / 5,656 | 0,9912× |
| owned | 5,970 / 6,796 | 6,025 / 7,551 | 5,743 / 8,545 | 1,0395× |

| Fall | jobs1 RSS median / p95 MiB | jobs2 RSS median / p95 MiB | jobs4 RSS median / p95 MiB |
|---|---:|---:|---:|
| Generated 100k | 71,50 / 71,50 | 74,23 / 74,23 | 75,17 / 75,19 |
| selfhost/main.ddr | 57,23 / 57,23 | 62,00 / 62,00 | 63,48 / 63,48 |
| selfhost/main_parallel.ddr | 58,89 / 58,89 | 65,27 / 65,44 | 67,19 / 67,20 |
| Tabell | 5,50 / 5,50 | 7,34 / 7,36 | 7,53 / 7,53 |
| hello | 4,00 / 4,00 | 4,00 / 4,00 | 4,00 / 4,00 |
| owned | 4,08 / 4,08 | 4,08 / 4,08 | 4,27 / 4,27 |

Owned förbättras i median men försämras i p95: **6,796 → 8,545 ms** för en till
fyra arbetare. Hello får ingen medianvinst. Dessa svansar är kvar i rådata;
inget allmänt snabbare småbygge påstås. Alla parvisa median/p95-kvoter och
min/max finns i [slutvalideringen](all-cohorts-validation.json),
[råsvansarna](tails.csv) och de oförändrade summary-/samples-filerna.

Generated klarar gränsen 100 000 rader/s vid p95: A:s nya standard **161 178**;
B jobs1 **102 353**, jobs2 **128 931**, jobs4 **161 499** rader/s.

## Bilder, faktisk exekvering och korrekthet

Samtliga sex nativebilder är byteidentiska över A/B och alla arbetarantal.
Serial- och parallel-entryns bildhashar matchar bootstrapens frysta produkter.
De tre lämpliga Wasm-konsumenterna generated/hello/owned har byggts och körts
utanför tiderna med samma resultat och identiska bilder. De fyra vanliga
runtimefallen numeric/scan/text/sort har också byteidentiska bilder och
oberoende korrekta checksummor över revisioner/arbetarantal. Därför behövdes
ingen ny C-runtime-serie. Det ger inget nytt runtime- eller C-paritetspåstående.

Nativekörningarna här är ARM64 macOS. Wasm körs i den faktiska Node-motorn.
Hela kompilatorns och Tabells filbaserade CLI körs inte som Wasm-kompilatorer.
De separata QA-matrisernas ARM64-/x86-64-Linuxbilder är endast korsbyggda,
inte exekverade på Linux i detta bevis.

[verification-freeze.json](verification-freeze.json) binder den godkända
fullverifieringen till produktionskoden cc4ba3d, startrevision 4130d31 och den
slutliga mätfrysningen. [full-verify.log](full-verify.log) har exit 0 enligt
kvittot och omfattar seedkontroll, seriell stage3=stage4 och identiska
serial-produced/jobs1/jobs4-parallellbilder. Huvudsviterna redovisar 138 native-
och 124 Wasm-grupper (14 uttryckligt native-only) samt 46 runtimekontrakt.
Efteråt ändrades oraklet för att skriva även StackPush-parets övre nollhalva;
produktionsändringen därefter var kommentarer i `selfhost/mir.ddr`.
Återbyggt main_parallel gav exakt samma kompilatorbild. Den rättade generiska
matrisen, SSA, regional och gemensam Frame/Expr kördes sedan om separat:

| Efterkontroll | Målbilder | Native/Wasm-körningar | Oberoende semantik |
|---|---:|---:|---|
| [Generiskt orakel](oracle-padding-fixed.json.gz) | 1 092 orakelbilder | 548 | 1 092 fullföljda interpreterkörningar; max 188 steg av 10 000; 24 metadataavslag |
| [SSA](ssa-padding-fixed.json) | 670 | 360 | 438 par; 232 uttryckliga interpreterluckor |
| [Regional](regions-padding-fixed.json) | 454 | 252 | 396 par |
| [Gemensam Frame/Expr](expression-final.json) | **725** | **397** | **700 exakta bitpar** |

Matrisernas mängder överlappar och summeras inte till ett universellt bevis.
Det generiska oraklet redovisar två täckningsluckor och en avsiktlig
fuel-begränsad cykel. Calls/FP/externa minnen har specificerade gränser; stora
scratchfall och vissa resurs-/guardfall stöds av verklig native-/Wasm-körning.
`guard_upper` körs bara i Wasm. JSON och råloggar redovisar gränserna direkt.

[ObjectData-kvittot](qa/object-data/summary.json) redovisar 13 unitgrupper på
native och Wasm, 24 kernelbilder samt 6 native- och 6 Wasm-körningar. Det
provar verkliga tasks, återanvändning, mutation, discard, avslag, scopeåterinträde
och resultatets livstid efter barnpoolens/scopeets cleanup. Strukturvalidering
är inte ett semantiskt certifikat för godtyckliga körbara ord; task-scope samt
ordinal/deklaration förblir betrodd proveniens.

[StackOrigin-kvittot](qa/stack-origin/receipt.json) innehåller 16 raw/temp-bilder,
normal native-/Wasm-körning, verkliga push/pop-fel i Wasm och avsiktliga
icke-felande elisionskontroller. Ursprungstaggen ensam tillåter inte elision.
Detta bevisar inte att kompilatorn som helhet bevarar varje möjligt rått
stack-/OS-resursfel. Det valbara source-temporaries-kontraktets avgränsning
framgår av de frysta källorna.

Två tidigare försök till fullverifiering behålls öppet:
[pre-output-fix](full-verify.pre-output-fix.log) stoppade när ett verktyg korrekt
vägrade återanvända en gammal resultatkatalog; 4130d31 skapade separata färska
matriskataloger. [pre-schema-fix](full-verify.pre-schema-fix.log) stoppade när
rollback-schemans regex läste kommentarordet `only:` som ett fält; 038a536
exkluderade kommentarer. Dessa är inte godkända fulla körningar och loggarna
har inte skrivits över. Oraklets senare paddingrättelse är också redovisad;
produktionsverifieringen försvagades inte för att få ett grönt resultat.

## Arkitekturens gräns och reproduktion

Jobbet klonar fortfarande en full MIR-funktion med 62 fält in i tasken.
Worker utför verifiering/pass och sänkning lokalt och returnerar kompakt
numerisk ObjectData; föräldern gör strukturell import och ordnad acceptans,
cache/publicering och länkning. Frontend, typning och källtraversering är
fortfarande seriella. Det finns ingen beständig workerpool. In-/uttransport,
frontendarbete och framtida större parallelliseringsgränser är separata frågor.

A jämför hela det integrerade paketet och isolerar inte en enskild mekanism.
B ändrar kapaciteten på exakt samma nya binär. Frame/Expr-SSA är funktionellt
prövad men avstängd i dessa mätningar: ingen ny sådan runtimevinst är uppmätt.

Alla ursprungliga CSV-radbrytningar, commands.jsonl, loggar och övrig råmetadata
är bevarade byte för byte. Det stora generiska oraklets JSON är komprimerat
förlustfritt som gzip; den dekomprimerade bytemängden är identisk med originalet. [archive-manifest.json](archive-manifest.json) binder
kopiorna till sina orörda original; [checksums.sha256](checksums.sha256) täcker
arkivfilerna. [source-hashes.json](source-hashes.json) innehåller hela den
uppmätta källhashmängden och utvalda frysta käll-/verktygssnapshots. Komplett
källa och redan committade fixtures finns i Git-revision 073856c. Körbara
binärpayloads är utelämnade; bilderna och producenterna har SHA-256 i kvittona.
Absoluta ursprungssökvägar i råmetadata är avsiktligt oförändrade.

Exakta huvudkommandon kördes från den frysta huvudcheckouten:

```sh
python3 tools/performance-compiler-matrix.py build/ssa-regional/baseline build/ssa-regional/candidate build/ssa-regional/revision-performance --mode revision --include-check --rounds 30
python3 tools/performance-compiler-matrix.py build/ssa-regional/candidate build/ssa-regional/candidate build/ssa-regional/workers-performance --mode workers --rounds 30
```

Rapportkataloger måste vara tomma vid en ny, uttryckligt beställd reproduktion.
Arkiveringen själv körde inga nya byggen, tester eller tidsmätningar.

Generiska oraklets lossless gzip: original **13324221 byte**, SHA-256
`5e037e998541f1a2936ccef8e202affb7be95bdb34b4de6e42465d4cb077f9d2`; komprimerat **161952 byte**, SHA-256
`cd8aa2d95d7b79bcc62c1a22021d7d08f13de16fd2bf979a6a2dd9711ad78ed4`. Originalfilen i build är orörd.
