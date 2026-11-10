# Dunders arkitektur

**Plattformsstöd 2026-11-10:** bootstrapen skriver nu ARM64/macOS,
ARM64/Linux och x86-64/Linux med explicit målval. ELF, Mach-O, systemanrop
och kodgenerering ingår i kompilatorn. x86-64 sänker den delade interna
instruktionsvokabulären till nativekod vid kompilering; det finns ingen
runtimeinterpreter. Läs [målplattformar och begränsningar](targets.md).

Grundkraven är native maskinkod, en egen verktygskedja, minnessäkerhet utan
GC, snabb kompilering och självhosting. Implementerad syntax och aktuella
begränsningar finns i [syntaxen](syntax.md) och [statusen](status.md).
Ägarmodellen, registeroptimeringarna och den inkrementella cachen nedan är
fortsatt designmål, inte färdiga funktioner.

## Språkets kostnadsmodell

Ett värde har en statiskt känd typ och layout. Första kärnan innehåller heltal med bestämd bredd, booleska värden, poster, fasta arrayer, slices, funktioner och strukturerat kontrollflöde. En slice består av en adress och en längd och lånar sin lagring. Anrop är direkta när målet är känt. Allokering, kopiering av ägda resurser och dynamisk dispatch ska gå att se där de väljs. Ett litet standardbibliotek får bero på operativsystemets gränssnitt.

För vanlig heltalsaritmetik föreslår vi kontrollerat spill, med explicit omslagsaritmetik för exempelvis hashning. Felaktiga index och division med noll ska ha definierat beteende. Samma program har samma säkerhetsregler i alla byggprofiler. Bevisar kompilatorn att ett index ryms får den ta bort kontrollen.

## Minne utan GC

Föreslagen första modell är **unikt ägarskap, flytt av värden och lexikalt avgränsade lån**. Stackvärden lever till slutet av sitt scope. Heapminne begärs från en uttrycklig allokator och hålls av ett ägarvärde. När ägaren lämnar sitt scope frigörs resursen deterministiskt. En flytt överför det ansvaret. Referensräkning ingår inte i grundmodellen.

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
3. **Välj riktiga AArch64-instruktioner direkt.** Gå över det typade trädet, välj register eller stackplatser och skriv instruktionsord i funktionens kodbuffer. Spara lokala etiketter och korrigeringsposter för avstånd som ännu är okända. Vi bygger ingen generell IR, MIR, SSA eller bytekod. Registertabeller, källpositioner och olösta hopp är data med konkreta uppgifter i backend.
4. **Lägg ut programmet en gång.** Dunders länkare placerar funktioner och konstantdata i deterministisk ordning, löser symboler och fixar hopp och adresser. Ett hopp utanför instruktionsräckvidden får en längre sekvens eller en närliggande hoppstubbe. Bildskrivaren skapar och ad hoc-signerar Mach-O-filen över dess slutliga byte. Publicera resultatet med atomiskt namnbyte när bygget lyckats.

Första backendversionen håller tillfälliga värden i register och ger värden som lever över en gren en känd hemplats. Vid anrop respekteras register som anroparen måste spara. Nästa steg är att behålla lokala värden i register över strukturerade loopar och grenar där alla inkommande vägar kan enas om placeringen. Kompilatorn får använda begränsad analys per funktion. En permanent andra modell av hela programmet skulle däremot bryta mot designen.

**Optimering i standardbygget.** Börja med laglig konstantvikning, döda konstanta grenar, immediater och ARM64:s adresseringslägen, direkt jämförelse följd av villkorligt hopp och borttagning av överflödiga registerflyttar. Prioritera därefter loopars registeranvändning och bevisat överflödiga gränskontroller utifrån mätningar. En optimering måste bevara evalueringsordning, sidoeffekter, lån och möjliga fällor. Exempelvis får vikning av multiplikation med noll inte kasta bort ett anrop med sidoeffekter. Inlining får en storleksbudget, och konsumenten måste bero på den inlinade kroppen. SIMD, mer aggressiv inlining och specialisering prövas mot uppmätta flaskhalsar. C/C++-paritet är en prövbar ambition, särskilt svår på vektoriserbara loopar.

## Verktygskedjan och målplattformarna

Målen är `arm64-macos`, `arm64-linux` och `x86_64-linux`. Dunder äger parser,
typkontroll, kodgenerering, relokeringar, bildskrivning, macOS ad hoc-signering
och byggdrivrutin. Ett vanligt Dunderbygge kör aldrig LLVM, GCC, en extern
assembler, länkare eller signerare. Linuxbilderna använder direkta systemanrop
utan libc. Externa verktyg får inspektera eller jämföra resultat i tester.

Maskindelen äger instruktionskodning och räckvidd för hopp. Plattformslagret
anpassar startpunkt, systemanrop, metadata-layout och binärformat. ARM64-målen
delar instruktionskodning; x86-64 sänker samma interna instruktionsvokabulär
till nativekod vid kompilering. Dunders interna funktions-ABI är inte ett
publikt C-ABI. Se [målplattformarna](targets.md) för verifiering och begränsningar.

## Byggkontraktet

`dunder build` hittar närmaste `dunder.toml` från arbetskatalogen uppåt. Första manifestet behöver bara programnamn och rotmodul. Rotmodulens uttryckliga importer bestämmer vad som byggs. Lokala beroenden räcker för första versionen. Standardkommandot bygger optimerat för värddatorns stödda mål och skriver till `build/<mål>/<program>`. Det kräver varken nätverk, bakgrundsprocess eller användarskrivna byggskript. `--target arm64-macos` väljer målet uttryckligen. `--timings` redovisar faser och återanvändning. Ett fel ger källposition, felkod från kommandot och inget halvskrivet resultat.

En första implementation får bygga om allt. Nästa version lagrar maskinkodsfragment och deras symboliska korrigeringsposter. Nyckeln omfattar kompilatorversion, mål och ABI, bygginställningar, eget källinnehåll och relevanta beroendens fingeravtryck. En intern ändring i en anropad funktion kräver normalt bara omkompilering av den funktionen och omlänkning. Ändrad typ-layout, signatur, inlinad kropp eller använd exporterad konstant ogiltigförklarar också konsumenterna. Ändringar i modulers namnutbud måste fångas vid namnupplösningen. En ren och en inkrementell byggning av samma källträd ska ge exakt samma binär, även med olika antal byggarbetare.

## Vägen till en självhostad kompilator

Rust-bootstrapen bygger den Dunder-skrivna kompilatorn i `selfhost/`.
Den bygger redan skalära program till ARM64/macOS, men saknar ännu stöd
för sina egna importer och aggregat. Nästa steg är att implementera dessa
språkdelar och sedan låta kompilatorn bygga sig själv i två på varandra
följande steg. Resultaten ska vara byteidentiska och klara språktesterna.
Återställning från ren källkod och verifieringskraven finns i
[bootstrapguiden](../selfhost/README.md).

Rust-bootstrapen är den fungerande återställningsvägen. Generics,
metaprogrammering, pakethantering och ett publikt stabilt Dunder-ABI återstår.
Prestanda bedöms med [mätkontraktet](prestanda.md).
