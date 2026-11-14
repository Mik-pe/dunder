# Dunder i Dunder

Kompilatorn är skriven i Dunder och bygger sig själv på ARM64/macOS,
ARM64/Linux och x86-64/Linux. Varje självbyggd kompilator kan skriva
program för alla tre nativa målen och wasm32.

Lexer, modulladdare, parser, typkontroll, kodgenerering, runtime och
ELF- och Mach-O-skrivare är Dunder-kod. Kompilatorn anropar ingen extern kompilator,
assembler eller länkare. QEMU används för Linux-körningar med en annan processorarkitektur än värddatorns.

ARM64-backendens runtime återanvänder minnesblock per storleksklass i en
512 MiB-pool och räknar referenser till dynamiska strängar. Kompilatorn
frigör lokaler, temporära uttryck, arraybuffertar och structfält vid
tilldelning och scope-slut, inklusive retur och loopavbrott. Statiska
strängar har räknare noll. Allokering och ägande beskrivs i
[minnesarkitekturen](../docs/memory.md).

ARM64-vägen kompilerar `f32` och `f64` med IEEE-aritmetik, konverteringar,
jämförelser, bitoperationerna `from_bits`/`to_bits`, avrundning och
klassificering. Decimalliteraler avrundas direkt till begärd precision.
Nativa utskrifter och `.str()` använder [decimalformatteraren](../runtime/float.ddr).

## Byggkedjan

Kör från projektets rot.

```sh
dunder_bin="$(./tools/bootstrap.sh)"
"$dunder_bin" run examples/hello.ddr
# 42
# true
```

Skriptet väljer den frysta kompilatorn för värden ur `bootstrap/`, verifierar
SHA-256 och bygger dagens seriella ingång till en byteidentisk fixpunkt.
På ARM64/macOS bygger den därefter taskingången och kräver byteidentiska
självbyggen med `--jobs=1` och `--jobs=4` innan den publicerar `dunder` med
fyra workers som standard. Linux behåller seriell `dunder`.
`./tools/bootstrap.sh --serial` bygger enbart `dunder-serial` på alla värdar
utan att ersätta standardbinären.
Rust-kompilatorn har tagits ur byggkedjan. De frysta startkompilatorerna och
Dunder-källorna är återställningsvägen. [Fröbinärer och återbyggnad](../bootstrap/README.md)

`selfhost/main_parallel.ddr` är standardingång på ARM64/macOS.
`main.ddr`, `linux.ddr` och `x86_linux.ddr` är de seriella ingångarna och
väljer värdens standardmål. `--target` väljer vilket mål som helst för det byggda programmet.
Kör korsbyggd Linux-kod på motsvarande Linux-system eller med QEMU och
målarkitekturens användarland.

## Tasks i kompilatorn

`--jobs N` och `--jobs=N` väljer 1–256 workers i den taskbaserade ingången.
Den kör på ARM64/macOS och kan skriva program för alla fyra mål. Den
seriella ingången stöder bara `--jobs=1`. En worker, `check`, `--mir=off`
och `--profile-compiler` går genom samma seriella Session-drivrutin utan
scheduling. Profilens fasintervall förblir seriella väggtider.

En återupptagbar `compile.Session` äger frontend, typning, symboler, nåbarhet,
inlinebudget, provisorisk kod och cache-I/O. `next_into` lämnar en ägd
helfunktions-MIR åt gången. Jobbet verifierar och optimerar med egna
arbetsarenor och sänker sedan i samma OS-worker direkt till numerisk
`mir_lower.ObjectData`. Svaret innehåller slutobjekt och statistik i stället
för hela MIR-arenan. Föräldern validerar strukturen, gör ordnad acceptans,
sparar cache och sammanfogar genom `mir_link` i ursprunglig källordning.
Den kör ingen andra sänkning.
Ingen worker lånar Session, Code eller globala importtabeller.

Varje taskfönster omfattar högst `workers` källsteg och håller snapshots
och resultat levande till ordnad acceptans. Varje kallt jobb startar en
OS-tråd och skapar lokal `pipeline.Workspace`; ingen permanent worker-
eller arenapool återanvänds. `--jobs=1` återanvänder den seriella kedjans
arbetsminne. Cacheträffar använder redan slutligt sänkta objekt utan att köra
MIR-passen igen. Diagnostik, bindningsposter, cacheberoenden, inlinebudgetar
och kodbilder är deterministiska oberoende av färdigordning och workerantal.
[Taskkontrakt](../docs/tasks.md) och
[historisk mätning med en och fyra workers](../benchmarks/2026-11-12-parallel-compiler/README.md).

[Frysta interface och ägda typade frontendjobb](../docs/parallel-frontend-plan.md)
är nästa planerade gräns. Dagens typning och frontendemission är fortfarande
Session-ägd; den planen är ännu ingen aktiv parallell frontend.

## Helfunktionspass och valbar SSA

Varje nyemitterad källfunktion går genom den gemensamma MIR-gränsen med
obligatorisk verifiering. Samma [`mir_pipeline`](mir_pipeline.ddr) utför
framepromovering → valbar SSA → värdepropagering seriellt och i workers.
Cacheträffar är slutligt sänkta och kör inga nya MIR-pass. Typning och
diagnostik använder samma fulla frontend oberoende av passval.

`--mir-ssa=on/off` väljer [`mir_ssa`](mir_ssa.ddr); standard är fortsatt **off**.
I den historiska [v2-mätmatrisen](../benchmarks/2026-11-12-ssa-pruned/README.md)
kostade SSA på 21,26 procent extra byggmedian för kompilatorkorpuset och
29,47 procent för Tabell utan belagd samlad runtimevinst. MIR och MIR-opt
måste också vara på. V2 bygger egna ValueID:n för GP/NZCV def/use,
blockparametrar och föregångarkanternas argument glest efter verklig
efterfrågan. Oförändrade GP-/NZCV-ingångar delas och iterativ kantlösning
undviker tät block×variabel-expansion. Aktiva live-mängder och verkliga
in-/utträdeshändelser styr interferens och placeringsvittnen, utan att varje
operation skannar alla statiska SSA-versioner.
[`mir_ssa_cells`](mir_ssa_cells.ddr) grupperar privata åttabytesceller med
fullbreddsskrivningar och inneslutna läsvyer på 1/2/4/8 byte. En cells
phi-värden förenas och placeras i GP0–GP17 mot fysisk liveness och fulla
clobbers. En separat `source_inputs`-premiss får förfina anropsläsningarna.
Stores skriver fortsatt igenom till minnet och
okända ingångsminnesversioner stannar i minnet. Efter en barriär får en
fullt MUST-initierad åttabytescell fångas på nytt i en separat registerregion
endast med giltigt frame-livstidskontrakt. Barriären bevisar ingen
värdelikhet. Inga nya frameceller,
SP-justeringar, etiketter eller import-ID:n skapas.

Budgetsatt analys, placeringskontroll, gles undo och slutverifiering ryms
inom 4 194 304 enheter per funktion. Avslag återställer funktionen exakt.
`--timings=json` redovisar effektiv `ssa_enabled` och faktiska `ssa_*`-räknare
från accepterade jobb. Förändringsräknarna gäller committade omskrivningar.
Check och full cacheträff utför inget SSA-arbete. On/off och
SSA-markören `v3-regional-expression1-work4194304` ingår i cache- och
testkvittoidentiteten. Fysisk v5 binder `call-flags1`, `stack-origin1`,
`frame-life1` och källanrops-ABI2. De underliggande livstids-, ursprungs- och
lagringspremisserna är fortsatt oberoende. Workerantalet ingår inte.

[`mir_memory`](mir_memory.ddr), [`mir_terminals`](mir_terminals.ddr) och
[`mir_call_abi`](mir_call_abi.ddr) håller privata NoModRef-celler, sex exakt
angivna utgångar utan observationer och källanropens ABI1 åtskilda. Vanliga
råa fabriker ger inga sådana källpremisser. Den kompletta källproducenten
sätter ABI1 och äger beviset för giltiga anrop och bryggor. Det befintliga
framepasset med fyra registerhem får därmed fortsätta säkert för källkod;
rå MIR med nåbara okända utgångar eller anrop avstår utan tillräckligt kontrakt.
Anrop synkroniserar fortfarande originalceller och behåller sina clobbers.
[Reglerna för privata objekt, stackargument och FFI](../docs/ffi.md#objektminne-och-privata-stackceller)
är oförändrade.

[`mir_call_inputs`](mir_call_inputs.ddr) ger en separat helproducentpremiss,
`source_inputs`, som är false i råa fabriker och explicit true hos den
kompletta källproducenten. GP0–GP8, GP16, GP18–GP31, alla FP-register och
explicita mål bevaras som indata för stödda ABI1/2/3-anrop. Scratch GP9–GP15
och GP17 får undantas bara utan källsemantisk dependency på inkommande eller
lokalt odefinierade post-call-bitar; fler än åtta argument följer
stackkonventionen. Premissen omfattar runtimehelpers och C-/callback-/
taskbryggor och följer inte av en extern deklaration, MAY-clobber, NoModRef
eller anrops-ABI. Råmasker muteras inte, och clobbers, minne och flaggor
begränsas inte. Okända anropsformer behåller sina konservativa läsningar.

[`mir_decode`](mir_decode.ddr) ger FE:s target explicit X17 och Wasm-FB:s
X16; targets kan inte räknas som fri scratch. Rå Call läser inkommande NZCV
och kan förstöra dem, alltså `flags=3`. Det tidigare `flags=2` var felaktig
metadata. Fyndet har inte visat någon felkompilering i dagens värdepass.

Den historiska [SSA-v1-mätningen](../benchmarks/2026-11-12-ssa-foundation/README.md)
gav 70,92 procent högre byggmedian för kompilatorkorpuset och 42,34 procent
för Tabell utan samlad runtimevinst. SSA-optimerad producentkod gav heller
ingen meningsfull byggvinst där. Detta gäller v1 och är inga mätningar av
gles v2 eller `source_inputs`; bootstrapprofilen behålls.

Frame-livstid är en oberoende del av [`mir_call_abi.Context`](mir_call_abi.ddr).
En normal retur ska återvända med samma FP/SP och giltiga, mappade
caller-frameintervall; cellvärdena får ändras. Detta ändrar inga råa
anropsmasker och ger varken NoModRef eller ett initieringsbevis. Ett separat
full8 MUST-init-bevis krävs före regional återläsning efter barriären.

[`mir.Function.stack_roles`](mir.ddr) markerar positivt ursprung för
implementationens uttryckstemporärer; råa push/pop har ingen sådan rätt.
[`mir_expr_stack`](mir_expr_stack.ddr) håller separat `source_temporaries`-
behörighet för att elidera den privata sextonbyteslagringen och dess egna
resursfel. Markeringen ensam, source-objects och call-ABI räcker inte.
Argument, FP/LR, brygglagring och källadresserbara värden undantas. Explicita
aritmetik-, index-, åtkomst- och kapacitetskontroller i andra operationer
behåller sin ordning. En persistent LIFO-graf binder Temp-värden genom CFG
och loopar; det är ingen matchning av intilliggande instruktioner.

Frame- och Expr-värden går genom samma GP0–GP17-allokering med en fysisk
liveness-beräkning, gemensamma konflikter, SSA-budget på 4 194 304 enheter
och en gemensam undo-/slutverifiering. Båda delarna ser varandras levande
värden innan ett register får väljas.

Generellt SSA-baserat operationsval, vidare värdeplacering, heap-CSE och
loopplacering återstår. Nuvarande registerplacering gäller den gemensamma
logiska banken; direkt x86-registerallokering och Wasm-lokaler är framtida
målbackendsteg. De funktionella proven visar korrekthet och minskad
lasttrafik i vissa fall, men ger inget nytt latens- eller C-prestandaanspråk.
Historisk riktad v2-QA med `flags=3` passerade 670 målbilder och 360 native-/
Wasm-körningar, med 438 exakta bitpar och 232 redovisade interpreterluckor.
Den matrisens källacceptans passerade 24 målbilder och 17 körningar. Linux är endast
bildbyggt i dessa matriser. Den revisionens fulla verifiering passerade
också, inklusive bootstrapens fixpunkt med en och fyra workers. V2 minskade
SSA-byggmedianen för kompilatorkorpuset 27,65 procent mot v1, men runtime/C-geomedlet 1,1841
med passet på klarar inte C-gränsen. Analysvinsten och standardvalet var separata beslut. Dessa resultat
beskriver den frysta v2-revisionen, före regional återläsning, gemensam
Frame-/Expr-placering och worker-lokal sänkning.
[SSA- och rollback-orakel](../tools/test-mir-ssa.py),
[gemensam placering](../tests/mir_ssa_joint.ddr),
[ObjectData-livstid](../tools/test-mir-object-data.py),
[käll-/worker-/cacheprov](../tools/test-ssa-source.py) och
[detaljerad omfattning](../docs/optimization.md#ssa-för-privata-framevärden).

## Vad kompilatorn klarar

Självbygget använder punktimporter, alias, privata/exporterade symboler,
cykliska modulgrafer, structar med djup värdekopiering, fält och lån,
fasta och dynamiska arrayer, taggade enums med payload, UTF-8-strängar
och strängsnitt. Ett `match` kontrollerar uttömmande fall, binder egna
kopior av payloadvärden och kan returnera funktionens värde.
Funktioner stöder framåtanrop, rekursion, aggregatreturer och fler än åtta
parametrar. Funktionsvärden med signaturen `fn(...) -> T` kan skickas som
argument och anropas indirekt. `range` bevarar tecknet hos båda gränserna
och en iterator från `.iter()` kan användas i flera loopar. Heltalsaritmetik, bitoperationer, kontrollflöde, `print`,
`assert` och de systemanrop som behövs för kompilatorns filhantering ingår.

Standardbibliotekets källor är inbäddade i `bundled.ddr`. Efter ändringar
i de moduler som listas i `stdlib/modules.txt`, regenerera med Dunder själv:

```sh
"$dunder_bin" run selfhost/embed.ddr
```

Modulladdaren bevarar källpositioner så att fel pekar på ursprunglig fil
och rad. Den skyddar indata och importerade filer från överskrivning,
även via symboliska och hårda länkar. Utdata publiceras med temporär fil
och rename efter ett lyckat bygge.

Vanliga byggen skickar modulladdarens namnupplösta tokenström direkt till
kompilatorn. Varje källfil lexas en gång, och tokenpositionerna mappas
till ursprunglig modul. Testbyggen använder fortfarande den sammansatta
källtexten för att lägga till testfunktioner och teststart.

`build` genererar kod för nåbara funktioner och typkontrollerar också
oanropade kroppar i importträdet. `check --agent` skriver JSONL med
modulsökväg, UTF-8-byteoffsetar och härledda typer för lokala bindningar.
Diagnostiken innehåller fil, rad, kolumn och felkod.

Lån är funktionsparametrar. Kontrollen spårar variabler, fält,
arrayelement och mutationer under argumentutvärderingen. Två delade lån
är tillåtna. Ett muterbart lån kräver exklusiv åtkomst till sin rot.
Ett lexikalt regionlån får heller inte flyttas till ett resultat, lagras i
ett längre levande värde eller lämnas vidare till ett anrop som kan göra en
rå pekare av lånet. Se [minnesarkitekturen](../docs/memory.md).

CLI:t stöder även `run`, `test --affected`, tidsgränser för tester, manifest,
målval och macOS-appar med resurssignering. Nativebyggen kan återanvända
funktionsfragment med `--incremental`.

### Modulprivilegier och filhandtag

Vid filbaserade byggen kontrollerar modulladdaren `capability unsafe` i
varje fil, även i onåbara funktioner. Ett uppslag av en funktion med
`@requires(filesystem)` kräver filens eget `capability filesystem`. Det
gäller även funktionsvärden. Endast den deklarerande modulen får konstruera
en `opaque struct` eller öppna ett fält i säker kod. Den kontrollen sker vid
typkontroll av funktionskroppen, även om programmet aldrig anropar den.
[Filhandtag och legacy-API:er](../docs/ffi.md#filåtkomst-genom-delegerade-handtag)

### Wasm32 från den självbyggda kompilatorn

Den självbyggda kompilatorn kan skriva en modul för Node eller webbläsaren.

```sh
./tools/bootstrap.sh
build/selfhost/arm64-macos/dunder build examples/hello.ddr --target wasm32 -o hello.wasm
node tools/test-wasm.mjs build/selfhost/arm64-macos/dunder
```

Välj värdens katalog under `build/selfhost/`. Kompilatorn använder samma
typkontroll, kopiering, ägarskap och kod för regioner som de nativa målen.
Backenden omvandlar den gemensamma instruktionsföljden till Wasm-funktioner.
Vanliga anrop och rekursion använder Wasms anropsstack. En kontrollerad
värdestack på 1 MiB i linjärt minne gör lokaler och lånade fält adresserbara.
Runtimen återanvänder frigjorda block och utökar minnet med `memory.grow`
när den behöver mer utrymme.

Modulen exporterar `main`, `memory` och `__indirect_function_table`. Vid
värdgränsen upptar pekare och `usize` fyra byte. Skalära importer och
callbacks har deklarerade Wasm-signaturer. Värden tar emot C-layoutposter
via pekare. Systemanrop för filer och processer kräver ett nativt mål eller
en uttrycklig värdimport. Se [måldokumentationen](../docs/targets.md).

## Filer

| Fil | Ansvar |
| --- | --- |
| `lexer.ddr` | Tokenisering, UTF-8, escapes, positioner och interning. |
| `modules.ddr` | Modulgraph, namn, synlighet, deduplicering och källmappning. |
| `bundled.ddr`, `embed.ddr` | Inbäddat standardbibliotek och dess Dunder-generator. |
| `compile.ddr` | Återupptagbar Session, typer, kontrollflöde, värdesemantik, källordning och slutlig emission. |
| `compile_parallel.ddr`, `mir_task.ddr` | Begränsade taskfönster, ägda funktionsjobb och ordnad acceptans. |
| `mir_pipeline.ddr` | Samma verifiering och frame-/valbara SSA-/värdepass för seriell körning och workers. |
| `mir_ssa.ddr`, `mir_ssa_cells.ddr`, `mir_expr_stack.ddr` | Regional frame-SSA och positiva Temp-värden i gemensam registerplacering, med separata livstids-/lagringspremisser. |
| `mir_memory.ddr`, `mir_terminals.ddr`, `mir_call_abi.ddr` | Separata producentpremisser för objektminne, exakta terminalobservationer och källanropens ABI. |
| `mir_call_inputs.ddr` | Separat semantisk anropsindata-vy för den kompletta källproducenten; råkontraktet bevaras. |
| `mir_lower.ddr`, `mir_link.ddr` | Worker-lokal sänkning, numerisk ObjectData-transport, strukturell objektvalidering och ordnad sammanfogning. |
| `record_take.ddr` | Normalisering vid transfer av unika fulluttryckstemporärer; lokaler flyttas inte. |
| `task_data.ddr`, `task_effects.ddr`, `task_native.ddr`, `task_scope_emit.ddr` | Isolerad datagräns, effektbevis, OS-jobb och lexikal cleanup. |
| `opt_ir.ddr`, `opt_parse.ddr` | Återanvändbar typad funktionsmodell och konservativ skalär parser. |
| `opt_pass.ddr` | Begränsad förenkling och bevis för säkra index och ökningar. |
| `opt_verify.ddr` | Typ-, scope-, kontrollflödes-, effekt- och strukturinvariants före och efter passet. |
| `scalar_rules.ddr` | Gemensamma namn-, operator- och heltalsliteralregler för källparsrarna. |
| `opt_alloc.ddr`, `opt_emit.ddr` | Livstidsintervall, registerplacering, spill och emission för modellen. |
| `x86.ddr`, `x86_linux.ddr` | x86-64-kodgenerering och Linux-ingång. |
| `wasm_native.ddr`, `wasm_flow.ddr` | Modulformat, funktionsanrop, grundblock och relokeringar för Wasm. |
| `wasm_abi.ddr`, `wasm_alu.ddr`, `wasm_memory.ddr` | Typade värdanrop, aritmetik och linjärt minne för Wasm. |
| `arm64.ddr` | Maskinord, register, stack, etiketter och hoppfixering. |
| `runtime.ddr`, `runtime_sys.ddr` | Minne, samlingar, strängar och Darwin- och Linux-systemanrop. |
| `driver.ddr` | Gemensam modulladdning, felrapportering och publicering av filer. |
| `elf.ddr`, `linux.ddr` | ELF-format och kompilatorns Linux-ingång. |
| `macho.ddr`, `main.ddr`, `main_parallel.ddr` | Signerad Mach-O och seriell respektive taskbaserad macOS-ingång. |

## Verifiering

```sh
dunder_bin="$(./tools/bootstrap.sh)"
"$dunder_bin" test tests/suite.ddr
"$dunder_bin" run tools/test_diagnostics.ddr -- "$dunder_bin"
python3 tools/runtime-contracts.py "$dunder_bin"
node tools/test-wasm.mjs "$dunder_bin"
node tools/test-wasm-abi.mjs "$dunder_bin"
node tools/test-wasm-scalars.mjs "$dunder_bin"
python3 tools/runtime-contracts.py "$dunder_bin" --target wasm32
# Riktade funktionella SSA-/ABI-prov, utan prestandamätning:
python3 tools/test-mir-ssa.py "$dunder_bin"
python3 tools/test-mir-frame-exits.py "$dunder_bin"
python3 tools/test-ssa-source.py "$dunder_bin"
```

De körda programmen har förväntade värden, utskrifter och felutfall.
Diagnostikfallen använder både giltiga exempel och källor som ska avvisas.
Testgrupperna får varsin process. För en enskild grupp kan kompilatorn
skriva en egen startpunkt med `build-tests`.

```sh
"$dunder_bin" build-tests tests/suite.ddr --list
"$dunder_bin" build-tests tests/suite.ddr --group 0 --target wasm32 -o group.wasm
node tools/test-wasm.mjs --execute group.wasm
```
