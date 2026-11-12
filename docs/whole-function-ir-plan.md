# Gemensam IR för hela funktioner

Målet är C-nära körning och snabb kompilering av hela språket. Fortsättningen
bygger därför på en gemensam funktionsrepresentation. Den tidigare slutna
skalärmodellen och lokala emitterförbättringar räcker inte som huvudarkitektur.
Den gemensamma gränsen, verifieringen och de första helfunktionspassen är
införda i standardkompilatorn. Planen skiljer denna grund från återstående
SSA, registerallokering och looparbete. C-målet är fortfarande öppet.

## Kedjan

```mermaid
flowchart LR
    source[Källkod] --> frontend[Typning och språkregler]
    frontend --> mir[Hel funktions-IR]
    mir --> verify[Verifiering]
    verify --> dataflow[CFG, liveness och minnesplatser]
    dataflow --> optimize[Promotion och värdepass]
    optimize --> verify2[Verifiering]
    verify2 --> lower[Register och målsänkning]
    lower --> image[Maskinkod och funktionscache]
```

Anrop, ägda värden, flyttal, loopar, tidiga returer och FFI ska rymmas i
samma kedja. En funktions stöd för optimering avgörs av explicita effekter
och bevis, inte av ett andra språkurval som bara accepterar skalära kroppar.
Typkontrollen fortsätter att kontrollera all källkod före omskrivning.

Övergången använder en adapter från den befintliga gemensamma
instruktionsvokabulären. Den gör det möjligt att flytta hela språket genom
gränsen utan att samtidigt skriva om all diagnostik. Adaptern får behålla
ursprungliga kodord för exakt återspelning, men en operation som ändras
måste sänkas från sin kanoniska semantik. Pass får inte avkoda råa ARM-ord
för att upptäcka sina operander eller effekter. Frontend börjar samtidigt
avge explicita minnes- och ABI-kontrakt; på sikt ersätter direkt IR-emission
adaptern. Den gamla direkta maskinkodsvägen ska då avvecklas.

## Milstolpar

1. **Hela språket genom gränsen.** Återanvändbar IR-arena, kontrollflöde,
   explicita operationer, register, flaggor, byteadresserade minnesplatser,
   anropskontrakt och ägareffekter. Sänkning förbereder ändringar atomiskt
   och flyttar etiketter, relokeringar och imports tillsammans. Börja med
   byteidentiska bilder, även för ägarinitiering som ligger efter `RET`.
2. **Register över hela funktioner.** Liveness och verkliga CFG-cykler,
   promotion av oexponerade frameplatser, explicita spill och bevarande av
   register vid anrop och returer. Börja med generella, verifierade
   byteintervall och inför SSA när värdeidentitet och joins behöver det.
   Inget pass får bero på en viss benchmark eller ett funktionsnamn.
3. **Arbete ur looparna.** Dominans, loopstruktur och placeringsanalys för
   konstanter och rena beräkningar. En möjlig fälla, retain, release,
   allokering eller minnesåtkomst får inte flyttas utan ett tillräckligt
   kontrakt. Noll varv och felordning måste bevaras.
4. **Direkta målbackends.** Låt samma värden och ABI-beskrivningar styra
   fysisk registerallokering på ARM64 och x86-64 och lokaler på Wasm.
   Att flytta ARM-register i dagens x86-översättning är inte en färdig
   fysisk x86-allokering. Avveckla övergångsadaptern efter att frontend
   avger hela funktionsmodellen direkt.
5. **Självhosten som användare.** Kompilatorns parser, symboltabeller,
   samlingar och egna IR-pass ska använda vanliga språkfunktioner. Om ett
   nödvändigt uttrycksmedel saknas implementeras det generellt i språket
   och provas genom samma kedja.

Första runtimeleveransen ska angripa trafik över hela loopar. I den faktiska
numeriska benchmarkens framgångsrika varv finns nio frame-läsningar och fyra
frame-skrivningar; C-referensen håller värdena i register. Detta är underlag
för prioriteringen, inte en regel som passet får känna till. Att ta bort
enstaka instruktioner lämnar detta huvudproblem kvar.

## Kontrakt som måste hålla

- Maskinbredd är skild från källtyp: en fysisk W-operation nollutvidgar;
  signerad `i32`-normalisering är en uttrycklig operation. SP och XZR får
  aldrig blandas ihop trots samma kodade registernummer.
- Laddningens utvidgning hör till operationen. Minnesidentitet följer
  byteintervall, överlapp, ursprung och escape. Ett SP-relativt intervall
  är inte automatiskt oberoende av en FP-relativ plats.
- Anropets resultatdefinitioner är skilda från register som kan förstöras.
  Intern Dunder-ABI, runtimehjälpare, C-ABI och värdanrop behöver egna
  kontrakt. Nya registerhem kräver uttryckligt bevarande.
- CFG följer exekveringskanter. Ett bakåthopp från ägarinitiering till
  kroppen är ingen loop. Okänd kontrollöverföring ger en okänd kant och
  spärrar analyser som saknar tillräcklig information.
- Flagganvändning, utvärderingsstack, frame, fällor och cleanup behåller
  sin ordning. Endast bevisat giltiga minnesåtkomster kan vara fällfria.
- Kontrollbyggen behöver ingen backend-arena. Cacheträffar återställer
  slutligt sänkta fragment; de ska inte optimeras en andra gång. IR-version,
  mål och passpolicy ingår i cache- och testidentitet.

## Kompileringstid och acceptans

Använd återanvändbara platta arenor, stabila ID:n och begränsat arbete per
funktion. Analyser ska besöka den aktuella funktionen och dess kanter;
upprepade genomsökningar av tidigare funktioner ger en kvadratisk kedja.
En arbetsbudget får avstå en frivillig optimering, medan trasiga metadata
är ett internt kompilatorfel. Bygg inte små heapobjekt per instruktion.

Mät vid sammanhängande milstolpar: fasta vanliga runtimeprogram mot C,
kompilatorns egna arbetslaster, kallt genererat bygge, självbygge och
inkrementella ändringar. Behåll samma indata, fällkontroller och C-flaggor.
Redovisa fördelning och total byggtid; profilerade delklockor är attribution,
inte vanlig latens. En snabb liten testfunktion ersätter inte det faktiska
programmet som fortfarande tappar prestanda.

[Prestandakontraktet](prestanda.md) gäller samma standardprofil för båda
egenskaperna: minst 100 000 källrader/s vid p95 för rent bygge, C-geomedel
högst 1,10× och inget enskilt runtimefall över 1,25×. De föreslagna
interaktiva målen är p95 högst 50 ms utan ändring och 100 ms efter en
privat funktionsändring. De är acceptansmål, inte uppnådda resultat.

Första grinden är korrekt genomförd helfunktionskedja med oberoende
differentialprov och verifierad självhostfixpunkt. Nästa grind kräver att
den nya register- och placeringsmodellen förbättrar representativa program
och fortfarande klarar kompileringstidsmålet. En ny arkitektur får kosta
arbete; den måste också förklara och mäta vart arbetet går. Cacheformat och
processdiagnostik fortsätter som separata uppgifter och styr inte denna
leverans.


## Exklusiv transaktionsgräns

Den tidigare analyze/apply-vägen med en full funktionsplan och exakta
snapshots är ersatt i värdepasset. En publik
`optimize_with_context(work, &mut function, context)` håller det exklusiva
lånet genom hela transaktionen. Arbetsminnet återanvänds mellan funktioner.

Analysen läser originalet till fixpunkt. Stagning sparar endast ändrade
operationsrader i en gles undo-arena; rena raderingar behöver bara sitt
ursprungliga mode. Stabil ordning, platser, etiketter och CFG kopieras inte.
Originalraden överför värdefakta innan dess ersättning skrivs. Verifiering
och liveness läser sedan den faktiska stagade representationen. Normala
avslag återställer samtliga ändrade rader samt verified/optimizable; ingen
delvis optimerad funktion lämnar det exklusiva lånet.

Verifierarens seen-scratch ligger i dess eget arbetsminne, så modellens
övriga metadata förblir orörda. En snäv bevisbevarande regel låter DCE
behålla ett redan giltigt certifikat när enbart mode ändras för rena
Const/Copy/MOVK. Även de befintliga smala skalära aliasomskrivningarna
kan behålla ett redan sant certifikat efter samma lokala radkontrakt.
FP/SP eller annan obevisad ändring tar full verifieringsväg efter replay.
Varje ändrat slutresultat fullverifieras ovillkorligen. Hela det reserverade verifierings- och rollbackarbetet
debiteras. Budgetavslag efter stagning ska återställa exakt samtliga fält.

Detta angriper kompileringskostnaden utan att ändra de valda
målinstruktionerna. Nästa runtimeetapp behöver verklig värde-SSA och
registerallokering för privata skalärplatser, smala läsvyer och
uttrycksstackvärden. Återanvändning av heaplaster behöver dessutom
adressidentitet och explicita minnesversioner; scalar-SSA ensam bevisar
inte att två heapåtkomster är oberoende.


## Parallell kompilering och språkets trådar

Det [isolerade taskstödet](tasks.md) och dess första riktiga
kompilatorkonsument är implementerade på ARM64/macOS. Den taskbaserade
ingången använder fyra workers som standard och stöder `--jobs` 1–256.
Bootstrap verifierar först seriell fixpunkt från fröet och bygger därefter
taskingångens byteidentiska självbyggen med en och fyra workers. Den frysta
seriella ingångens beroendeträd innehåller ingen ny tasksyntax. Linux
behåller seriell standardkompilering; `bootstrap.sh --serial` publicerar en
separat `dunder-serial` på alla värdar utan att ersätta `dunder`.

### Den genomförda ägargränsen

`compile.Session` äger frontendens typning, symboler, nåbarhetsordning,
diagnostik, inlinebudget, Code, imports och cache-I/O. `next_into` fortsätter
exakt samma källtraversering efter varje suspension och lämnar ett ägt
MIR-jobb, ett redan färdigt cacheobjekt eller slut/fel. Alla provisoriska
ord och globala ID:n behålls tills den slutliga bilden kan ersättas.

`mir_task.Job` innehåller ägd `mir.Function`, passpolicy, terminalkontrakt,
källordinal och deklarations-ID. `mir_pipeline` kör samma verifiering och
begränsade frame-/värdepass som den seriella vägen med workerägda
arbetsarenor. Varje kallt jobb startar en OS-tråd och skapar lokal
`pipeline.Workspace`; ingen permanent worker-/arenapool återanvänds.
`--jobs=1` använder i stället den seriella kedjans återanvända arbetsminne.
Svaret innehåller ägd MIR och status/statistik. Föräldern sänker till ett ägt `FunctionObject`: lokala ord- och etikettoffsetar,
symboliska globala ID:n samt importernas ursprungliga site-ID:n. Inga
levande referenser till förälderns Session, Code eller imports passerar
jobbgränsen. Hela källfunktionen ingår, även ägarinitiering efter `RET`.

`compile_parallel` håller ett taskfönster om högst `workers` källsteg och
tar emot svar i ursprunglig ordinalordning. `mir_link` fångar prefixets
etikettägare före första jobbet, validerar kompletta objekt före infogning och sammanfogar i
källordning. Globala ID:n bevaras och namespacet kan växa monotont innan
det förseglas. Runtime- och dataetiketter binds senare av bildskrivaren.
Föräldern äger fortsatt sänkning, sammanfogning, signering och publicering.

Cacheträffar hoppar över MIR-verifiering och optimering och återspelar
nåbarhetsberoenden och budgetdebitering innan nästa källfunktion.
Tidigare fördröjda backendfel behåller sin prioritet över senare frontendfel.
Både rena och blandade cachebyggen bevarar diagnostik, bindningsposter,
inlinebudget, passbudget och programbilder oberoende av workerantalet.
Cacheidentiteten innehåller passpolicy men inte antalet workers.

Acceptans-API:t kontrollerar ordinal och strukturella ursprungsgränser.
Köägaren måste dessutom garantera att svar hör till exakt det utlämnade
jobbet i samma Session; gränskontrollen autentiserar inte objekt från andra
Sessioner med samma struktur. Den nuvarande koordinatorn har en Session,
färska lexikala taskscopes och separata ordinal-/deklarationskolumner.
Ett återanvändbart generellt kö-API kan senare få egna opaka jobbiljetter.

En worker, `check`, `--mir=off` och `--profile-compiler` använder samma
seriella Session-drivrutin. Kontrolläget skapar inga backendarenor;
profilering behåller ett seriellt väggtidskontrakt. Scheduling ändrar inte
vilka frivilliga passbudgetar som gäller.

### Språkets isolering och generella värdesemantik

`task scope jobs(4)`, `spawn jobs worker(input)` och `await jobs(id)` äger
OS-jobb lexikalt. `TaskID<R>` är en kopierbar typad resultatnyckel, ingen
ägare. Scope, generation, ordinal och exakt resultattyp kontrolleras före
kontrollblocksåtkomst. Generationer återanvänds aldrig. Normal utgång,
`return`, `break` och `continue` väntar in jobben före föräldraägd cleanup.
Ett misslyckat join bevisar inte att barnet är färdigt och får inte följas
av läsning eller frigöring av dess lagring.

`task_data` kontrollerar mot semantisk typmetadata att den rekursiva
klondomänen inte delar managed-lagring. `task_effects` beräknar transitiveffekter över den
fullt kontrollerade anropsgrafen. `task_native` och `task_scope_emit`
genererar separat C-entry, barnpool, create/join och resultatkloning före
poolreklamering. Arbetare får ha lokala lån, strängar och allokering i sin
egen domän; typerna vid jobbgränsen får inte dela managed-handtag. De transparenta
MIR- och statusposterna uppfyller samma datagräns som andra användartyper.
Opaka typer får inget särskilt MIR-undantag.

`record_take` tar bort extra djupkopior av unika fulluttryckstemporärer i
vanlig recordkod och därmed också i jobb/resultat. En separat förseglad
plan normaliserar nästlade arraykapaciteter och emitterar endast begärda
hjälpare utanför källfunktionernas MIR. Lokaler och fältprojektioner flyttas inte;
strängar, delade enums och inline C-layout är stoppunkter. Resursavslag
kopierar som vanligt och `check` behöver ingen fylld plan. Regeln ändrar
varken taskgränsens isoleringskopior eller språkets vanliga värdesemantik.

### Mätning och återstående arkitektur

[30-rundorsrapporten](../benchmarks/2026-11-12-parallel-compiler/README.md)
skiljer samma binär med en/fyra workers från tidigare seriell kompilator
mot ny standard. Genererat bygge går från 953,718 till 609,106 ms i den första
jämförelsen; standardkandidatens p95 är 619,768 ms, 163 003 källrader/s.
Runtimebilderna är identiska. C-geomedlet 1,178× och maxkvoten 1,308×
överskrider fortfarande acceptansgränserna.

Parallell modulläsning/lexning, typade frontendresultat och separata
FunctionContext är framtida etapper. Frontend ska inte delas muterbart
bara för att öka trådantalet. Värde-SSA, registerplacering över anrop och
loopar samt direkta målbackends återstår som runtimearbete. Workerpool,
nollkopierande gränstransfer, delad heap och generellt Send/Sync behöver
separata kontrakt; den nuvarande isolerade modellen gör inte dessa anspråk.
Linux och Wasm behöver egna verifierade runtime-/transportgränser innan
de kan köra språkets tasks. Alla fortsättningar måste behålla samma
källregler, determinism och mätta totala byggkostnad.
