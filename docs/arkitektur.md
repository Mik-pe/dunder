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
registerallokering. Generella livstider, explicita flyttar och registerallokering
för hela språket är fortsatt designmål. Nativebyggen
kan använda [funktionscachen](incremental.md).

## Språkets kostnadsmodell

Ett värde har en statiskt känd typ och layout. Första kärnan innehåller heltal med bestämd bredd, booleska värden, poster, fasta arrayer, slices, funktioner och strukturerat kontrollflöde. En slice består av en adress och en längd och lånar sin lagring. Anrop är direkta när målet är känt. Allokering, kopiering av ägda resurser och dynamisk dispatch ska gå att se där de väljs. Ett litet standardbibliotek får bero på operativsystemets gränssnitt.

För vanlig heltalsaritmetik föreslår vi kontrollerat spill, med explicit omslagsaritmetik för exempelvis hashning. Felaktiga index och division med noll ska ha definierat beteende. Samma program har samma säkerhetsregler i alla byggprofiler. Bevisar kompilatorn att ett index ryms får den ta bort kontrollen.

## Minne utan GC

Den implementerade modellen kombinerar **unika arraybuffertar, värdekopior,
lexikal städning och referensräknade oföränderliga strängar**. Tilldelning
ersätter och frigör gamla resurser; scope-slut, `break`, `continue` och retur
städar ägda värden. Nästlade structar och aktiva enumpayloads städas rekursivt.
String-, byte- och runevyer behåller strängen tills sista ägaren försvinner.
Referensräkningen är icke-atomär och omfattar enbart oföränderliga löv utan
cykler. Det reviderar det tidigare designmålet att helt undvika
referensräkning: strängkopior behåller nu billig delning och säker livslängd
utan att hålla kvar alla strängar till processlut. Ingen spårande GC behövs.
Läs [layout, allokering, tester och gränser](memory.md).

Explicita flyttar, användarvalda allokatorer och generella livstider är nästa
designsteg. Tabellen beskriver målen för den fulla lånemodellen; dagens
referenser är begränsade till funktionsparametrar.

| Situation | Vad kompilatorn ska tillåta eller upptäcka |
| --- | --- |
| Läsa en buffer från två funktioner | Flera samtidiga läslån är giltiga. |
| Skriva genom en referens | Ett exklusivt lån ger ensam åtkomst till den lånade platsen under lånets giltighetstid. |
| Flytta, frigöra eller växa en buffer medan en slice lever | Avvisa. Alla tre operationerna kan ogiltigförklara adressen. |
| Returnera en referens | Signaturen binder resultatet till ett inlån eller statisk lagring. En referens till en lokal variabel avvisas. |
| En gren flyttar ett värde och den andra behåller det | Läsning efter grenen kräver att värdet är initierat på båda vägarna. Städningen frigör precis de resurser som respektive väg fortfarande äger. |
| Nollställa en arena | Kräver exklusiv åtkomst till arenan. Alla lån ur den måste ha upphört. |
| Gå via en råpekare eller ett externt C-anrop | Kräver en avgränsad `unsafe`-operation med ett dokumenterat kontrakt. Säkra anropare får bara se ett säkert gränssnitt om omslaget upprätthåller kontraktet. |

Typkontrollen för ett funktionsanrop behöver bara den anropade funktionens signatur, inklusive lånens ursprung. Inuti en funktion håller kontrollen reda på initiering, flyttar och aktiva lån. Första versionen lånar hela lokala värden och förbjuder partiell flytt. Vid en förgrening förenas tillstånden konservativt. Varje bakåtkant i en loop måste återställa de ägar- och lånevillkor som krävs vid loopens början. En tidig retur ska städa alla fortfarande ägda resurser i omvänd ordning. Detta kan uttryckas på den strukturerade källkoden och vid behov med en lokal initieringsflagga i maskinkoden.

Säker kod ska även hindra läsning av oinitierat minne, ogiltiga taggar och osynkroniserade data races. Första delmängden är enkeltrådad och tillåter varken delat muterbart globalt tillstånd, referenser gömda genom heltalskonvertering eller användardefinierade destruktorer. För trådar behöver vi senare specificera överföring av ägarskap och synkronisering innan ett säkert tråd-API kan släppas. Säkerheten vilar också på att kompilatorn och den lilla osäkra kärnan i allokator, containers och plattformsanrop är korrekta.

## Direkt från funktion till ARM64

1. **Läs moduler och samla deklarationer.** Varje fil läses till en sammanhängande buffer. Namn interneras och källpositioner sparas som offset. Modulernas exporterade typer och funktionssignaturer fastställs före funktionskropparna. Det gör framåtreferenser och ömsesidiga funktionsanrop möjliga.
2. **Bearbeta en funktion åt gången.** Bygg ett kompakt syntaxträd i tillfälligt minne, lös namn, sätt typer och kontrollera ägarskap. Samma träd bär källpositioner och de fakta som kodgeneratorn behöver. Frigör arbetsminnet när funktionen är klar. Oberoende funktioner kan hanteras av olika arbetare.
3. **Optimera lokalt och välj AArch64-instruktioner.** Den implementerade skalära delmängden använder en kompakt typad funktions-IR för förenkling, loopbevis och registerallokering. Övriga kroppar använder den befintliga kombinerade kontroll- och emissionsvägen. Skriv instruktionsord i funktionens kodbuffer och spara lokala etiketter och korrigeringsposter för avstånd som ännu är okända. Ingen helprograms-IR, SSA eller bytekod byggs.
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

Grafikbindningarna följer med exempelprogrammens källkod. Dunder-modulerna
i `dogfood/graphics/` beskriver bibliotekens C-layout, slår upp
funktionspekare och äger fönster-, renderings- och händelselogiken. WebGPU-exemplet
i `dogfood/webgpu/` anropar sin JavaScript-värd via en vanlig Wasm-import.
Kompilatorns del av grafikanropen är det generella FFI-stödet och
kodgenereringen för målet. Se [FFI](ffi.md), [Metal-exemplet](macos-metal.md)
och [Vulkan-exemplet](linux-vulkan.md).

Maskindelen äger instruktionskodning och räckvidd för hopp. Plattformslagret
anpassar startpunkt, systemanrop, metadata-layout och binärformat. ARM64-målen
delar instruktionskodning; x86-64 sänker samma interna instruktionsvokabulär
till nativekod vid kompilering. Dunders interna funktions-ABI är inte ett
publikt C-ABI. Se [målplattformarna](targets.md) för verifiering och begränsningar.

## Byggkontraktet

`dunder build` hittar närmaste `dunder.toml` från arbetskatalogen uppåt. Första manifestet behöver bara programnamn och rotmodul. Rotmodulens uttryckliga importer bestämmer vad som byggs. Lokala beroenden räcker för första versionen. Standardkommandot bygger optimerat för värddatorns stödda mål och skriver till `build/<mål>/<program>`. Det kräver varken nätverk, bakgrundsprocess eller användarskrivna byggskript. `--target arm64-macos` väljer målet uttryckligen. `--timings` redovisar faser och återanvändning. Ett fel ger källposition, felkod från kommandot och inget halvskrivet resultat.

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
