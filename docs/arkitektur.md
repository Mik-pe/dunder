# Dunders arkitektur

**Plattformsstöd 2026-11-10:** Dunder skriver ARM64/macOS, ARM64/Linux, x86-64/Linux och wasm32 med explicit målval. ELF, Mach-O, systemanrop
och kodgenerering ingår i kompilatorn. x86-64 sänker den delade interna
instruktionsvokabulären till nativekod vid kompilering; det finns ingen
runtimeinterpreter. Läs [målplattformar och begränsningar](targets.md).

Grundkraven är native maskinkod, en egen verktygskedja, minnessäkerhet utan
GC, snabb kompilering och självhosting. Implementerad syntax och aktuella
begränsningar finns i [syntaxen](syntax.md) och [statusen](status.md).
Automatisk frigöring för arrayer och strängar samt lokal maskinkodsoptimering
är implementerade i kompilatorns kodgenerering. Anropsfria skalära funktioner
kan använda en typad lokal modell, begränsat optimeringspass och
registerallokering. Generella livstider och registerallokering
för hela språket är fortsatt designmål. Nativebyggen
kan använda [funktionscachen](incremental.md).

## Språkets kostnadsmodell

Ett värde har en statiskt känd typ och layout. Första kärnan innehåller heltal med bestämd bredd, booleska värden, poster, fasta arrayer, slices, funktioner och strukturerat kontrollflöde. En slice består av en adress och en längd och lånar sin lagring. Anrop är direkta när målet är känt. Allokering, kopiering av ägda resurser och dynamisk dispatch ska gå att se där de väljs. Ett litet standardbibliotek får bero på operativsystemets gränssnitt.

Vanlig heltalsaritmetik kontrollerar overflow. Metoder som `wrapping_add` ger explicit omslagsaritmetik för exempelvis hashning. Felaktiga index och division med noll ska ha definierat beteende. Samma program har samma säkerhetsregler i alla byggprofiler. Bevisar kompilatorn att ett index ryms får den ta bort kontrollen.

## Minne utan GC

Den implementerade modellen kombinerar **unika arraybuffertar, värdekopior,
lexikal städning och referensräknade oföränderliga strängar**. Tilldelning
ersätter och frigör gamla resurser; scope-slut, `break`, `continue` och retur
städar ägda värden. Nästlade structar och aktiva enumpayloads städas rekursivt.
Strängkloner och snapshotiteratorer behåller sin strängägare; lokala
intervallvyer lånar i stället originalets lagring till blockslut.
Referensräkningen är icke-atomär och omfattar enbart oföränderliga löv utan
cykler. Det reviderar det tidigare designmålet att helt undvika
referensräkning: strängkopior behåller nu billig delning och säker livslängd
utan att hålla kvar alla strängar till processlut. Ingen spårande GC behövs.
Läs [layout, allokering, tester och gränser](memory.md).

Den implementerade lånemodellen omfattar funktionsparametrar och lexikala,
oföränderliga lokala läsreferenser. Användarvalda allokatorer, lokala skrivlån
och signaturer för returnerade lån återstår.

| Situation | Vad kompilatorn ska tillåta eller upptäcka |
| --- | --- |
| Läsa en buffer från två funktioner | Flera samtidiga läslån är giltiga. |
| Skriva genom en referens | Ett exklusivt lån ger ensam åtkomst till den lånade platsen under lånets giltighetstid. |
| Flytta, frigöra eller växa en buffer medan en slice lever | Avvisa. Alla tre operationerna kan ogiltigförklara adressen. |
| Returnera en referens | Avvisas tills ett uttryckligt livstidskontrakt kan binda resultatet till indata eller statisk lagring. |
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
lås vid deklarationsblockets slut, inte vid sista användningen. Skrivning,
skrivlån, flytt och `replace` använder samma överlappningsfråga. Ett lån av
ett fält blockerar dess förfäder och ättlingar men tillåter syskonfält.
Arrayindex och intervall följer konservativt arrayens identitet.

`borrowed_view` emitterar intervallkontroller och en framebaserad deskriptor
för originalets data; den registrerar ingen runtimeägare. Arrayvyer har
typen `&[]T`, textvyer `&[]u8`. Längd och kapacitet är intervallängden.
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
kontrolleras även när färdig maskinkod kan återanvändas. HTTP-servern i
`dunder-hundmat` använder lokala path- och bodyvyer i stället för separata
offsetpar vid API-gränsen. Kompilatorns fragment-cache lånar tokenstavningar
och läser nyckel, signatur och payload som lokala vyer ur en bytebuffert.
Hashningen bygger uttryckligen sin ägda indatabuffert; wireformatet är
oförändrat. Cacheproven verifierar 24 byteidentiska kalla/varma bilder, åtta
värdkörningar och 166 atomiska avvisningar, inklusive ogiltig UTF-8.
Detta är korrekthets- och representationskontrakt, utan nya prestandaanspråk.
[Körbara låneprov](../tools/test-local-borrows.py).

Säker kod ska även hindra läsning av oinitierat minne, ogiltiga taggar och osynkroniserade data races. Första delmängden är enkeltrådad och tillåter varken delat muterbart globalt tillstånd, referenser gömda genom heltalskonvertering eller användardefinierade destruktorer. För trådar behöver vi senare specificera överföring av ägarskap och synkronisering innan ett säkert tråd-API kan släppas. Säkerheten vilar också på att kompilatorn och den lilla osäkra kärnan i allokator, containers och plattformsanrop är korrekta.

## Direkt från funktion till ARM64

1. **Läs moduler och samla deklarationer.** Varje fil läses till en sammanhängande buffer. Namn interneras och källpositioner sparas som offset. Modulernas exporterade typer och funktionssignaturer fastställs före funktionskropparna. Det gör framåtreferenser och ömsesidiga funktionsanrop möjliga.
2. **Typkontrollera och välj instruktioner per funktion.** Huvudtråden löser namn, kontrollerar typer och ägarskap och håller reda på nåbara funktioner. Den skalära delmängden använder en typad lokal modell för förenkling, loopbevis och registerallokering. Uttryckskompilatorn hanterar även anrop och ägda värden. Båda vägarna använder samma regler för kontrollerad addition och subtraktion. [Heltalsemittern](../selfhost/backend/integer_emit.ddr)
3. **Verifiera och sänk funktionskoden.** Maskinord, etiketter, relokeringar och källannotationer lagras i separata kolumner. Vid parallella byggen på ARM64/macOS får varje worker spann för sina funktioner och bygger deras MIR i lokalt arbetsminne. Den verifierar, optimerar och sänker varje funktion till ett objekt. Huvudtråden tar emot och validerar objekten i källordning. [Funktionskedjan](optimization.md#gemensam-helfunktionskedja)
4. **Lägg ut programmet en gång.** Dunders länkare placerar funktioner och konstantdata i deterministisk ordning, löser symboler och fixar hopp och adresser. Ett hopp utanför instruktionsräckvidden får en längre sekvens eller en närliggande hoppstubbe. Bildskrivaren skapar och ad hoc-signerar Mach-O-filen över dess slutliga byte. Publicera resultatet med atomiskt namnbyte när bygget lyckats.

Den skalära vägen behåller lokaler i register över strukturerade loopar och
grenar med konservativa livstidsintervall och deterministiska spill. Anrop
och ägda värden återstår för denna väg. Kompilatorn använder begränsad analys
per funktion och återanvänder arbetsminnet; någon permanent andra modell av
hela programmet hålls inte. [Omfattning, budget och verifiering](optimization.md).

Kompilatorn kontrollerar även kroppar som saknar anropare. Endast nåbara
funktioner, datasegment och importplatser länkas in i programmet. Frigöring
av ägda lokaler och tillfälliga värden genereras vid respektive utgång ur
ett scope. [Kodgenerering och kontroller](optimization.md).

Självhosten i `selfhost/` är uppdelad i avgränsade moduler:
`frontend_types` (frontendtyper och symboltabeller), `ffi_abi`
(C-layout och plattforms-ABI), `task_certify` (statisk effekt- och
kloncertifiering för tasks), `compile_session` (kärntillstånd och
länksession), `compile` (seriell drivrutin och frontend/check) samt
`compile_parallel` (workerstyrd backend). Vid parallell kompilering
strömmar föräldern packade `mir_task.Batch`-enheter genom ett begränsat
`task scope`-fönster där arbetare kör `mir_pipeline` och `mir_lower` till
numerisk `ObjectData`, innan föräldern accepterar, cachar och länkar
resultaten deterministiskt i deklarationsordning.

## Verktygskedjan och målplattformarna

Native-målen är `arm64-macos`, `arm64-linux` och `x86_64-linux`.
Det fjärde målet är `wasm32`. Dunder äger parser,
typkontroll, kodgenerering, relokeringar, bildskrivning, macOS ad hoc-signering
och byggdrivrutin. Ett vanligt Dunderbygge kör aldrig LLVM, GCC, en extern
assembler, länkare eller signerare. Vanliga Linuxbilder använder direkta
systemanrop utan libc. Ett program med nåbara `@extern`-deklarationer får
en dynamisk ELF med de bibliotek och symboler som deklarationerna anger.
På Wasm blir deklarationerna typade värdimporter.

Kompilatorn bäddar in standardmodulerna som räknas upp i
[stdlib/modules.txt](../stdlib/modules.txt). När självhostens inbäddade
källtext regenereras läser [selfhost/embed.ddr](../selfhost/embed.ddr)
samma lista. Vanliga filimporter löses på disk relativt den importerande filen.

Grafikbindningarna ligger i syskonrepot `dunder-hundmat`: Dunder-modulerna
i `dogfood/graphics/` beskriver bibliotekens C-layout, slår upp
funktionspekare och äger fönster-, renderings- och händelselogiken. WebGPU-exemplet
i `dogfood/webgpu/` anropar sin JavaScript-värd via en vanlig Wasm-import.
Kompilatorns del av grafikanropen är det generella FFI-stödet och
kodgenereringen för målet. Se [FFI](ffi.md).

Maskindelen äger instruktionskodning och räckvidd för hopp. Plattformslagret
anpassar startpunkt, systemanrop, metadata-layout och binärformat. ARM64-målen
delar instruktionskodning; x86-64 sänker samma interna instruktionsvokabulär
till nativekod vid kompilering. Dunders interna funktions-ABI är inte ett
publikt C-ABI. Se [målplattformarna](targets.md) för verifiering och begränsningar.

## Byggkontraktet

`dunder build` hittar närmaste `dunder.toml` från arbetskatalogen uppåt. Första manifestet behöver bara programnamn och rotmodul. Rotmodulens uttryckliga importer bestämmer vad som byggs. Beroenden kan vara lokala kataloger eller [Git-källor med låsta commits](dependencies.md). Standardkommandot bygger optimerat för värddatorns stödda mål och skriver till `build/<mål>/<program>`. Lokala och cachade beroenden kan byggas utan nätverk; `--offline` förbjuder hämtning. Git-källor hämtas med Git, utan bakgrundsprocess eller användarskrivna byggskript. `--target arm64-macos` väljer målet uttryckligen. `--timings` redovisar faser och återanvändning. Ett fel ger källposition, felkod från kommandot och inget halvskrivet resultat.

Med `--incremental` lagras maskinkodsfragment och symboliska korrigeringsposter.
Cacheidentiteten omfattar den körande kompilatorn.
Kompilatorn kontrollerar hela källträdet inför varje bygge. Tester jämför
varma och rena bilder byte för byte på alla tre native-mål.
[Användning och cachekontrakt](incremental.md).

## Återbyggnad

Ett verifierat Dunder-frö för värdplattformen bygger först kompilatorns
källträd under `selfhost/`. Den nya binären bygger samma träd igen.
Två efterföljande byggen måste bli byteidentiska innan skriptet publicerar
kompilatorn. Den kan därefter bygga program för alla fyra mål.
[Frön och återbyggnad](../bootstrap/README.md).

Wasm-backenden använder samma typade funktioner och symboliska instruktioner
som native. Den omvandlar hopp till Wasm-block, anrop till Wasm-funktioner
och minnesåtkomster till operationer på modulens linjära minne. Nåbara
externa deklarationer blir typade importer. Filer och processer hör till
nativevärdens API.
