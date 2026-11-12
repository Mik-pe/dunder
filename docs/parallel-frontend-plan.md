# Frysta gränssnitt och parallell frontend

Planen började vid källrevision `f025cb5`; baslinjen nedan skiljer dagens
worker-lokala maskinsänkning från ännu inte införd parallell frontend.
Nästa sammanhängande steg är att samma språkfrontend producerar en ägd, fullspråklig `TypedFunction` mot frysta
deklarationsgränssnitt. Då kan även typad kroppssänkning till MIR och
typkontroll bli isolerade funktionsjobb. Ingen andra parser eller ny
begränsad språkö ska införas.

## Vad som redan finns

Standardkompilatorn på ARM64/macOS använder fyra workers. `--jobs=1..256`
väljer storlek på ett begränsat taskfönster. Föräldern äger en `compile.Session`
och utför fortfarande modulladdning, deklarationer, typ- och lånekontroll,
kroppsemission, MIR-lift, cache och slutlig sammanfogning. Workers tar emot
en ägd `mir.Function`, kör verifiering och MIR-pass och sänker därefter med
samma lowerer direkt till slutlig numerisk `mir_lower.ObjectData`. De
returnerar slutobjekt och statistik i stället för hela MIR-arenan. Föräldern
strukturellt validerar objektet före ordnad acceptans, cache och länkning;
den sänker inte en andra gång. Varje kallt jobb startar en riktig OS-tråd
via språkets befintliga pthread-baserade taskruntime på ARM64/macOS och
skapar lokala pipeline-/sänkningsarenor; någon permanent
worker- eller arenapool finns inte. Kontrolläge, en worker, MIR-off och
kompilatorprofilering använder den delade seriella Session-vägen.

Detta framgår av [compile_parallel](../selfhost/compile_parallel.ddr),
[dagens jobbobjekt](../selfhost/mir_task.ddr) och
[Session.next_into](../selfhost/compile.ddr#L2870).
[Taskkontraktet](tasks.md) beskriver isolering och resultatskopiering.

Den historiska [parallellmätningen](../benchmarks/2026-11-12-parallel-compiler/README.md)
visar redan en vinst av MIR-parallellism: samma binär med en respektive fyra
workers byggde det genererade programmet på median 953,718 respektive
609,106 ms och den frysta seriella självhostingången på 874,940 respektive
709,677 ms. Dessa värden kommer från den rapportens källrevision. De mäter
varken parallell frontend eller hela bootstrapkedjan. De föregår också
worker-lokal maskinsänkning och regional Frame-/Expr-placering och är ingen
aktuell baslinje för den nya implementationen.

## Gränsen som saknas

`Session` blandar nu tre livstider i en ägarrot:

| Livstid | Nuvarande innehåll | Föreslagen ägare |
| --- | --- | --- |
| Hela programmet, oföränderligt efter deklarationer | Modulägare, namn, signaturer, nominella typer, fält, layout och åtkomsträttigheter | `DeclarationWorld` |
| En funktions semantik och lokala lagring | Cursor, scopes, lokaler, mutabilitet, temporära ägare, places, cleanup och loopar | `FunctionContext` och `TypedFunction` |
| Hela programmets ordnade samordning | Nåbarhet, globala typ-/symbol-ID:n, imports, inlinebudget, cache, lånegraf, taskeffekter och publicering | `Session` |

Utgångspunkterna finns i [Session-fälten](../selfhost/compile.ddr#L75),
[headers](../selfhost/compile.ddr#L483) och
[compile_body](../selfhost/compile.ddr#L3261). Kroppsreseten där visar en stor
del av det tillstånd som faktiskt har funktionslivstid.

Att enbart flytta dessa fält till en ny record räcker inte:

- [add_type](../selfhost/compile.ddr#L285) tilldelar direkt clone/drop-etiketter
  i global Code. `compound`, `fixed` och `callable_type` internerar typer även
  under kroppstypning. Typidentitet och maskinsymboler måste få skilda livscykler.
- [invoke](../selfhost/compile.ddr#L1369) gör eager argumenttypning, borrow- och
  mutationskontroll, lagrar argumentslots, förbrukar inlinebudget och emitterar
  anrop i samma funktion. Den semantiska ordningen måste kunna uttryckas utan
  global Code.
- Strängar, imports, callbacks, statisk lagring och typhelpers upptäcks under
  kroppar. De ska bli ordnade symbolbehov, inte worker-mutationer av globala
  räknare.
- [scratch.Loans](../selfhost/scratch.ddr#L45) innehåller en helprogramsgraf med
  parameterkanter och indirekta anrop. Taskcertifieringen har också en
  transitiv effektgraf. Dessa kan inte behandlas som enbart lokal scratch.

## Dataobjekt och identiteter

### Normaliserad källa och DeclarationWorld

Modulladdning, namnupplösning över modulgränser och eventuell formatterinjektion
sker exakt en gång. Källkartor och deklarations-ID:n binds till just denna
normaliserade källa. En worker får inte återinjicera hjälpkällor eller köra en
alternativ modulflattening.

`DeclarationWorld` förseglas efter samma deklarationsvalidering som idag:

- `ModuleID`, `NameID` och `DeclID` tilldelas deterministiskt i befintlig
  ordning. Funktionssignaturer inkluderar parameter- och returtyper, modulägare,
  unsafe/extern/test-egenskaper och nödvändiga ABI-kontrakt.
- Nominella `TypeID` och deras fullständiga fält-, enum-, layout-, opaque- och
  ägarmetadata förseglas. En framåtdeklaration är inte ett tomt färdigt record.
- Namn, källkartor och signaturer läses via lånade gränssnitt i den seriella
  frontenden. Den frysta världen äger inga emitteringscursorer, maskinetiketter
  eller funktionslokala ägare.
- En innehållsidentitet omfattar interface, åtkomsträttigheter och relevant
  mål-/layoutpolicy. Ett enkelt generationsnummer räcker för lokal lifecycle,
  men ersätter inte innehållsidentitet i persistent cache.

Se [modulupplösningen](../selfhost/modules.ddr) och
[deklarationspassen](../selfhost/compile.ddr#L483). Gränsen måste bevara
opaque-ägare och modulkapabiliteter, inte bara typers storlek och fältnamn.

### TypeRef och funktionslokala typer

Frysta deklarationstyper och nya kroppslokala strukturella typer skiljs åt med
en explicit `TypeRef { space, id }`. Första varianten pekar in i världen; den
andra i funktionens egen typ-arena. Inga råpekare eller negativa maskinetiketter
används som typidentitet.

Den lokala arenan beskriver strukturella tupler: array/fixed-array inklusive
längd, borrow och mutabilitet, callable med argument/retur samt TaskID och andra
befintliga strukturella typer. Den får referera till världstyper och andra
lokala typ-ID:n. Samma regler för likhet, layout och tillåtna element används
av en gemensam typmodul; arbetaren får inte implementera en egen typvariant.

Koordinatorn validerar och internerar inkomna typer i källordinal och lokal
förekomstordning och skapar en explicit remap. Därmed kan två workers skapa
samma strukturella typ utan en delad muterbar interner. Befintliga typ-ID:n och
hjälpbehov ska behålla den seriella ordningen under övergången. Ett redan
förseglat nominellt interface får aldrig utökas av en kropp.

### TypedFunction

`TypedFunction` är resultatet av den befintliga auktoritativa kroppstypningen.
Det är ingen ny scalar-parser och behöver inte vara ett fullständigt konkret
syntaxträd. Det är en platt arena med typade värden, places och ordnad
kontroll, före fysisk register- och frameplacering.

Minsta fullspråkliga kontrakt omfattar:

| Del | Måste uttryckas explicit |
| --- | --- |
| Värden och typer | Literalbredd, konvertering, arithmetic/trap-kontrakt, float, bool, enum, record och samlingar |
| Places | LocalID, fält, index, dereferens, lån, mutabilitet och relevant byte-/layoutinformation |
| Kontroll | Block och scopes, if/match, alla loopformer, kortslutning, return, break och continue |
| Anrop | Resolved DeclID eller explicit indirekt mål, exakt signatur/ABI, eager argumentordning, utvärdering även av oanvända argument och borrow-refresh före anrop |
| Ägande | Kopiering, temporär ägare, drop, scope-cleanup och de befintliga bevisade transferfallen för hela unika temporärer |
| Effekter och tjänster | Typade intrinsics, FFI/unsafe, callbacks, statisk lagring, source tasks, möjliga fällor och ordnade runtime-/ägareffekter |

Varje relevant operation behåller ursprunglig token-/källposition. Locals,
scopes, ägare, värden och places har egna stabila lokala ID:n. Ingen fysisk
registermask eller ARM-kod behövs för att förstå källoperationens semantik.
Representationen behöver inte vara SSA; lokala variabler och tilldelningar är
en tillräcklig första typad gräns.

Ett första konkret arenaformat använder operationernas `kind`, `result`,
`TypeRef`, `first_operand`, `operand_count`, `source_at` och `effects`, med en
gemensam operandkolumn. Block har en uttrycklig operationsordning och typade
terminatorer med blockmål. Värden har ValueID och typ; muterbara lokaler har
LocalID med separata read/store-operationer. Place-kolumner beskriver bas,
projektion, typ och åtkomsträtt. Scope-/owner-kolumner binder cleanup till
normal blockutgång och varje return/break/continue-kant. Kontroll och
ägareffekter får inte försvinna när ett expressionsresultat är oanvänt.

Anrop refererar till redan utvärderade ValueID:n i en argumentlista. De ska
alltså inte bära en tokenrange som lowering återbesöker. Samma sak gäller
destinationer och index: deras place-beräkning och möjliga fälla ligger på en
bestämd plats i operationsordningen. En verifierare kontrollerar slutna
typ-/värde-/place-/blockreferenser, signaturer, fullständig cleanup och att
modellen verkligen är förseglad innan lowering får konsumera den.

Typning måste fortfarande kontrollera kod efter return, konstanta döda grenar
och kortslutna uttryck. När typningen lyckats kan sänkningen använda bevisen;
den ska inte typa om tokenströmmar. Call- och place-operationer behåller den
befintliga ordningen för destinationskontroll, argumenteffekter, fällor och
cleanup. Record-transfer innebär inte allmänna moves av lokaler eller fält.

### FunctionFacts och symbolbehov

Vid sidan av kroppen lämnas `FunctionFacts` med primitiva arenor:

- Refererade deklarationer och tagna funktionsadresser.
- Parameterbaserade borrow-/escape-kanter, lokala regioners vidarebefordran
  och indirekta call-fakta för senare helprogramsupplösning.
- Direkta effekter, call-kanter, spawn-worker-ID:n och fullständighetsmarkörer
  för taskcertifiering. Okända operationer är fortfarande fail-closed.
- Bindings och diagnostik som källspann, kod och typ-/namn-ID:n. Parenten
  formatterar meddelanden och mappar till ursprunglig fil.
- Lokal typ-arena och symbolbehov med lokal förekomstordning.

Symbolreferenser skiljer minst mellan källdeklaration, typ-clone/drop,
runtimehelper, import, callback, literal, statisk lagring och funktionslokal
etikett. Ett symbolbehov innehåller också typ/ABI, storlek/alignment och
relevant lokal källidentitet. Koordinatorn reserverar globala ID:n och storage
i samma ordning som tidigare och ger ett låst symbolunderlag till lowering.
Workers uppdaterar aldrig Session.code, importtabeller eller globala räknare.

Typhelpers, taskproviders och callback-entryfunktioner är separata kroppar.
De emitteras fortfarande utanför en sourcefunktions MIR-/cachegräns; ett
upptäckt helperbehov får inte smyga in en ny funktion i en workers kropp.

De föreslagna modulgränserna är `typecheck_into(world, source, context, body,
facts)`, `bind_function_into(session, body, facts, lowering_job)` och
`lower_into(job, workspace, result)`. Dessa är plansignaturer, inte befintliga
API:er. World lånas oföränderligt och samtliga muterbara arenor är separata
ägarrötter; inga populerade Function/World-records tilldelas by value mellan
interna steg. Resultat lämnas i återanvändbara out-arenor. Tasktransporten får
sedan äga de färdiga publika numeriska jobb-/resultatvärdena enligt sitt
befintliga snapshotkontrakt.

## Jobb och ägargräns

Första nya jobbgränsen ligger efter full kroppstypning och ordnad bindning av
typer/symboler:

```text
parent: fryst värld -> befintlig frontend -> TypedFunction + FunctionFacts
parent: ordnad facts-merge + typ/symbolbindning + inlinebeslut
worker: ägt TypedFunction-jobb -> MIR -> verifiering/pass -> sänkt resultat
parent: ordnad accept -> cache + link + slutlig bild
```

Jobbet äger kroppen och ett slutet underlag för de typer, layouts och symboler
som lowering behöver. Det ska inte klona hela DeclarationWorld per funktion.
Parenten behåller källnamn och rapportering där arbetaren bara behöver ID:n.
Varje worker har lokala återanvändbara arbetsarenor inom sitt jobb eller sin
framtida batch; de får inte ligga som lånade fält i jobbindata.

Dagens [task-Data](../selfhost/task_data.ddr) tillåter inte delade RC-strängar,
borrows eller oklassificerade opaque-värden över gränsen.
[lexer.Tokens](../selfhost/lexer.ddr#L5) är därför inte ett färdigt jobbobjekt.
Använd primitiva ID-kolumner och packade UTF-8-bytearenor med offset/längd där
källtext verkligen behöver transporteras. Befintligt `FunctionObject` är opaque och korsar inte taskgränsen. Den
införda transparenta numeriska `mir_lower.ObjectData` använder de vanliga
Data-/klonreglerna och valideras efter transport. Den bär lokala offsetar,
globala symbol-ID:n, import-site-ID:n, ursprungsspann och en monoton
operations-/ordgränskarta. Rätt form ger inget semantiskt certifikat för
godtyckliga maskinord; exakt taskscope, ordinal och deklaration behåller
resultatets betrodda ursprung. Ingen namnregel specialgodkänner MIR eller
opaque-resultat. Denna transport kan återanvändas även efter den framtida
TypedFunction-gränsen.

För parallell typning blir nästa jobb `FunctionSource + FrozenInterface`.
Det behöver namnupplösning och mer av världen än loweringjobbet. Börja med ett
explicit ägt packat interface per begränsad batch och mät kopieringskostnaden.
Att dela en referens till parentens värld över dagens taskgräns är inte en
lösning. Ett framtida generellt immutable-transportkontrakt kan ersätta
batchkopian efter separat språk-/livstidsverifiering.

## Vad som återanvänds

- [mir.Function](../selfhost/mir.ddr) och
  [mir_pipeline](../selfhost/mir_pipeline.ddr) behåller verifiering, kanoniska
  maskinoperationer och pass. Dagens MIR är till stor del fysiskt precolored
  med explicita ABI-/minneseffekter; den ersätter inte den nya källtypade
  gränsen. Direkta frontend-operationer ska på sikt ersätta word-liftad semantik.
- [FunctionObject och lowerer](../selfhost/mir_lower.ddr) och
  [mir_link](../selfhost/mir_link.ddr) återanvänds för lokala offseter,
  relokeringar och ordnad sammanfogning. Jobbets ID-bindning måste vara färdig
  innan dess maskinsymboler används.
- [opt_ir.Nodes](../selfhost/opt_ir.ddr) ger ett återanvändbart mönster för
  kolumner och källpositioner. Dess typdomän och opcode-set är uttryckligen
  begränsade; `opt_parse` får inte bli auktoritet för resten av språket.
  Befintliga skaläroptimeringar kan senare konsumera den fullt typade modellen.
- Delade regler i [scalar_rules](../selfhost/scalar_rules.ddr), nuvarande
  typ-/layoutregler och borrow-/ägarkontrakt flyttas till gemensamma moduler,
  inte till en andra uppsättning regler i en workerparser.
- Session, taskfönster, ordinal/decl-sidecars, cacheberoenden och ordnad accept
  ger redan en fungerande koordinatorsstruktur.

## Genomförande och leveransgränser

### 1. Separera seriellt utan ny språkväg

Extrahera DeclarationWorld och funktionslokalt tillstånd ur Session. Skilj
typinternering från etikettallokering och introducera TypeRef samt symbolbehov.
Behåll först en worker och dagens ordnade traversal. Samma parserfunktioner
utför typning, scope-/borrowkontroll och producerar typade operationer.
Flytta deras maskinemission bakom en gemensam loweringkonsument.

Arbetet kan delas i interna commits för uttryck/places/anrop, kontroll och
ägare/FFI/tasks, men milstolpen aktiveras först när hela dagens språk täcks.
Ingen funktion får välja en enklare scalar-parser eller gammal frontend som
produktionsfallback. Ett fryst gammalt compilerprogram får användas som
oberoende testreferens. Semantik ska inte implementeras två gånger.

Den seriella adaptern använder befintliga emitters i samma ordning för att
först bevisa identiska bilder under oförändrad passpolicy. Check använder
samma semantiska producent med en facts-only sink: lokalt kontrolltillstånd
finns kvar, men inga machine-/MIR-/loweringarenor konstrueras. Frivilliga
optimeringsbudgetar får inte begränsa vilket giltigt språk som accepteras.

**Leveransbevis:** fullspråklig seriell bootstrap/fixpunkt, tidigare diagnostik
och sourcebindings, alla mål och flagglägen samt separat mätt kostnad för den
nya seriella gränsen. Behåll ingen omätt permanent dubbelrepresentation.

### 2. Flytta typad kroppssänkning till ägda jobb

Maskinsänkningen MIR→ObjectData kör redan i workers. Det framtida steget
flyttar också den typade kroppens sänkning till MIR: Session producerar
TypedFunction i samma nåbarhetsordning, sammanför facts och låser
typer/symboler och inlinebeslut. Workers kör sedan samma fullspråkliga
kroppssänkning, MIR-pipeline och slutobjektssänkning som den seriella vägen. Returnera ägt resultat med ordinal, DeclID och
status; inga lånade slices av provisorisk Code.

Använd befintlig ordered-linker, slutobjektscache och strukturellt validerad
ObjectData-transport. Opaque-resultat ska fortsatt inte få ett typundantag. Cacheträffar
går direkt till ordnad accept och kör inga backendpass igen. Serial/parallel
har samma semantiska steg och policy; en worker återanvänder seriella arenor.

**Leveransbevis:** workerantal ändrar varken bilder, första diagnostik,
sourcebindings, inlinefrontier eller cachedebitering. Mät hur mycket av dagens
seriella kroppsemission/lift/lower som faktiskt flyttats och den totala vinsten,
inklusive packning, snapshot, child clone och resultatskopiering.

### 3. Parallellisera typning mot frysta interface

Låt ett ägt sourcejobb köra samma FunctionContext och parser mot världen och
returnera TypedFunction + FunctionFacts eller diagnostik. Separera gärna de
två köerna: typningsresultat accepteras i ordning, varefter färdigbundna
loweringjobb kan startas. Ingen worker tilldelar globala ID:n eller konsumerar
globala budgetar.

Typning kan utföras i förväg, men resultat exponeras enligt den tidigare
traversalen. Bindning av lokala typer, upptäckta callees, helpers och resurser
sker först när just det resultatet får accepteras. Oanropade kroppar och
helprogramsfakta kontrolleras fortfarande med samma diagnostikprioritet.
Framtida parallellt kontrolläge behöver facts och lokala semantikarenor men
ingen backendpayload.

**Leveransbevis:** heltäckande frontend- och helprogramsdiagnostik mot den
seriella modellen, inklusive dead-body-ordning, taskcertifiering och scratch
som blir rå via indirekta anrop. Först därefter får check eller vanlig
typning välja flera workers som standard.

## Determinism, budgetar och cache

1. **Källordning betyder dagens faktiska traversal.**
   [Session-svepet](../selfhost/compile.ddr#L2877) skannar needed/compiled och
   kan upptäcka bakåtriktade callees först på nästa varv. Att sortera alla
   deklarationer en gång ger inte automatiskt samma kod- eller felordning.
   Arbete får köras spekulativt; merge och publicering följer den seriella
   ordningen. Tidigare väntande backendfel får slå ut ett senare frontendfel.
2. **Ordning inom en kropp är också semantik.** Argument utvärderas eager och
   vänster till höger, även om optimering senare inte använder deras värden.
   Mutationskontroll, destinationsfällor, borrow-refresh och cleanup får inte
   flyttas över varandra utan samma bevis som idag.
3. **Globala optimeringsbudgetar ägs av koordinatorn.** Inlinekatalogen och
   dess policy är frysta; caller/global growth-frontier debiteras vid den ordnade
   facts-/typningsmergen, före lowering, inte när en worker råkar bli färdig.
   Lokala MIR-budgetar är oförändrade när workerantalet ändras. Resursrefusal
   för en frivillig optimering lämnar en giltig ooptimerad kropp. Trasig metadata
   är ett internt fel, aldrig ett tyst byte till en annan språkparser.
4. **Cache skiljer semantik från slutkod.** Första etappen bevarar dagens
   slutfragmentcache och dess skip-beteende; tvinga inte fram omtypning av en
   giltig varm träff bara för refaktorns skull. Nästa cacheformat måste bära
   de facts som en träff ska återspela. En framtida typcache kräver egen
   versionsatt body/interface/ägare/policy-identitet. Slutkod kräver dessutom
   target, kodpolicy, privata inlinecallee-digests och rätt budgetfrontier.
   [Dagens context/key](../selfhost/fragments.ddr#L29) och
   [restore-debitering](../selfhost/compile.ddr#L2888) är utgångspunkter.
5. **Semantikfakta får inte försvinna på en träff.** Needed-ID:n, tagna
   funktionsadresser, inlinekostnader och obligatoriska borrow-/taskeffekter
   återställs före nästa ordnade steg. Taskprogram behåller dagens cacheavstående
   tills hela certifikatberoendet kan återställas säkert. Inga helpers eller
   typer får hittas endast tack vare ett kallt bygge.
6. **Ägar- och ursprungskontrakt är explicita.** Ett jobb/resultat binds till
   normaliserad källa, interface och deklaration. Produktionens taskscope och
   ordinal/decl-sidecars bevarar exakt ursprung. Dagens trusted accept kontrollerar
   strukturella spann; det är inte innehållsautentisering mellan olika Sessioner.
   Inför inte en osäker global nonce eller råa handles som ersättning för en
   riktig ägargräns.
7. **Ingen publicering före validerad helhet.** Typade modeller och fakta har
   en verifierad state/generation innan de blir jobb. Outputobjekt valideras
   före ordnad append. Slutbilden publiceras först efter obligatoriska
   helprogramskontroller och targetfinalisering. Giltiga tidigare cachefragment
   kan fortsatt sparas enligt den gamla seriella felordningen.

## Verifiering före aktivering

Bygg en fryst referens och jämför varje seriell separationsetapp innan
parallellisering läggs till. Det behövs både små oberoende API-prov och verkliga
sourceprogram; en positiv scalarloop bevisar inte den nya gränsen.

- Alla fyra mål: bygg/check, exakta ogiltiga diagnostiker och sourcebindings,
  oförändrade bytes där endast representation/schemaläggning ändras, native-
  och Wasm-körning för tillgängliga runtimekontrakt. Målspecifika funktioner
  behåller sin befintliga support eller diagnostik.
- Språktäckning: owned setup och init efter RET, nästlade ägare, tidiga returer,
  alla loopformer, match/enum, float, kortslutning, narrow/raw places, C-layout,
  aliases/reborrows, indirekta och många argument, callbacks, FFI, statisk
  lagring, strängar, scratch och task-cleanup. Kontrollera ordningen när flera
  olika operationer kan fälla.
- Typer/symboler: samma lokala typ skapad i flera funktioner, nya callable-
  och arraytyper i kroppar, opaque åtkomst från fel modul, kolliderande lokala
  symbol-ID:n, framtida labels/imports och helpers upptäckta genom varm cache.
- Ordning: framåt-/bakåtanrop, rekursion, fler needed-svep, fel i både tidigare
  worker och senare frontend, ogiltig dead branch/efter-return och ogiltig
  oanropad funktion. Framtvinga olika färdigordning utan att ändra utdata.
- Cache: kall, varm och blandad träff/miss; privat inlinecallee-ändring;
  signatur-/fält-/layout-/ägarrättighetsändring; budgetfrontier vid cacheträff;
  effektfull helper efter tidigare säker taskcertifiering. Workerantal ingår
  inte i cacheidentiteten.
- Ägande/lifecycle: misslyckad prepare lämnar output oförändrad, reset/reuse,
  felaktig generation/schema och taskresultat efter child-poolens reclaim.
  Kontrollera kloner, normalisering och drops för hela recordtemporärer utan
  att tillåta nya lokala/field-moves.
- Bootstrap: den frysta seedens nåbara kärna behåller stödd syntax. Seriell
  och parallell fixpunkt samt imageidentitet över jobs 1/2/4 ingår vid varje
  aktiverad milstolpe. Befintliga lokala verify-verktyg återanvänds; ingen ny
  GitHub-CI införs.

## Representativ kompileringstid och aktiveringsbeslut

Varje sammanhängande milstolpe mäts separat: seriell separation, parallell
lowering och parallell typning. Frys binärer, källor, verktyg och policy. Jämför
både gammal mot ny med en worker och samma nya binär med jobs 1/2/4. Ändra inte
optimeringsbudget eller säkerhetsprofil för att göra workerjämförelsen snabbare.

Använd den genererade flermodulskorpusen, faktisk `selfhost/main.ddr`, faktisk
`selfhost/main_parallel.ddr`, Tabell och vanliga owned-/call-/arraydominerade
program. Den parallella ingångens taskcertifiering redovisas separat. Om hela
bootstrapkedjan mäts är det en egen operation, inte ett nytt namn på att
kompilera `main.ddr`. Mät check och kall/varm/blandad funktionscache separat;
filesystem-cache och incremental-cache får inte blandas ihop i rapporteringen.

Följ det befintliga [prestandakontraktet](prestanda.md): total vanlig latens,
median/p95, minst 100 000 källrader/s vid p95 och oförändrad säkerhet. Kör
representativa färska processer enligt samma publicerade protokoll, normalt
30 alternerade rundor vid en aktiveringsjämförelse. Profilering och
allokeringsattribution körs separat; överlappande worker-CPU summeras inte till
väggtid eller till gamla seriella profilfaser.

Redovisa peakminne, antal/byte för snapshot- och resultatkloner, serialiserade
interfacebytes, jobbantal/fönsterstorlek och arbete som fortfarande görs av
parenten. Undvik per-nod heapobjekt och fulla Function/World-kopior i interna
anrop. Optimera inte klonantal genom att ändra observerbar arraykapacitet eller
släppa taskens isoleringskontrakt.

Runtimebilder ska först vara identiska vid ren representation/schemaläggning.
Även oförändrade bilder och runtime ska registreras, så en byggtidsvinst inte
beskrivs som förbättrad C-närhet. Den nya frontendarkitekturen har ingen mätt
vinst ännu. Aktivera först efter korrekthet, reproducerbarhet och ett samlat
byggtids-/minnesresultat; rapportera även förlorande fall och svansar.

En permanent workerpool och fler återanvända batcharenor kan därefter bedömas
mot observerad trådstart-/klonkostnad. De ersätter inte den frysta semantiska
gränsen. Generell delning av immutable data, callbacks/closures eller andra
nya språkfeatures kräver egna ägar- och effektkontrakt innan de används för
att kringgå dagens tasktransport.
