# Dunders första arkitektur

**Lägesändring 2026-11-10:** användaren prioriterade det riktiga typade
källspråket före fortsatt fröarbete. Den första Rust-bootstrapkompilatorn
och dess native backend är implementerade; se [syntaxen](syntax.md) och
[aktuell status](status.md). Stegordningen med ett självassemblerande frö
nedan beskriver den ursprungliga långsiktiga planen. Den blockerar inte
språkutvecklingen. Den implementerade syntaxen är nu ett första arbetsbeslut.

**Designförslag 0.1.** Grundkraven är ARM64-maskinkod, en egen verktygskedja, minnessäkerhet utan GC, snabb optimerad kompilering och självhosting. Nedan finns de arbetsbeslut vi behöver för att kunna bygga och pröva dem. Syntax och nyckelord får vänta.

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

## Verktygskedjan och första målplattformen

`arm64-macos` är första hela målet. Dunder äger parser, typ- och lånekontroll, AArch64-kodning, relokeringar, länkning, Mach-O-skrivning, lokal ad hoc-signering och byggdrivrutin. Ett vanligt bygge ska fungera på en Mac utan Xcode Command Line Tools. Det kör aldrig LLVM, GCC, en extern assembler, länkare eller signerare. Operativsystemets laddare och libSystem ingår i plattformen. Externa verktyg kan inspektera eller jämföra resultat i tester.

Maskindelen äger instruktionskodning och räckvidd för hopp. Darwin-delen äger anropskonvention, stack och registerregler, startpunkt, systemimporter och binärformat. Första provet behöver bara en process som returnerar en vald exitkod. Nästa prov gör ett vanligt anrop och därefter ett importerat libSystem-anrop. Kontrollera bland annat 16-bytejusterad stack, reserverat x18, bevarade register och argument som spills till stacken. Senare mål kan återanvända kodningen och byta ABI- och formatdelarna. [Apples ARM64-ABI](https://developer.apple.com/documentation/xcode/writing-arm64-code-for-apple-platforms) · [Apples laddare](https://github.com/apple-oss-distributions/dyld/blob/main/mach_o/UnsafeHeader.cpp) · [Signering av körbara filer på Apple Silicon](https://developer.apple.com/documentation/xcode/embedding-a-helper-tool-in-a-sandboxed-app)

## Byggkontraktet

`dunder build` hittar närmaste `dunder.toml` från arbetskatalogen uppåt. Första manifestet behöver bara programnamn och rotmodul. Rotmodulens uttryckliga importer bestämmer vad som byggs. Lokala beroenden räcker för första versionen. Standardkommandot bygger optimerat för värddatorns stödda mål och skriver till `build/<mål>/<program>`. Det kräver varken nätverk, bakgrundsprocess eller användarskrivna byggskript. `--target arm64-macos` väljer målet uttryckligen. `--timings` redovisar faser och återanvändning. Ett fel ger källposition, felkod från kommandot och inget halvskrivet resultat.

En första implementation får bygga om allt. Nästa version lagrar maskinkodsfragment och deras symboliska korrigeringsposter. Nyckeln omfattar kompilatorversion, mål och ABI, bygginställningar, eget källinnehåll och relevanta beroendens fingeravtryck. En intern ändring i en anropad funktion kräver normalt bara omkompilering av den funktionen och omlänkning. Ändrad typ-layout, signatur, inlinad kropp eller använd exporterad konstant ogiltigförklarar också konsumenterna. Ändringar i modulers namnutbud måste fångas vid namnupplösningen. En ren och en inkrementell byggning av samma källträd ska ge exakt samma binär, även med olika antal byggarbetare.

## Vägen till en självhostad kompilator

Förslag för att slippa lägga en befintlig kompilator i förtroendekedjan är ett litet granskbart ARM64-frö. Fröet är ett separat engångsarbete. Det kostar handkodning men gör det möjligt att börja på en ren Mac och att senare återbygga hela kedjan. Ett litet frö hjälper bara om resten av kedjan kan reproduceras.

| Etapp | Vad vi bygger | Bevis innan nästa etapp |
| --- | --- | --- |
| 0. Första binären | En minimal ARM64/Mach-O-bild med förklarade instruktionsord, layout och ad hoc-signatur. En enkel bytebeskrivning får materialiseras med ett hexverktyg. | Binären körs på en fysisk Apple Silicon-Mac och ger förväntad exitkod. Bilden kan återskapas byte för byte. |
| 1. Fröverktyget | Ett litet ARM64-verktyg som kodar ett begränsat urval instruktioner, löser etiketter och skriver körbara signerade bilder. Dess första binär checkas in tillsammans med läsbar källa, bytebeskrivning, kontrollsumma och återbyggnadsinstruktion. | Fröet bygger sin egen källa identiskt och bygger ett andra program som gör ett funktionsanrop. Inga externa kompilatorverktyg i körvägen. |
| 2. Första vertikala snittet | Bygg en minimal bootstrapkompilator med fröverktyget. Implementera `dunder build` för en källfil med heltalsretur, därefter lokaler, funktioner och grenar. | En ändring av returvärdet i källan ändrar processens exitkod. Ett felaktigt anrop får ett typfel med källposition. Kör samma prov på en Mac utan utvecklarverktyg. |
| 3. En användbar delmängd | Lägg till moduler, poster, slices, ägare, lån och explicit allokering. Skriv filinläsning, diagnostik och därefter lexern i Dunder. | Den egenbyggda lexern bearbetar kompilatorns riktiga källkod. Kör positiva och negativa säkerhetsfall för varje operation i minnesmodellen. Börja publicera faktiska byggtider här. |
| 4. Självhosting | Implementera resten av kompilatorn och byggdrivrutinen i den Dunder-delmängd bootstrapkompilatorn förstår. | Bootstrapkompilatorn bygger D1. D1 bygger D2 från kompilatorns Dunder-källa. D2 bygger D3 från samma källa. D2 och D3 är byteidentiska och klarar språktesterna. |
| 5. Prestandabevis | Optimera mot de program som ännu missar bygg- eller körmålen. Pröva sedan inkrementella ändringar på kompilatorns egen kodbas. | Publicera korpus, råtider, korrekta körresultat och jämförbara C/C++-byggen enligt mätkontraktet. |

Den första implementationen börjar vid etapp 0. Grammatik utöver den delmängd som behövs, generics, metaprogrammering, pakethantering, ytterligare mål och ett publikt stabilt Dunder-ABI är öppna designfrågor.
