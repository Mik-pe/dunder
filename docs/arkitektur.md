# Dunders arkitektur

Dunder skriver ARM64/macOS, ARM64/Linux, x86-64/Linux, `wasm32`, `wasm32-wasi`
och `x86_64-windows` (PE32+) med explicit målval. ELF, Mach-O, PE32+, systemanrop
och kodgenerering ingår i kompilatorn. Källfunktionerna går genom verifierad MIR. ARM64-backenden emitterar maskinord,
x86-64/Linux använder `x86_mir` och Wasm-målen använder `wasm_mir`.
Windows använder x86-sänkningen med Win64-ABI. Läs [målplattformar och begränsningar](targets.md).

Grundkraven är native maskinkod, en egen verktygskedja, minnessäkerhet utan
GC, snabb kompilering och självhosting. [Syntax](syntax.md) · [Fortsatt utveckling](next-steps.md)
Automatisk frigöring för arrayer och strängar, POD-värden och lokal/helfunktions
optimering (MIR-SSA, helfunktionsinlining, `BorrowHome`) ingår som standard.
Byggen för macOS och Linux kan bädda in DWARF v4-debuginformation (`-g`/`--debug`)
samt använda [funktionscachen](incremental.md). `dunder lsp` föreslår
`mut` vid tilldelning till en oföränderlig lokal variabel. När en importerad
modul redigeras uppdateras diagnostiken även i de öppna filer som beror på den.

## Språkets kostnadsmodell

Ett värde har en statiskt känd typ och layout. Språket har bland annat heltal med bestämd bredd, booleska värden, poster, fasta arrayer, slices, funktioner och strukturerat kontrollflöde. En slice består av en adress och en längd och lånar sin lagring. Anrop är direkta när målet är känt. Allokering, kopiering av ägda resurser och dynamisk dispatch ska gå att se där de väljs. Ett litet standardbibliotek får bero på operativsystemets gränssnitt.

Vanlig heltalsaritmetik kontrollerar overflow. Metoder som `wrapping_add` ger explicit omslagsaritmetik för exempelvis hashning. Felaktiga index och division med noll ska ha definierat beteende. Samma program har samma säkerhetsregler i alla byggprofiler. Bevisar kompilatorn att ett index ryms får den ta bort kontrollen.

## Minne utan GC

Den implementerade modellen kombinerar **unika arraybuffertar, värdekopior,
lexikal städning och referensräknade oföränderliga strängar**. Tilldelning
ersätter och frigör gamla resurser; scope-slut, `break`, `continue` och retur
städar ägda värden. Nästlade structar och aktiva enumpayloads städas rekursivt.
Strängkloner och snapshotiteratorer behåller sin strängägare; lokala
intervallvyer lånar i stället originalets lagring till blockslut.
Referensräkningen är icke-atomär och omfattar enbart oföränderliga löv utan
cykler. Läs [layout, allokering, tester och gränser](memory.md).

Lånemodellen omfattar funktionsparametrar, lokala läslån och delade
returvärden vars livstid hör till en angiven parameter.

| Situation | Vad kompilatorn ska tillåta eller upptäcka |
| --- | --- |
| Läsa en buffer från två funktioner | Flera samtidiga läslån är giltiga. |
| Skriva genom en referens | Ett exklusivt lån ger ensam åtkomst till den lånade platsen under lånets giltighetstid. |
| Flytta, frigöra eller växa en buffer medan en slice lever | Avvisa. Alla tre operationerna kan ogiltigförklara adressen. |
| Returnera en referens | Kontrollera att alla returvägar leder till signaturens källparameter. Bind anroparens lån till motsvarande argument. |
| En gren flyttar ett värde och den andra behåller det | Läsning efter grenen kräver att värdet är initierat på båda vägarna. Städningen frigör precis de resurser som respektive väg fortfarande äger. |
| Nollställa en arena | Kräver exklusiv åtkomst till arenan. Alla lån ur den måste ha upphört. |
| Gå via en råpekare eller ett externt C-anrop | Kräver en avgränsad `unsafe`-operation med ett dokumenterat kontrakt. Säkra anropare får bara se ett säkert gränssnitt om omslaget upprätthåller kontraktet. |

Anropens typer kontrolleras mot signaturen. Lånens ursprung och möjliga
råpekarexponering följs dessutom genom en gemensam interprocedural analys.
Inuti funktionen skiljs initiering och flyttar från aktiva lån. Structfält
har separata initieringstillstånd; lokala fältlån tillåter åtkomst till
oberoende syskon, medan arrayelement låser hela sin array. Lån under anrop
kontrolleras fortsatt konservativt per rot. Grenar förenas konservativt och
varje loopbakåtkant måste återställa de ägarvillkor som krävs vid loopens
början. En tidig retur städar fortfarande ägda resurser i omvänd ordning.

### Initiering per lagringsplats

`ownership_state` kanoniserar lokaler och fältkedjor i ett glest träd per
funktion. Upprepade källåtkomster till samma fält får samma ägaridentitet,
medan `typed_places` behåller separata rader för varje typad åtkomst och
dess emission. Typträdet expanderas aldrig rekursivt i förväg. En läsning,
ett lån eller en flytt av hela platsen kräver initierade förfäder och
ättlingar; traversering till ett syskon kräver bara giltiga förfäder.

Checkpoints kopierar tillstånd och platsidentiteter utan att kopiera den
aktiva loopstacken. Schemat växer monotoniskt genom grenar. Fält som
upptäcks efter ett checkpoint ärver begränsningarna från sina förfäder.
Samma regel gäller bakåtkanter och `break`, även om roten redan var
partiellt flyttad vid loopinträdet. Tilldelning återställer exakt sitt
subträd. `defer` behåller tills vidare ett konservativt lås på hela roten.

Källans initieringsbevis är separat från runtimeägare och MIR-optimering.
En managed-fältflytt nollställer bara fältets ägarslot; befintlig rekursiv
cleanup städar resten. Inga per-fältflaggor behövs vid körning. `replace`
återanvänder samma typade läs-, skriv- och ägaroperationer: båda operanderna
slutförs före bytet, den tidigare ägaren blir resultat och destinationen
förblir giltig. Lokala lån återanvänder dessa platsidentiteter.

### Proveniens och lexikala läslån

`local_borrows.Table` lagrar proveniens och aktiva lokala lån med kanoniska
ägar-ID:n från `ownership_state`. Tabellen har ingen egen initieringsmodell.
Alias härleds till den ursprungliga platsen; ett nytt återlån skapar ett
eget aktivt lån på samma ägare. Referenserna är oföränderliga och frigör sitt
lås efter sista lexikala läsningen i samma block och loopdjup (eller vid
blockets slut när ett `defer` eller en inre loop ändrar ägaren). Skrivning,
skrivlån, flytt och `replace` använder samma överlappningsfråga. Ett lån av
ett fält blockerar dess förfäder och ättlingar men tillåter syskonfält.
Arrayindex och intervall följer konservativt arrayens identitet.

`borrowed_view` emitterar intervallkontroller och en framebaserad deskriptor
för originalets data; den registrerar ingen runtimeägare. Arrayvyer har
typen `&[]T` och strängvyer typen `&str` (med `.as_bytes()` för `&[]u8`).
Gränser kontrolleras före textens UTF-8-gränser, med status 103 respektive
106. Tomma textvyer kräver också giltiga UTF-8-gränser. Konstruktionen
allokerar ingen heapbuffert och gör ingen retain; `.clone()` begär en ägare
uttryckligen. Temporära lån vid anrop lever genom hela anropet och kan
inte bindas lokalt. Scratchlagring kan ännu bara lånas under anrop.

Proveniens tillhör ett färdigt uttrycksresultat. Ett nytt ägt anropsresultat,
en konstruktor eller ett operatorresultat nollställer argumentens
lånemetadata i `FunctionContext.loan` (`LoanState`), bland annat `loan.home`, så att det inte ärver en inre vys
lagringsplats. Projektioner bevarar däremot sitt verkliga ursprung. Den
befintliga globala escapegrafen för lån återanvänds även för lokala lån och
indirekta anrop. Nya uttrycksformer måste fånga analysen före emission;
saknas förberett proveniensbevis avvisas programmet. Nya lån får inte nå en
råpekarexponerande funktion. Direkt `addr` i explicit `unsafe` kan användas
för att verifiera adressidentitet.

Funktionscachen hoppar inte över dessa kontroller: källkroppar och lån
kontrolleras även när färdig maskinkod kan återanvändas. Kompilatorns fragmentcache lånar tokenstavningar
och läser nyckel, signatur och nyttolast som vyer ur en bytebuffert.
[Körbara låneprov](../tools/test_local_borrows.ddr).

På macOS och Linux kan isolerade uppgifter köras på flera trådar.
Typkontrollen begränsar vilka data och effekter en uppgift får använda.
Runtimen kopierar indata och resultat mellan separata allokeringsarenor.
[Uppgifter och ägarskap](tasks.md)

## Från funktion till maskinkod

[Modulladdaren](../selfhost/driver/modules.ddr) löser importer och bevarar
ursprungliga källpositioner. Deklarationernas signaturer fastställs före
kropparna så att funktioner kan anropa varandra rekursivt.

En generisk instans identifieras av deklarationen och de konkreta
typargumentens identiteter. Samordnaren upptäcker instanserna före parallell
kodgenerering. [Design för generics](generics-design.md)

Den seriella och den parallella kompilatorn använder samma typkontroll. En
arbetare kan typkontrollera en kropp med en fryst bild av deklarationerna.
Samordnaren binder symboler och håller reda på nåbara funktioner i bestämd
ordning. Oanropade kroppar kontrolleras också. [Kompilatorns moduler](../selfhost/README.md)
· [Parallell kompilering](tasks.md#kompilatorn-som-taskkonsument)

En skalär funktion kan förenklas i en lokal typad modell. Varje nyemitterad
källfunktion går sedan genom samma MIR-verifiering och sänkning. Länkaren
placerar nåbara funktioner, datasegment och importer i deterministisk ordning.
Mach-O-skrivaren signerar den slutliga bilden. [Kodgenerering](optimization.md#gemensam-helfunktionskedja)

## Verktygskedjan och målplattformarna

Native-målen är `arm64-macos`, `arm64-linux`, `x86_64-linux` och `x86_64-windows` (PE32+).
Wasm-målen är `wasm32` och `wasm32-wasi`. Dunder äger parser,
typkontroll, kodgenerering, relokeringar, bildskrivning (Mach-O, ELF64, PE32+, Wasm), DWARF v4-debuginfo, macOS ad hoc-signering
och byggdrivrutin (`dunder build`/`run`/`test`/`check`/`fmt`/`lsp`/`install`/`upgrade`). Ett vanligt Dunderbygge kör aldrig LLVM, GCC, en extern
assembler, länkare eller signerare. Vanliga Linuxbilder använder direkta
systemanrop utan libc. Ett program med nåbara `@extern`-deklarationer får
en dynamisk ELF med de bibliotek och symboler som deklarationerna anger.
På Wasm blir deklarationerna typade värdimporter (eller WASI-importer under `wasm32-wasi`).

Kompilatorn bäddar in standardmodulerna som räknas upp i
[stdlib/modules.txt](../stdlib/modules.txt). När självhostens inbäddade
källtext regenereras läser [selfhost/embed.ddr](../selfhost/embed.ddr)
samma lista. Vanliga filimporter löses på disk relativt den importerande filen.

Grafikexemplen (Metal, Vulkan, WebGPU) i testbädden `dunder-hundmat` provar
Dunders FFI och kodgenerering: de beskriver bibliotekens C-layout, slår upp
funktionspekare och äger fönster-, renderings- och händelselogiken, och
WebGPU-exemplet anropar sin JavaScript-värd via en vanlig Wasm-import.
[Dunders FFI-kontrakt](ffi.md) · [dunder-hundmat](https://github.com/Mik-pe/dunder-hundmat)

Maskindelen äger instruktionskodning och räckvidd för hopp. Plattformslagret
anpassar startpunkt, systemanrop, metadata-layout och binärformat. ARM64-målen
delar instruktionskodning; x86-64/Linux (`x86_mir`) sänker MIR direkt till fysiska register
med System V-ABI, medan x86_64-windows sänker samma MIR-producerade symboliska ström
till Win64 via x86-sänkningen, och Wasm (`wasm_mir`) sänker MIR direkt till typade
Wasm-lokaler och strukturerat kontrollflöde. Dunders interna funktions-ABI är inte ett
publikt C-ABI. Se [målplattformarna](targets.md) för verifiering och begränsningar.

## Byggkontraktet

`dunder build` hittar närmaste `dunder.toml` från arbetskatalogen uppåt. Första manifestet behöver bara programnamn och rotmodul. Rotmodulens uttryckliga importer bestämmer vad som byggs. Beroenden kan vara lokala kataloger eller [Git-källor med låsta commits](dependencies.md). Standardkommandot bygger optimerat för värddatorns stödda mål och skriver till `build/<mål>/<program>`. Lokala och cachade beroenden kan byggas utan nätverk; `--offline` förbjuder hämtning. Git-källor hämtas med Git, utan bakgrundsprocess eller användarskrivna byggskript. `--target arm64-macos` väljer målet uttryckligen. `--timings` redovisar faser och återanvändning. Ett fel ger källposition, felkod från kommandot och inget halvskrivet resultat.

Med `--incremental` lagras maskinkodsfragment och symboliska korrigeringsposter.
Cacheidentiteten omfattar den körande kompilatorn.
Kompilatorn kontrollerar hela källträdet inför varje bygge. Tester jämför
varma och rena bilder byte för byte på de tre bootstrapmålen.
[Användning och cachekontrakt](incremental.md).

## Återbyggnad

Ett verifierat Dunder-frö för värdplattformen bygger först kompilatorns
källträd under `selfhost/`. Den nya binären bygger samma träd igen.
Två efterföljande byggen måste bli byteidentiska innan skriptet publicerar
kompilatorn. Den kan därefter bygga program för alla sex mål.
[Frön och återbyggnad](../bootstrap/README.md).

Wasm-backenden ([`wasm_mir`](../selfhost/backend/wasm_mir.ddr)) sänker samma
verifierade MIR som native direkt till Wasm. Den omvandlar hopp till
Wasm-block, anrop till Wasm-funktioner och minnesåtkomster till operationer
på modulens linjära minne. Nåbara externa deklarationer blir typade importer.
Filer och processer hör till
nativevärdens API.
