# Dunder-status

Kompilatorn på `main` är skriven i Dunder. Den bygger sig själv på ARM64/macOS,
ARM64/Linux och x86-64/Linux och kan från varje värd skriva kod för alla tre
nativa målen och wasm32.
[Återbyggnad](../bootstrap/README.md) och
[målplattformar](targets.md).

## Språk och runtime

- Isolerade uppgifter använder `task scope`, `spawn` och typad `await` på
  alla tre nativa mål. Scopeägda trådar, separata pooler och kontrollerad kloning
  håller managed-värden åtskilda. Typ- och effektanalys kontrollerar workerns
  gräns; scope-exit väntar in jobben före cleanup. Kompilatorn använder samma
  taskstöd för parallell MIR-verifiering, optimering och sänkning. [Uppgifter och begränsningar](tasks.md).
- Moduler har privata och exporterade symboler, alias, namngivna typer och
  stöd för cykliska importer. Typinferens, typade funktionsvärden, indirekta
  anrop, rekursion och aggregatreturer används i kompilatorns egen källkod.
  [Syntax](syntax.md)
- Heltal har kontrollerad aritmetik och konvertering, explicita
  bitoperationer och wrapping-metoder. `f32` och `f64` har IEEE-aritmetik,
  bitkonvertering, klassificering och avrundning. UTF-8-strängar stöder
  byteindex, runes, giltiga strängsnitt och innehållsjämförelse.
  [Flyttal](floating.md)
- Resursägande strängar, arrayer, structar och enums flyttas vid
  bindning och retur. Vanliga parametrar, loopar och match lånar;
  `move` markerar ägarövertagande och `.clone()` begär kloning uttryckligen. Nästlade structfält och enumpayloads kan äga samlingar.
  Vanliga arrayelement omfattar primitiva värden, strängar, pekare, structar,
  enums med eller utan payload, nästlade arrayer och typade tasknycklar.
  Kloning och cleanup följer samma regler som för fristående värden.
  `@c_layout` använder målplattformens fältstorlek
  och justering. [Enums](enums.md) och [FFI](ffi.md).
- `@resource` gör externa OS-resurser unika även när deras representation
  är skalär. Sockets och barnprocesser kan flyttas men inte klonas;
  rekursiv kloning och isolerade tasktransporter av sådana värden avvisas.
  Stängning och processhantering är fortfarande explicita operationer.
- Structfält kan flyttas och återinitieras oberoende, även genom nästlade
  fältkedjor. Hela värdet kan användas när alla fält är initierade.
  `replace(&mut place, replacement)` lämnar tillbaka den tidigare ägaren
  och installerar en giltig ersättare utan oinitierad lucka.
- Ägda arraybuffertar frigörs deterministiskt. Oföränderliga strängar
  använder referensräkning. Namngivna lokala läslån av fält och arrayelement
  gäller till deklarationsblockets slut. Array- och textintervall kan lånas
  som vyer utan heapallokering eller retain. Temporära uttryck kan lånas
  under anrop. Kontrollen avvisar överlappande flytt, ersättning och mutation.
  Regionen äger minnet i
  ett lexikalt scratch-block och kontrollerar vilka anrop som får låna det.
  [Minnesmodellen](memory.md)
- Standardbiblioteket innehåller map, set, array, text, hash, bytes,
  SHA-256, parse och sys. Sortering, hashning, omhashning och SHA-256 är
  Dunder-kod. Textläsning kontrollerar UTF-8 och filoperationer returnerar
  typade felvärden. [Standardbibliotek](standard.md) och [system-API](sys.md).

Bibliotekets lagringsoperationer tar över indata: map/set-nycklar,
processargument och stdinbuffertar samt JSON-dokumentets bytebuffert.
Läsoperationer lånar. Om anroparen vill behålla en självständig snapshot
anges `.clone()` där den behövs. Strängsortering och omhashning överför
befintliga ägare utan kloning. [Rekommenderade nästa steg](next-steps.md).

## Byggverktyg och exempel

`check` och `build` typkontrollerar även oanropade funktioner i importträdet.
`build` genererar körbar kod för nåbara funktioner. `check --agent` skriver
strukturerade poster med moduler, bindningar och positioner. Diagnostiken
anger ursprunglig fil, rad och kolumn. Nativa byggen stöder manifest,
funktionscache, atomisk publicering och testkörning med tidsgräns.
[Kompilatorns byggkedja](../selfhost/README.md) och [inkrementella byggen](incremental.md).

Standardprofilen optimerar anropsfria skalära funktioner genom en typad lokal
modell med konstantförenkling, konservativa loopbevis och registerallokering.
Delade `&[]i64`-parametrar stöds för läsning och exklusiva `&mut []i64`
för elementskrivning. Räknade `for`-loopar fångar sina gränser en gång.
Modellen verifieras före och efter passet. Ägda värden, storleksändring av
arrayer, flyttal och anrop använder tills vidare den befintliga kodgeneratorn.
Alla nyemitterade källfunktioner från båda vägarna går därefter genom den
gemensamma helfunktions-IR:n med obligatorisk verifiering och atomisk
sänkning. Generell CFG-liveness, framepromovering, ett uttryckligt kontrakt för
privata källobjekt och värdepropagering delar samma kedja. Ett nytt valbart
SSA-steg ligger mellan frame- och värdepassen.
`--mir-opt=off` och `--mir=off` stänger av de valfria MIR-passen och
behåller verifiering och sänkning.
[Omfattning och gränser](optimization.md).

`--mir-ssa=on` aktiverar den valbara regionala Frame/Expr-kedjan;
standard är fortsatt **off**. Privata åttabytesceller med fullbreddsskrivningar
och inneslutna 1/2/4/8-bytesvyer delar GP0–GP17 med positivt identifierade
privata uttryckstemporärer. En fysisk liveness-analys, gemensam interferens
för båda värdefamiljerna och en exklusiv undo-transaktion används.
Budgeten är totalt 4 194 304 enheter med obligatorisk slutverifiering.

Regional återläsning efter en barriär kräver full åttabytesinitiering på
alla inkommande vägar och ett separat kontrakt om aktiv frame-livstid efter
normal retur. Minnesinnehåll får ändras av anropet. `stack_roles` anger
ursprung; separat `source_temporaries` tillåter att privat uttryckslagring
utelämnas. Rå stacktrafik, argument och observerande barriärer behålls.
Source-object-, terminal-, call-input-, ABI-/frame- och temporärkontrakten
är skilda premisser; råa fabriker är konservativa. Call läser och kan
förstöra NZCV (`flags=3`); råmasker och clobbers begränsas inte.
Cache/kvitton binder fysisk v5 och SSA:s `v3-regional-expression1-work4194304`.
Generellt SSA-baserat operationsval, heap-CSE, LICM och direkt fysisk
x86-registerallokering/Wasm-lokalplacering återstår.

Den oberoende gemensamma matrisen passerar 725 målbilder, 397 native-/Wasm-
körningar och 700 exakta bitjämförelser, inklusive återläsning och
stackomskrivning i samma transaktion samt exakt rollback av alla 62 fält.
Basmatrisen passerar 670/360/438 med 232 uttryckliga interpreterluckor;
källacceptansen passerar 48 målbilder och 29 native-/Wasm-körningar med
workers, cache och kvitton. Linux är endast bildbyggt i dessa matriser.
Full verifiering inklusive seriell fixpunkt och byteidentiska självbyggen
med en och fyra workers passerar. [Kontrakt och avgränsning](optimization.md).

Historiska v2-mätningar gäller föregående SSA, inte regional Frame/Expr.
Med SSA på i båda kompilatorerna minskar v2 kompilatorkorpusets byggmedian
27,65 procent mot v1; Tabell förbättras 8,48 procent i median men får
20,41 procent högre p95. Samma nya kompilator med SSA av/på ger en
byggpremie på 21,26 procent för kompilatorkorpuset och 29,47 procent för
Tabell. Runtime/C-geomedlet blir 1,1841 med passet på mot 1,1883 av;
text/sort ligger på 1,3376/1,2649. Ingen samlad runtimevinst är belagd och
C-gränserna återstår. Standardlägets byggmedianer ändras 0,28–0,69 procent
mot förra kompilatorn med samma källor och byteidentiska konsumenter.

Den historiska [v1-mätningen](../benchmarks/archive/2026-11-12-ssa-foundation/README.md)
visade 70,92 procent högre byggmedian för kompilatorkorpuset och 42,34
procent för Tabell; runtime/C-geomedlet var 1,181 mot 1,178 med passet av.
En separat SSA-optimerad kompilatorbinär gav ingen meningsfull byggvinst
med oförändrad användarpolicy. Dessa siffror gäller v1, före gles v2 och
`source_inputs`, och är inga aktuella v2-resultat.
[SSA-orakel](../tools/test-mir-ssa.py) ·
[Källprov](../tools/test-ssa-source.py) ·
[Omfattning](optimization.md#ssa-för-privata-framevärden).

I den parade mätningen före ägarskapsbytet byggde den typade kedjan samma låsta
kompilatorkälla på 359,79 ms mot 468,17 ms vid `4585bc4`. Det blandade
korpuset tog 202,61 ms mot 220,13 ms. [Mätserie för den typade byggkedjan](../benchmarks/compiler/2026-11-15-shared-frontend/README.md)

I den föregående mätningen av den nativa kodgeneratorn tog bygget av den låsta
kompilatorkällan 458,91 ms på Apple M5. I runtimemätningen sjönk medianen för
sortering med 26,3 % och för arraysummering med 5,6 %. Heltalsberäkning och
UTF-8-text låg inom en procent av sin tidigare median.
[Mätserie och rådata](../benchmarks/compiler/2026-11-15-native-code/README.md)

Den tidigare [30-rundorsmätningen av parallell kompilering](../benchmarks/archive/2026-11-12-parallel-compiler/README.md),
före SSA-grunden, ger med samma binär genererad byggmedian
953,718→609,106 ms när workerantalet ökar från en till fyra. Standardkandidatens genererade p95 är 619,768 ms,
163 003 källrader/s. Runtimebilderna är identiska. C-geomedlet 1,178× och
största kvoten 1,308× i den rapporten ligger över målen 1,10× respektive
1,25×. C-målet är fortfarande öppet; rapporten är inget mätresultat för SSA.

Resursägande lokaler, resultat och managed-fält överförs utan en extra
datakopia och behåller sina arraykapaciteter. Kompilatorns jobb- och
resultatkedjor använder samma regel. Ett vanligt `spawn` av lånade indata
behöver ingen föregående `.clone()`; taskgränsen skapar själv den nödvändiga
isolerade snapshoten. Managed-lagring delas fortfarande inte mellan workerarenor.

Den fulla kodgeneratorn kan utelämna extra strängoperandägare när hela
högerledet saknar relevanta effekter. En validerad funktionsrepresentation
korrigerar instruktioner, etiketter och relokeringar atomiskt.
`--ownership-opt=off` gör mekanismen jämförbar separat från skalärpasset.
Fasur och allokeringstrafik kan profileras separat utan att vanlig emission
instrumenteras. [Profilering](incremental.md#mät-byggkedjans-faser).

Grafikexemplen (Metal, Vulkan, WebGPU), CSV-verktyget Tabell och
HTTP/1.1-servern underhålls i syskonrepot
[dunder-hundmat](https://github.com/Mik-pe/dunder-hundmat).
Se även [macOS-appar](app-bundles.md).

## WebAssembly (senarelagt under språkutveckling)

Wasm-backenden finns kvar i `selfhost/backend/wasm_*.ddr`, men Wasm-testsviter,
`wasm32`-byggen i testmatrisen och externa JS-runtimes (Node/Bun) har tagits
bort ur verifieringskedjan och **senarelagts** under pågående språkutveckling.
All aktiv språk- och kompilatorverifiering (`./tools/dev.sh` och
`./tools/verify-selfhost.sh`) fokuserar tills vidare på de tre nativa målen
(`arm64-macos`, `arm64-linux` och `x86_64-linux`) utan JS-beroenden.

Wasm-moduler som behöver filer, processer eller grafik måste deklarera
motsvarande värdimporter. Det publika ABI:t använder fyra byte för
pekare och `usize`, medan kompilatorn internt använder 64-bitars registerceller.
Tokens, lånekontroll och logiken för kopiering och frigöring delas med
nativmålen. [Wasm-ABI och utskrifter](targets.md#webassembly)
