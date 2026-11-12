# Kodgenerering i Dunder

Vanliga byggen går genom [typkontrollen och kodgeneratorn](../selfhost/compile.ddr).
Kompilatorn avger instruktioner, lokala etiketter och relokeringar per funktion.
Den tar med de runtimefunktioner som programmet behöver. Overflow, ogiltiga
index och ogiltiga konverteringar behåller sina kontroller även vid korsbyggen.

Huvudarkitekturen är nu en
[gemensam IR för hela funktioner](whole-function-ir-plan.md), med samma gräns
för skalära kroppar, anrop och ägda värden. En första valbar SSA-grund
placerar privata framevärden i register. Planen beskriver fortsatt generell
SSA-sänkning, loopoptimering och direkta målbackends.

## Gemensam helfunktionskedja

Alla nyemitterade källfunktioner passerar [`mir_lift`](../selfhost/mir_lift.ddr),
[`mir_verify`](../selfhost/mir_verify.ddr) och
[`mir_lower`](../selfhost/mir_lower.ddr) innan funktionscachen sparas. Hela
kroppen ingår, även ägarinitiering som ligger efter en retur i kodlayouten.
Den seriella kedjan återanvänder platta arenor och arbetsminnen mellan
funktioner; taskjobb har lokalt arbetsminne. Kontroll utan emission skapar inga backendarenor; cacheträffar återställer
redan sänkt kod.

[`mir.Function`](../selfhost/mir.ddr) beskriver operationer, operander,
maskinbredder, flaggor, kontrollkanter, byteintervall och anropseffekter.
Frontend lämnar fullständiga spann för frameåtkomster, inklusive de
scratchregister som stora offset använder. Okänd instruktionseffekt blir
en uttrycklig barriär och spärrar analyser som saknar ett tillräckligt
kontrakt. Funktionen går fortfarande genom IR-gränsen.

Övergångsadaptern behåller ursprungliga kodord för byteidentisk återspelning.
Verifieraren avkodar dem med samma gemensamma dekoder och kontrollerar exakt
att de fortfarande motsvarar operationens semantik. Ändrad semantik kräver
kanonisk sänkning. Syntetiska operationer kontrolleras också mot härledda
operand-, effekt-, use/def-, frame- och scratchkontrakt. Trasiga metadata
ger ett internt kompilatorfel. Sänkningen förbereder etiketter, relokeringar,
importplatser och kod atomiskt före commit.

`--mir=off` stänger av den nya gränsen för jämförelse. Läget och kedjans
version skiljer cachefragment och testkvitton åt. `--timings=json` visar
antal nyemitterade IR-funktioner, operationer och opaka barriärer.
Med `--mir-opt=off` ger gränsen byteidentisk kod mot den tidigare
kodgeneratorn. Representationen ensam innebär ingen runtimevinst eller ett
uppnått C-mål.

### Seriell Session och taskbaserade funktionspass

[`compile.Session`](../selfhost/compile.ddr) är den gemensamma frontendens
återupptagbara gräns. Föräldern behåller full provisorisk Code och imports genom
hela källtraverseringen. Ett kallt jobb får en ägd, lyft MIR-funktion;
[`mir_pipeline`](../selfhost/mir_pipeline.ddr) utför samma obligatoriska
verifiering, begränsade pass och efterverifiering seriellt eller i en task.
Varje kallt taskjobb skapar en lokal `pipeline.Workspace` i en ny OS-tråd
och returnerar ägd MIR och statistik. Det finns ingen permanent worker-
eller arenapool. Sänkning till `FunctionObject`, ordnad acceptans, cachelagring och länkning
sker hos föräldern. Först efter färdig sammanfogning ersätts den provisoriska
bilden. Cacheträffar är redan slutligt sänkta och får inget nytt backendjobb.

[`compile_parallel`](../selfhost/compile_parallel.ddr) äger begränsade
fönster av språkets isolerade tasks. Antalet workers ändrar inte symbol-
och nåbarhetsordning, första diagnostik, inlinebudget, passbudget eller
cacheberoenden. `--jobs N` och `--jobs=N` stöder 1–256 på ARM64/macOS-
ingången, med fyra som standard. En worker, `check`, `--mir=off` och
`--profile-compiler` använder samma seriella Session-drivrutin. Vanliga
passräknare visar faktiskt utfört arbete även för workerresultat; fulla
cacheträffar och `check` har noll backendaktivitet.

Samma kompilator kan korsbygga alla fyra mål. Begränsningen till ARM64/macOS
gäller taskruntimen som kör kompilatorjobben, inte programmens målformat.
[Parallell kompilatormätning](../benchmarks/2026-11-12-parallel-compiler/README.md)
redovisar en worker separat från standardens fyra. Rapporten föregår SSA-grunden; dess
byteidentiska runtimebilder behåller samma kvarvarande C-gap.

### Registerhem över hela funktioner

[`mir_liveness`](../selfhost/mir_liveness.ddr) beskriver fysisk register- och
flaggliveness på verkliga CFG-kanter. Iterativ SCC-analys identifierar
cykliska block; ägarinitieringens bakåthopp skapar ingen falsk loop.
Returen läser både X0-resultatet och caller-observerbart ABI-tillstånd.
Möjlig registerförstörelse vid anrop är skild från definierade resultat.

[`mir_opt`](../selfhost/mir_opt.ddr) kan ge upp till fyra oexponerade,
disjunkta åttabytesplatser registerhem i x25–x28. Det använder vanlig
funktionssemantik, liveness och minnesintervall. Byte-, halvords- och
ordåtkomster, överlapp, exponerade adresser och otillräckliga stack-/frame-
kontrakt undantas. Passet ändrar varken språkets typning eller dess
utvärderings-, fäll- och cleanupordning.

Inkommande registervärden sparas i färska frameceller och återställs före
varje normal epilog. Den kompletta framereservationen utökas atomiskt.
Stora offset kräver explicit scratchregister och bevisad liveness.
Hemmen laddas initialt från de ursprungliga framebitarna. Anrop synkroniserar
hemmen mot de ursprungliga platserna och laddar om dem efteråt. Anropsmasker
och minnesbarriärer behålls. Frameutökningen kräver dessutom ett uttryckligt
anrops-ABI och tillräckliga kontrakt för alla nåbara utgångar; synkronisering
av cellinnehåll ensam bevarar inte godtyckliga observationer av SP eller
inkommande x25–x28.

Den kompletta källproducenten kan ge ett uttryckligt, versionsbundet
[objektminneskontrakt](ffi.md) genom [`mir_memory`](../selfhost/mir_memory.ddr).
En giltig indirekt åtkomst kan då varken läsa eller skriva oexponerade,
disjunkta celler i den aktuella framen. Frame-livstid, reserverat intervall,
exponering och överlappning kontrolleras fortfarande separat. Kontraktet
gör att heapåtkomster kan passeras utan onödig framesynkronisering. Det
bevisar varken registerbevarande vid anrop eller minnesfrihet för godtyckliga
anrop. `--mir-memory=off` stänger av just denna NoModRef-premiss och behåller
synkronisering vid indirekt minne. Anrops- och utgångskontrakten är separata.
En separat may-uninitialized-analys kräver att varje ersatt läsning har
en föregående skrivning på alla nåbara vägar.

Arbetsminnena återanvänds och verkligt arbete debiteras inom 4 194 304
enheter per funktion. Budget eller saknat bevis lämnar den verifierade
funktionen orörd. Ett framgångsrikt pass verifieras alltid på nytt före
sänkning; trasigt tillstånd är ett internt fel. Hela källfunktionen går
genom IR även när den frivilliga omskrivningen avstår.

`--mir-opt=off` stänger av optimeringspassen och behåller helfunktionskedjan.
Cachefragment och testkvitton skiljer läget och pipelineversionen åt.
Tidsrapportens `mir` visar effektiv `optimization_enabled`-policy samt
`optimized`, `promoted`, `removed_loads`, `removed_stores`, `analysis_units`
och `budget_refusals`. Faktiska arbetsräknare är noll vid check, avstängt
pass och full cacheåterställning. Policyflaggan kan fortfarande vara på
vid cacheträff. `source_objects_enabled` anger den effektiva minnespolicyn
och `private_memory_ops` räknar faktiskt använda privata minnespositioner.

### Skilda kontrakt för minne, anrop och utgångar

Rå MIR kan uttrycka en callee som läser sitt inkommande SP eller x25, eller
en extern utgång som inspekterar den aktuella framen. Den tidigare
frameomskrivningen ändrade ett sådant observationsprov från `333 / 13` till
`13 / 7`. Ett annat prov ändrade observerat call-SP från `16` till `32`, och
inkommande x25 från `333` till `13`. De
[frysta motexemplen och rättningen](../tests/fixtures/mir-frame-exit-receipt.json)
är funktionella korrekthetsbevis. Nu avstår rå- och minneskontextens
framepass vid nåbara okända utgångar och anrop utan tillräckligt ABI. Ett
terminalkontrakt ensamt tillåter heller inte anrop.

Tre versionsbundna producentpremisser prövas var för sig:

- [`mir_memory`](../selfhost/mir_memory.ddr) ger NoModRef för bevisat privata
  celler vid giltiga indirekta objektåtkomster. Det ger inget anrops-ABI.
- [`mir_terminals`](../selfhost/mir_terminals.ddr) namnger exakt sex
  producentägda statusutgångar: overflow, division med noll, assertion, I/O,
  bounds och flyttalsfälla. De terminerar utan att läsa inkommande register,
  flaggor eller användarminne, köra cleanup eller anropa användarkod. Enbart
  NoReturn räcker inte; varje nådd extern måletikett behöver sitt eget bevis.
- [`mir_call_abi`](../selfhost/mir_call_abi.ddr) håller ett separat
  käll-ABI-kontrakt. `mir_call_abi.conservative()` gör inget sådant påstående;
  `mir_pipeline.options().source_abi` är false för råa konsumenter. Den kompletta källproducenten deklarerar ABI1: x25–x28 är bevarat
  dolt tillstånd, och privat frameutfyllnad och exakt call-SP är
  implementationsegenskaper. Giltiga register-/stackargument, resultat och
  exponerade källobjekt behåller sitt vanliga ABI. Premissen ger ingen
  NoModRef för anrop och ändrar inga call-clobbermasker.

De nativa runtimehjälparna har granskats utan explicita läsningar eller
skrivningar av x25–x28. C-anrop, återinträde genom callbacks, stackargument
och bevarade register provas också vid källgränsen. Detta använder de
[befintliga reglerna för privata celler och främmande anrop](ffi.md#objektminne-och-privata-stackceller);
språkets åtkomst- eller ABI-garantier har inte försvagats. Ett vanligt
främmande anrop får inte inspektera andra privata lokaler genom stackvandring.
[Frame-/utgångsprov](../tools/test-mir-frame-exits.py) ·
[FFI-prov](../tools/test-ffi.py).

### SSA för privata framevärden

[`mir_ssa`](../selfhost/mir_ssa.ddr) är en verklig, begränsad SSA-grund över
hela funktionens CFG. Egna ValueID:n beskriver GP-/NZCV-definitioner och
användningar, blockparametrar och föregångarkanternas argument. En
MAY-clobber vid anrop får en egen tillståndsversion och uppfinns aldrig som
ett semantiskt anropsresultat. De ursprungliga fysiska operationerna och
deras effekter ligger kvar som fasta begränsningar.

[`mir_ssa_cells`](../selfhost/mir_ssa_cells.ddr) grupperar oexponerade,
justerade åttabytesceller med en befintlig fullbreddsplats. Alla skrivningar
måste omfatta hela cellen. Inneslutna läsvyer på 1, 2, 4 eller 8 byte kan
ersättas med korrekt bituttag och explicit noll-/teckenutökning. Delvisa
skrivningar, korsande eller exponerade intervall undantas. Vyerna antar
inte att råa bitar redan är en normaliserad bool eller rune.

Det första placeringssteget förenar en cells phi-värden i ett gemensamt
registernät och väljer bland GP0–GP17 mot originalfunktionens fulla fysiska
liveness, scratchdefinitioner och anropsförstörelse. Ursprungliga
frame-skrivningar finns kvar i samma ordning och skriver igenom till minnet.
Ingångens och post-call-barriärernas okända minnesversioner stannar i minnet.
Passet växer inte framen, ändrar inte SP och skapar inga nya etikett- eller
import-ID:n. CFG bevaras; sänkningen räknar om kodpositionerna.

Arbetsminnet består av återanvändbara platta arenor. Budgeten på 4 194 304
enheter omfattar analys, placeringsvittnen, stagning och slutverifiering.
En gles undo-logg återställer varje ändrad rad och övrig ändrad metadata
exakt vid avslag. En accepterad omskrivning fullverifieras innan den får
lämna transaktionen; ett gammalt certifikat ersätter inte den kontrollen.

`--mir-ssa=on` aktiverar steget mellan framepromovering och värdepass i den
gemensamma seriella/taskbaserade kedjan. Standard är **off** tills hela
mätmatrisen har bedömts. Effektiv aktivering kräver både MIR och MIR-opt;
`check`, avstängt steg och full cacheträff utför inget SSA-arbete. Cache och
testkvitton skiljer on/off, SSA-version 1 och arbetsbudgeten åt.
`--timings=json` visar `ssa_enabled`, `ssa_optimized`, `ssa_cells`,
`ssa_values`, `ssa_parameters`, `ssa_registers`, `ssa_removed_loads`,
`ssa_removed_stores`, `ssa_removed_pushes`, `ssa_removed_pops`, `ssa_copies`,
`ssa_analysis_units` och `ssa_budget_refusals`. Förändringsräknarna avser
committade SSA-resultat i accepterade jobb; arbete och budgetavslag visar
faktiska försök. Store-, push- och pop-eliminering ingår inte i denna version.

Den funktionella SSA-matrisen omfattar 554 målbilder, 292 native-/Wasm-
körningar och 438 exakta bitjämförelser före/efter. Ytterligare 116 par har
uttryckligen redovisade luckor i den oberoende interpretern, bland annat
anrop, flyttal och externa utgångar; dessa räknas inte som interpreterbevis.
Linuxmålen är endast bildbyggda i denna matris. Ett loopprov går från sex
frame-laster till noll men från 26 till 30 kodord; ett 80-värdesprov från
81 laster till noll och från 329 till 249 ord. Det är strukturella resultat,
inte uppmätt bygglatens eller runtimevinst.
[SSA-/rollback-orakel](../tools/test-mir-ssa.py) ·
[Cellkontrakt](../tests/mir_ssa_cells.ddr) ·
[SSA-kontrakt](../tests/mir_ssa.ddr) ·
[Källkod, workers, cache och kvitton](../tools/test-ssa-source.py).

Generellt SSA-baserat val av skalära operationer, uttrycksstackvärden,
heap-CSE, LICM och direkt registerallokering per mål återstår. Den nuvarande
placeringen använder den gemensamma logiska GP-banken. Den innebär ingen
fysisk x86-registerallokering eller ny placering i Wasm-lokaler. C-målet
är fortfarande öppet.

### Värdeidentitet över kontrollflödet

[`mir_value_prop`](../selfhost/mir_value_prop.ddr) körs efter framepromovering
och det valbara SSA-steget, före sänkning. Det håller separata
ekvivalensklasser för hela 64-bitarsvärden och de låga 32 bitarna, exakta konstanter och nollutökning. Fakta
möts över samtliga nåbara föregångare och löses till fixpunkt innan någon
omskrivning planeras. Skrivningar och anropsförstörelse dödar äldre fakta;
ett återkommande operations-ID i en loop är aldrig en värdeidentitet.

Faktadomänen är kompakt per funktion: bara fysiska GP-register som kan
delta i kandidatoperationernas värdefakta får slots, plus ett separat XZR.
Const/Copy/MOVK:s samtliga faktaproducerande roller ingår, även om själva
operationen inte kan omskrivas. Alla operationers kompletta skriv- och
clobbermasker projiceras, inklusive implicita scratchregister. W-värdenas
32-bitarsmasker är oförändrade; domänmasker beskriver endast analysens slots.
Full fysisk liveness, NZCV och ABI-observationer behålls separat. Aliasval
följer samma fysiska registerordning och definitionspoäng. Nya
faktaproducenter måste uttryckligen utvidga domänen eller välja full domän.

Passet ersätter bevisade registerkopior, väljer kanoniska immediate-former
och tar bort döda rena materialiseringar. Minnesvärden cachas inte och
flaggor, fällor och ägaroperationer flyttas inte. Oklassificerade externa
utgångar avvisas: en utgång som aldrig återvänder kan fortfarande observera
inkommande register. Endast ett uttryckligt producentkontrakt får undanta
en terminal utgång utan sådana observationer.

En exklusiv optimeringstransaktion håller samma muterbara lån genom analys,
stagning och commit. En återanvänd gles undo-arena sparar endast de rader
som faktiskt ändras. Det finns ingen hel funktionskopia, full snapshot
eller publik plan som kan muteras mellan analyze/apply. Alla normala
avslag återställer raderna och verifierarens två metadatafält exakt.
Verifierarens interna seen-arena ligger i dess eget arbetsminne.

Replay använder varje originalrad i dataflödesöverföringen innan den
stagade motsvarigheten skrivs, vilket bevarar fakta och aliasval. Därefter
kontrolleras varje ny rad av samma kanoniska kontrakt som den fulla
verifieraren. En snäv privat bevisregel får behålla ett redan sant
certifikat för de befintliga konst-/kopie- och operandaliasomskrivningarna.
Destination, bredd, flaggor, effekter och strukturell metadata bevaras.
FP-/SP-roller eller andra ändringar tar den vanliga verifieringsvägen före
liveness. Ett falskt certifikat får aldrig återställas manuellt. Radering
av en bevisat ren materialisering ändrar enbart mode. Varje ändrat
slutresultat genomgår ovillkorlig, debiterad fullverifiering före framgång.
Arbetet, inklusive återställning, är budgetbegränsat; upprepad DCE-liveness
är inte garanterat linjär.

`--mir-values=off` stänger av värdepasset separat. `--mir-opt=off` stänger
av frame-, SSA- och värdepassen. Cache och testkvitton inkluderar policyn.
`--timings=json` visar `values_enabled`, `values_optimized`,
`value_rewrites`, `value_deleted`, `value_immediates`,
`value_analysis_units` och `value_budget_refusals`. Dessa är faktisk
passaktivitet, inte ett bevis på dynamisk runtimevinst.

## Typad funktionsmodell och lokalt optimeringspass

Standardbygget provar först en kompakt, typad och strukturerad
[funktionsmodell](../selfhost/opt_ir.ddr). Den återanvänder samma arena mellan
funktioner. [Parsern](../selfhost/opt_parse.ddr) godtar i första etappen
anropsfria funktioner med `i32`, `i64`, `bool`, delade `&[]i64`-parametrar
och exklusiva `&mut []i64`-parametrar:
lokaler, tilldelningar, aritmetik, kortslutning, `if`, `while`, retur,
`break`, `continue`, räknade `for`-loopar, arraylängd, indexläsning och
elementskrivning. Högst åtta parametrar och en skalär eller tom retur
stöds. Ägda värden, storleksändring eller byte av arraylagring, flyttal,
osäker kod och anrop använder den befintliga vägen.

Alla grenar kontrolleras innan optimeringen börjar, även om ett villkor
är konstant. Om modellen inte stöder kroppen eller når sin resursgräns
tar den befintliga kompilatorn över, inklusive feldiagnostik. Gränserna
är 4 096 token, 2 048 noder, 256 lokaler och konservativt djup 64.
Denna strukturerade lokala modell bygger ingen egen SSA. Dess emission
går vidare genom den gemensamma helfunktionskedjan och dess valbara SSA-steg.
IR är ett tillåtet verktyg i kompilatorns fortsatta utveckling. Valet mellan
strukturerad modell, SSA och lägre funktionsrepresentation ska styras av
bevisbehov och uppmätt total byggtid. Snabb kompilering är kravet; att undvika
en viss representation är inget eget mål.

Nodkolumnerna ligger i en separat `Nodes`-arena som ägs av `Function`.
Verifiering, pass och registerallokering använder samma index och metadata.
Uttrycks- och villkorsemission lånar arenan direkt och kan därför även
användas i en befintlig anroparframe. `Plan.frame_slots` är tom för den
vanliga identitetsplaceringen; en explicit karta placerar annars varje
spill i anroparens slot. Hela funktioner kräver identitetsplacering.

`emit_expression` avger bara uttrycket och lämnar resultatet i x0.
Anroparen ansvarar för verifierad modell, färdigutvärderade argument,
typnormaliserade registervärden och initialiserade arraycacher. Den vägen
skapar ingen funktionsframe eller retur och kontrollerar att emissionen
saknar anrop. Standardbygget använder denna väg för begränsad inlining
efter att vanliga anropsargument redan är utvärderade och kontrollerade.

Den separata [`opt_inline`](../selfhost/opt_inline.ddr)-katalogen är nu en
testad byggsten: den lagrar högst 32 uttrycksnoder per redan verifierad
skalär retur, 4 096 noder och 256 deklarationer totalt. Bara primitiva
parametrar/resultat, högst åtta argument och den slutna anropsfria
uttrycksmängden godtas. Katalogen fryses före användning; generation och
strukturell identitet hindrar en gammal arbetsyta från att emittera ny kod.
Källfingerprint är tomt tills alla kroppsdigester är färdiga och cachas
vid frysningen. Felaktig signatur, mål, skuggad funktionspekare, frameplats
eller ordbudget lämnar kodströmmen orörd. Argument måste redan vara
utvärderade i källordning, även om kroppen inte använder dem.

Producenten prövar deklarationssignaturer och högst 128 kroppstoken, inom
en gemensam budget på 32 768 skannade/parsade token. Endast slutna,
anropsfria retur-uttryck med `i32`, `i64` och `bool` godtas. Den vanliga
valideringen och funktionens nåbarhet behålls. Säkerhetskontroller och bindningsposter
försvinner inte när själva BL-instruktionen ersätts.

Kodtillväxten begränsas till 4 096 uppskattade ord per caller och 65 536
totalt. Strömmande rune-iteration kan inom samma budget använda den
gemensamma UTF-8-dekodern direkt; dess staging debiteras uttryckligen.
`--inline-opt=off` stänger av både uttrycksinlining och denna expansion.
`--scalar-opt=off` stänger också av katalogens produktion. Check skapar
ingen katalog eller kodexpansion.

Funktionscachen binder läget till fryst katalogidentitet, kroppsdigester,
deklarationsberoenden och återstående global budget. Varma fragment
återställer samma budgetdebitering och behövda deklarationer som ett rent
bygge. Inlining sker före helfunktions-IR; registerpasset kan därmed arbeta
över de inlagda uttrycken med samma verifiering som resten av kroppen.

[Gemensamma språkregler](../selfhost/scalar_rules.ddr) används av både
den fulla parsern och modellparsern för ASCII-namn, reserverade namn,
operatorprioritet och kontextuell bredd på signerade heltalsliteraler.
Breddkontrollen validerar decimalparserns befintliga resultat utan att
skapa en extra resultatpost per literal.
En accepterad modell har kontrollerats av modellparsern; en avvisad modell
går vidare till den fulla parsern för stöd och diagnostik.

[IR-verifieraren](../selfhost/opt_verify.ddr) kontrollerar modellen före
och efter passet: parallella vektorer, index, typer, nodroller,
räckvidd för lokaler, initiering, parameterordning, returer, looputgångar,
trädägarskap och konservativa effekter. Cykler och delade exekverbara
noder avvisas; bortkopplade arbetsnoder efter förenkling är tillåtna.
Ett internt invariantfel ger ett kompilatorfel före emission. Det räknas
inte som en vanlig modellbegränsning och döljs inte genom återfall.
Verifieraren kontrollerar var bevis får användas; själva bevisregelns
sundhet provas separat och följer inte automatiskt av formkontrollen.
Verifierarens flaggor och scopestack återanvänds mellan funktioner och
passfaser i ett separat arbetsminne per kompilatoranrop. Varje anrop
initierar modellens aktuella del även efter ett tidigare verifieringsfel.
Form-, typ- och säkerhetskontrollerna körs fortfarande varje gång.

[Passet](../selfhost/opt_pass.ddr) förenklar konstanter, lokala värden och
konstanta grenar inom en fast arbetsbudget på 65 536 steg. Det räknar
bara ut aritmetik som är giltig med språkets kontrollerade heltalsregler.
Ett överflödande uttryck måste fortfarande nå samma runtime-fälla.
Kortslutning och den observerbara ordningen mellan möjliga fel bevaras.

För en loop med `i = 0`, villkoret `i < values.len` och en enda avslutande
ökning `i = i + 1` kan passet bevisa indexläsningar före ökningen och
själva ökningen säkra. Arrayen måste vara en oföränderlig inparameter.
Andra skrivningar till indexet, `continue` eller nästlade loopar hindrar
beviset. Summeringens overflow-kontroll finns kvar. Regeln bygger på
kroppens struktur och gäller oberoende av funktionsnamn eller modul.

En räknad `for i in low..high` har en egen strukturerad nod. Båda
gränserna är `i32` och utvärderas en gång, nedre först, innan iteratorn
kommer i scope. Iteratorn är oföränderlig i källkoden och kroppen har ett
inre scope. `continue` når ökningen; `break` når utgången. En dold
övre gräns behålls över hela loopen även om källans lokaler ändras.
En loop garanterar aldrig att funktionen returnerar ett värde.

Elementskrivning är en separat typad nod med uttryckligt minnesskrivningseffekt.
Den utvärderar och gränskontrollerar destinationen före högerledet och
bevarar adressen över högerledets indexläsningar. Den exklusiva referentens
skrivrätt skiljs från själva parameterbindningens oföränderlighet.
Arrayens dataadress och längd är stabila eftersom modellens slutna
operationsmängd inte kan byta descriptor, växa, exponera adresser eller
anropa annan kod. Innehållsvärden cachas inte över skrivningar. Nya
muterbara index och skrivningar behåller sina gränskontroller.
Arrayvärdesiteration använder fortsatt den allmänna vägens snapshot.

[Registerallokeringen](../selfhost/opt_alloc.ddr) använder konservativa
livstidsintervall, förlänger dem över loopars bakåtkanter och placerar
lokaler i tolv register eller i framen. [Emissionen](../selfhost/opt_emit.ddr)
behåller arraydata och längd, avger direkta jämförelsegrenar och tar bara
bort kontroller som modellen har bevisat överflödiga. Inkommande argument
sparas före registerplaceringen så att överlappande argumentregister är
säkra. x86-64 och Wasm sänker fortsatt den delade ARM-instruktionsvokabulären;
detta inför ingen separat direkt backend för dem.

Jämförelser kan använda registerhem och cachade längdregister direkt.
En spilld operand laddas separat; små positiva konstanter använder CMP:s
immediate-form där den ryms. En konstant till vänster får byta sida med
korrekt vänd signerad relation. Två sammansatta operander behåller sin
utvärderingsordning. Samma villkorsval används för grenar och bool-resultat,
även vid uttrycksemission i en anroparframe. Inga nya relokeringar införs.

`--scalar-opt=off` stänger av den nya modellen, passet och registervägen
för jämförelse och felsökning. Tidigare lokala kodgenereringsförbättringar
är fortfarande aktiva. `--timings=json` visar antal modellerade och
emitterade funktioner, omskrivningar, bevisade indexläsningar/skrivningar och
parseråterfall. Avvisade signaturer och cacheträffar ingår inte i
återfallsräknaren. Funktionscache och `test --affected` skiljer lägena åt;
CLI-cachen behåller varma fragment separat för båda lägena.

`tools/test-optimizer.py` jämför optimerad kod med avstängt pass och kan
även jämföra med en äldre kompilator genom `--baseline`. Den provar
resultat, fällor, diagnostik, bindningsposter och vilka funktioner som
använder modellen. `tools/test-opt-emitter.py` provar emission och spill
separat. Båda ingår i `tools/verify-selfhost.sh`.

### Signerad division och rest med konstant delare

När högeroperanden är en verifierad konstant IR-nod kan emissionen sänka
`i32`- och `i64`-division till en begränsad plan. Regeln känner inte till
funktionsnamn, moduler eller särskilda tal. Variabel delare använder den
befintliga divisionen. Typkontroll och konstantförenkling sker före valet;
ingen tokenprefixigenkänning inför en parallell språkregel.

[`const_div`](../selfhost/const_div.ddr) hanterar noll, ett, minus ett,
signerad minimidelare och tvåpotenser separat. Andra delare använder en
heltalsmultiplikator med multiplikationens höga del, skift och
teckenkorrigering enligt [Granlund–Montgomery, sats 5.1](https://gmplib.org/~tege/divcnst-pldi94.pdf).
Implementationens kvot-/restrekurrens är härledd från satsens felgräns och
använder wrapping internt; den behöver ingen bigint-allokering.
Planen har en hård budget på 128 steg och återanvänds inom funktionen.

Täljaren utvärderas exakt en gång innan planen förbereds, även vid `% 1`
eller division med noll. Därmed kan nästlad division inte skriva över en
yttre aktiv plan. Ett budgetåterfall återanvänder den beräknade täljaren i
den gamla divisionsvägen. Nollfällor, aritmetikfällor inuti täljaren och
overflow för både `MIN / -1` och `MIN % -1` behålls i samma ordning.

Emissionen använder bara scratchregistren x0–x3. ARM64 och x86-64 väljer
den generella multiplikatorvägen. Wasm behåller generell division när
en hög multiplikation skulle kräva flera delprodukter; bara de enkla
specialfallen används där. Detta är en målpolicy, inte en ändrad semantik.

`--const-div-opt=off` stänger av sänkningen separat från skalärpasset och
ägaroptimeringen. Native-fragment och berörda testkvitton skiljer lägena åt.
`tools/test-const-div.py` använder oberoende Pythonheltal för kvot avrundad
mot noll och rest. Den provar faktiska Dunder-planers matematiska certifikat,
uttömmande signerade bitbredder 3–10, breda 32-/64-bitarsfall, spill,
nästlad aritmetik, kortslutning, exakt nativ fällkod och fällordning.
Alla fyra mål byggs; host och Wasm körs. Wasm exponerar `unreachable`
snarare än nativa fällkoder, vilket rapporteras separat.

### Framefria registerfunktioner på ARM64

En anropsfri skalärfunktion kan behålla anroparens FP/LR när samtliga
använda lokaler och syntetiska loopgränser har registerplatser. Arrayer,
adresserbara värden, verkliga spill och okända operationer behåller frame.
Första målpolicyn gäller ARM64/macOS och ARM64/Linux; x86-64 och Wasm
använder sin befintliga frameöversättning.

Parameterplaceringen är en begränsad parallellflytt av högst åtta register.
Kedjor och cykler bryts med reserverat `x8`, och `i32` normaliseras först
efter samtliga kopior. Registerplanen och varje flytt valideras före
emission. En separat effektobservation kontrollerar också den faktiskt
emitterade kroppen: ett framtida anrop kan inte smyga in före en rå `RET`.
Expressionernas balanserade stacktemporärer finns kvar och trapputgångar
avslutar fortfarande processen. Callee-saved register och runtimeglobala
register bevaras.

`--frame-free=off` återställer den tidigare framevägen separat från andra
pass. Cachefragment och testkvitton skiljer lägena åt. Emitter-API:t lånar
en återanvänd `Options`-post; äldre wrappers behåller sina standarder utan
en ny postallokering per funktion.

`tools/test-frame-free.py` bygger 88 riktiga bilder över fyra mål och båda
lägena. Host/Wasm provar register- och stackbevarande, argumentcykler,
normalisering, loopar, tidig retur och fällordning. Den symboliska
flyttkontrollen tolkar faktiskt emitterade instruktioner över 40 320
permutationer. Fallbackbilder och bilder för övriga mål är byteidentiska.

### Budgeterade intervall och stabila arraylängder

[`opt_range`](../selfhost/opt_range.ddr) kör efter konstantförenklingen och
före den avslutande modellverifieringen. Den spårar i32-intervall samt
relationer till en bestämd arrayrots längd, exempelvis `j <= values.len-1`.
Elementvärden cachas inte. Både delade och exklusiva arrayparametrar har
stabil dataadress/längd i modellens slutna operationsmängd; elementskrivning
ändrar ingen descriptor. Anrop, storleksändring och escape stöds inte där.

Första fasen löser loopinvarianter med joins och widening. Första iterationens
fakta får aldrig användas som ett bevis för alla iterationer. Slutliga
headers måste täcka både inträde och samtliga backedges. Andra fasen gör
replay under dessa invarianter och samlar pending-bevis. Först efter en
hel lyckad replay skrivs proof-bitarna.

Samma analys kan bevisa i32-addition, subtraktion, multiplikation och
negation. Den beräknar hela det matematiska resultatintervallet i i64
innan det klipps till i32 och kräver att båda gränserna ryms. Multiplikation
använder de fyra hörnprodukterna. Ett barn som säkert trappar saknar normal
fortsättning; dess tomma intervall får aldrig användas i förälderns matematik.
Ett bevis för föräldern ändrar inte barnets utvärdering eller kontroller.
i64-aritmetik, division och rest får inga nya aritmetikbevis från denna domän.

Kortslutning modellerar båda orsakerna till falskt `A && B`; en looputgång
får inte anta att bara vänstervillkoret blev falskt. For-gränser har egna
snapshots. Break, continue, return och nästlade loopar följer sina respektive
kanter. Varje bounds-bevis gäller exakt sin arrayrot, även när andra
arrayer har andra längder.

Budgetarna är 131 072 abstrakta arbetsenheter, åtta headeröverföringar per
loop, djup 64, 32 768 faktaceller och 512 återanvändbara uttrycksslotar.
Även tillståndskopiering och joins debiteras. Vid resursstopp lämnas
funktionens befintliga proof-bitar orörda. Trasig modell eller invariant
ger internt kompilatorfel före emission. Funktionsarenor och alla buffertar
återanvänds; inga Value/Term-records allokeras per uttryck. Funktioner utan
arrayparametrar eller obevisade i32-kandidater hoppar över denna analys.

Emissionen tar bara bort de specifikt bevisade index- och aritmetikkontrollerna.
Nollkontroller behålls. En store beräknar och
bevarar destinationen före högerledet, även när kontrollen utelämnas.
`--range-opt=off` stänger av passet oberoende av övriga val. Cache och
testkvitton skiljer lägena åt; `--profile-compiler` har en separat
`range_analysis`-fas. Längdbeviset använder den uttryckliga
[i32-invarianten i runtime](memory.md).

`tools/test-range-emitter.py` provar verklig kontrollborttagning och
budgetfallback på fyra mål. `tools/test-range-analysis.py` exporterar
faktiska modeller/bevis och följer dem i en oberoende interpreter med
checkad aritmetik, kortslutning, snapshots och muterbara arrayer. Den
kontrollerar varje dynamiskt nådd bevisad åtkomst mot rätt lagring och varje
nytt aritmetikbevis med obegränsad heltalsmatematik före vanlig overflow-hantering.
Det är ett fryst ändligt testdomänprov, inte ett formellt bevis för alla
program. Bränslestopp ger ofullständig täckning och icke godkänd körning.

## Ägarlivslängder och verifierade funktionsomskrivningar

Den fulla frontendens strängoperatorer skapar normalt en extra temporär
ägare för vänsteroperanden. Den behövs om högerledet kan ersätta eller
frigöra den ursprungliga ägaren. Om hela högerledet saknar anrop,
källskrivningar och adress-/låneexponering kan operatorn i stället låna
det redan levande värdet. Regeln gäller samma språkstruktur i alla moduler.

Effektobservationen börjar efter vänsteroperandens extra setup och slutar
före själva operatorn och dess resultatägare. Nästlade observationer
behåller yttre bevisets barriärer. Direkta och indirekta anrop,
systemanrop, målens värdanrop samt explicita skrivningar eller exponeringar
bryter beviset. Instruktionsklassificeringen körs bara när ett sådant bevis
är aktivt. Kontrolläge och oanropade kroppars kontroll bygger inga editplaner.

Endast den extra ägarplatsen markeras borttagen; ursprungliga lokaler,
parametrar och fulluttryckets andra temporärer behåller sina livslängder.
Samtliga framtida cleanup-vägar och prologens nollställning hoppar över
den markerade platsen. Platsnummer och frame-layout är stabila.

[`machine_ir`](../selfhost/machine_ir.ddr) beskriver validerade redigeringar
i en funktions ofärdiga instruktionsström. Det är ett begränsat maskin-IR,
med befintliga etiketter och symboliska relokeringar, inte en ny semantisk
parser. Först valideras hela editplanen och en karta för alla ordgränser.
Hela instruktioner och relokeringspar tas sedan bort på riktigt; överlevande
etiketter, relokeringar och importernas byteadresser korrigeras. Delade par,
överlapp och borttagna importposter avvisas. Omarkerade data- och
callbacketiketter bevaras för bildfinaliseringen.

Valideringen är atomisk och begränsad till den aktuella funktionens suffix;
tidigare funktioner skannas inte om. Arbetsminnet återanvänds. Ett brutet
internt bevis ger kompilatorfel före cachelagring eller publicering. Ett
halvt återfall med borttagen cleanup och kvarvarande setup är inte tillåtet.

`--ownership-opt=off` stänger av denna ägaroptimering oberoende av
`--scalar-opt=off`. Både kodfragment och berörda testkvitton skiljer lägena åt.
`tools/test-ownership-leases.py` jämför resultat, diagnostik och bindningar
för båda lägena på alla mål; native och Wasm provar också mutation under
högerledet, fällordning och upprepad körning med samma linjära minne.
`tools/test-machine-ir.py` provar kompakteringen separat.

## Unika recordtemporärer

[`record_take`](../selfhost/record_take.ddr) undviker en extra djupkopia när
den fulla frontendens x0-värde hör till exakt samma unikt ägda
fulluttryckstemporär och typ som dess registrerade cleanupägare. Regeln är
generisk för vanliga records. Den flyttar inga lokaler, parametrar eller
fältprojektioner och inför inga allmänna moves.

En förseglad typplan klassificerar rekursiva record-/arraybarn innan den
globala etikettrymden och cachekontexten fryses. Normaliseringen bevarar den
synliga kapacitet som en vanlig arraykopia skulle ha haft: noll för tomma
arrayer, annars max(8, längden). Strängar och delade payload-enums är
stoppunkter; C-layoutfält förblir inlinebytes. Separata lokala snapshots
fortsätter att vara oberoende värden.

Planen har en egen ägarrot utanför Session. Session behåller bara primitiva
val per typ, och hela uttryckets vanliga ägargard avgör om transfer får ske.
Efter ordnad sammanfogning skannas slutliga relokeringar en gång för att
återställa hjälpardemand för både kalla och cachade funktioner. Endast den
transitivt begärda hjälparkoden emitteras, utanför källfunktionernas MIR-
och cachegränser. Cachepolicyn innehåller `record-take1`.

`check` kopierar ingen typmetadata till planen och reserverar inga sådana
hjälpare. Resursavslag använder den vanliga kopieringsvägen; trasig intern
metadata ger kompilatorfel. Mekanismen bevarar källans värdesemantik och
behåller de verkliga isoleringskopiorna vid taskgränsen. Den hjälper också
kompilatorns vanliga jobb- och resultatrecords utan en särskild MIR-typregel.

## Nästa etapper mot C-nivå

Målet prövas över hela den publicerade sviten enligt
[prestandakontraktet](prestanda.md). En snabb summeringsloop räcker inte.
Varje etapp behöver jämförelse med avstängt pass, kontrollerade resultat
och 30 försök för runtime, genererat bygge, självbygge och Tabell.
Median, p95 och binärstorlek ska redovisas; en högre p95 får inte döljas
bakom snabbare genererad stresskod.

| Etapp | Mekanism och förväntad nytta | Krav före aktivering |
| --- | --- | --- |
| Helfunktionsvärden, SSA och minnesmodell | Privat cell-SSA med blockparametrar och placering i GP0–GP17 är valbar, med bevarade minnesskrivningar. Generellt SSA-baserat operationsval, uttrycksstack och vidare registerplacering återstår. | SSA är off som standard i väntan på mätmatrisen. Bevarande av register, NoModRef och terminalobservationer är skilda verifierade producentpremisser. |
| Starkare heltalsbevis | Intervall, induktionsvariabler och reduktion av division/rest med konstanta delare minskar kostnaden i numerik. | Bevara overflow, nollfall, negativa operander och ordningen mellan möjliga fällor. Arbetsbudget och konservativt återfall ska vara deterministiska. |
| Egna målbackends från modellen | x86-register och Wasm-lokaler kan väljas direkt från de typade värdena. | Samma semantiska kontrakt på alla mål, med faktisk Linuxkörning för Linuxanspråk. Dagens översatta ARM-register är inte fysisk x86-registerallokering. |
| Bredare specialisering och inlining | Begränsad uttrycksinlining är standard. Helfunktionsinlining kan öppna fler verkliga callee-kroppar för samma värde- och placeringsmodell. | Fast budget per funktion och anropsställe, begränsad kodtillväxt, fulla ABI/ägarkontrakt och cacheidentitet för samtliga påverkande sammanfattningar. |
| POD och destinationsbaserade värden | [Gemensamma värdeplatser och ABI](pod-values-plan.md) kan undvika heapkonstruktion för små poster. | Deklarationsvalt passningssätt för både legacy och typed IR, argument-snapshots, returdestination och explicit lagringsägare. |
| Vektorer och minnesbandbredd | SIMD, loopuppdelning och sammanhängande åtkomst kan närma sig C i bredare dataloopar. | Bevis för aliasering, varje åtkomst och varje relevant mellanresultat. Kontrollerad summering får inte ersättas med en omordnad reduktion utan sådana bevis. |
| Kortare bygglatens | MIR-verifiering och optimering körs nu parallellt genom samma Session; frontend, sänkning och länkning har en ägare. Nästa steg kan återanvända fler typade resultat och beroendesammanfattningar. | Behåll stabil diagnostik, byteidentiska bilder och budgetar. Fortsatt arbete måste mäta varm cache, privat kroppsändring och delad layoutändring separat från kalla byggen. |

Prioritera nästa mekanism efter profiler och maskinkod för arbetsfallen
som fortfarande missar målet. Mät allokering och kopiering i lexer, parser
och bibliotek separat när de dominerar; fler maskinkodspass löser inte
automatiskt dessa kostnader. Den valbara flaggan `--profile-compiler` fördelar
nu kompilatorarbetet mellan modellbygge, verifiering, förenkling,
registerallokering, emission och den fulla frontendens kroppar.
`--runtime-profile` räknar utförd allokerings- och kopieringstrafik i ett
byggt program. Klockor och allokeringsräknare används i separata observationer;
vanliga binärer används för tidsjämförelser.
[Profileringskontrakt](incremental.md#mät-byggkedjans-faser).

## Övrig kodgenerering och runtime

Den fulla frontendens ursprungliga emission läser lokala värden direkt
från sin frameplats med rätt bredd och teckenutökning. Helfunktionspassen
kan sedan ersätta bevisade åtkomster enligt kontrakten ovan. Deras stabila adress beräknas först när ett lån, en rå
adress eller en tilldelning behöver den. Fält och arrayelement behåller
sin separat sparade adress och reglerna för omvärdering efter mutation.
Skalära tilldelningar till lokaler skriver direkt till frameplatsen.

En ensam skalär högeroperand utan anrop eller postfix låter vänstervärdet
stanna i ett register. Tätare operatorer, anrop och sammansatta uttryck
använder den allmänna stackvägen. Utvärderingsordning, operatorprioritet,
kortslutning och aritmetikkontroller är desamma. Looparnas interna
jämförelser behöver heller inte spilla operanderna till stacken.

Rune-literaler avkodas under kompileringen och emitteras som skalära
konstanter. Även Unicode och escape-sekvenser behöver därför ingen
strängkonvertering eller rune-array under körning. En ensam rune-literal
kan använda samma registerväg som andra skalära konstanter.

Ett helt ägt temporärt uttryck kan överföra sitt värde till en deklaration,
ett argument, en retur, en tilldelning eller en loop. Kompilatorn spårar
uttryckets exakta ägarplats och nollställer den vid överföringen. Befintliga
lokaler och projektioner av fält eller element kopieras fortsatt. Parenteser
behåller ursprunget; nya operator- och anropsresultat får egna ägarplatser.
Arrayer behåller den kapacitet en vanlig värdekopia skulle visa, även om en
större underliggande buffert återanvänds. Sammansatta värden med nästlade
ägda samlingar använder tills vidare den rekursiva kopieringsvägen.

[ARM64-backenden](../selfhost/arm64.ddr) använder direkt LDUR/STUR för
lokala värden nära framepekaren och immediate-adresser där offseten ryms.
[x86-64](../selfhost/x86.ddr) och [Wasm](../selfhost/wasm_native.ddr)
sänker den gemensamma instruktionsströmmen till respektive mål. Wasm använder
bland annat `memory.copy` för kopiering av sammanhängande block.
Native-runtime kopierar buffertar åtta byte åt gången och avslutar med en
byte-loop för resten. Varje ordläsning ryms inom den begärda längden,
även vid ojusterade adresser. Strängars och arrayelements ägarskap behålls.

Lexern klassificerar de fasta nyckelorden utan hashtabell och avkodar
vanliga literaler direkt från giltig UTF-8. Escape-sekvenser använder den
separata avkodningsvägen med samma diagnostik och källpositioner.

Direkt `for`-iteration över en typad strängs `.bytes()` eller `.runes()`
kan strömma från en behållen strängsnapshot. Den vägen bygger eller
kopierar ingen konverteringsarray. Lagrade konverteringar och uttryck som
inte matchar den begränsade direkta vägen använder vanliga arrayer.
Källan utvärderas en gång och behålls genom `continue`, `break` och retur.

`check` och kontroll av oanropade kroppar använder ett emissionsläge utan
lagrade maskinord, etikettpositioner eller relokeringar. Virtuella räknare
bevarar deterministiska positioner för de sänkningshjälpare som fortfarande
körs. Den nya modellens delmängd valideras utan kodgenerering. En
självständig semantisk frontend för hela språket återstår.

Byggkedjans valbara [fastider i text eller JSON](incremental.md#mät-byggkedjans-faser)
gör kostnaden synlig. Optimeringar jämförs med samma källor och säkerhetsregler
genom `tools/performance.py`; `--include-check` lägger till kontrolltider,
och byggsviten omfattar även Tabell som fristående konsumentprojekt.
Med samma binär i båda positionerna och
`--baseline-flag=--scalar-opt=off` isoleras optimeringslägets effekt.

Korrektheten provas med referensresultat för signerad division och rest,
IEEE 754, kortslutning och diagnostik. Minnesproven kör även om samma
Wasm-instans och kontrollerar att de frigjorda blocken återanvänds.
[Körbara kontrakt](../tools/runtime-contracts.py) · [Funktionscache](incremental.md).

Tidigare prestandamätningar och de då använda kompilatorrevisionerna finns i
[benchmarkarkivet](../benchmarks/README.md).
Helfunktionskedjans [första byggmätning](../benchmarks/2026-11-11-whole-function-mir/README.md)
redovisar dess kostnad med identiska bilder och separat `check`.
Konstantdivision och framefria funktioner förbättrar numerikkärnan;
intervallbevis och registerjämförelser minskar sorteringens kontrollkostnad.
Den [gemensamma arkitekturplanen](whole-function-ir-plan.md) är huvudspåret.
Högre p95 i vissa arbetsfall redovisas separat från medianvinster.
