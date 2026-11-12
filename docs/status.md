# Dunder-status

Kompilatorn på `main` är skriven i Dunder. Den bygger sig själv på ARM64/macOS,
ARM64/Linux och x86-64/Linux och kan från varje värd skriva kod för alla tre
nativa målen och wasm32. Återbyggnaden börjar med en fryst Dunder-kompilator
och verifierar en seriell fixpunkt. På ARM64/macOS byggs sedan den
taskbaserade standardingången till byteidentiska självbyggen med en och fyra
workers; standarden är fyra. Linux behåller seriell standardkompilering.
`bootstrap.sh --serial` publicerar `dunder-serial` på alla värdar utan att
ersätta `dunder`. Byggkedjan för den tidigare Rust-kompilatorn har ersatts.
[Återbyggnad](../bootstrap/README.md) och
[målplattformar](targets.md).

## Språk och runtime

- Isolerade uppgifter använder `task scope`, `spawn` och typad `await` på
  ARM64/macOS. Scopeägda trådar, separata pooler och kontrollerad kloning
  håller managed-värden åtskilda. Typ- och effektanalys kontrollerar workerns
  gräns; scope-exit väntar in jobben före cleanup. Kompilatorn använder samma
  taskstöd för parallell MIR-verifiering och optimering. [Uppgifter och begränsningar](tasks.md).
- Moduler har privata och exporterade symboler, alias, namngivna typer och
  stöd för cykliska importer. Typinferens, typade funktionsvärden, indirekta
  anrop, rekursion och aggregatreturer används i kompilatorns egen källkod.
  [Syntax](syntax.md)
- Heltal har kontrollerad aritmetik och konvertering, explicita
  bitoperationer och wrapping-metoder. `f32` och `f64` har IEEE-aritmetik,
  bitkonvertering, klassificering och avrundning. UTF-8-strängar stöder
  byteindex, runes, giltiga strängsnitt och innehållsjämförelse.
  [Flyttal](floating.md)
- Structar, fasta och dynamiska arrayer samt enums med payload har
  värdesemantik. Nästlade structfält och enumpayloads kan äga samlingar.
  Vanliga arrayelement är primitiva värden, inklusive strängar och pekare,
  enums utan payload eller typade tasknycklar. `@c_layout` använder målplattformens fältstorlek
  och justering. [Enums](enums.md) och [FFI](ffi.md).
- Ägda arraybuffertar frigörs deterministiskt. Oföränderliga strängar
  använder referensräkning. Ett lån av en lokal, ett fält eller ett
  arrayelement gäller under anropet. Kontrollen avvisar motstridiga lån
  och mutationer under utvärderingen av argument. Regionen äger minnet i
  ett lexikalt scratch-block och kontrollerar vilka anrop som får låna det.
  [Minnesmodellen](memory.md)
- Standardbiblioteket innehåller map, set, array, text, hash, bytes,
  SHA-256, parse och sys. Sortering, hashning, omhashning och SHA-256 är
  Dunder-kod. Textläsning kontrollerar UTF-8 och filoperationer returnerar
  typade felvärden. [Standardbibliotek](standard.md) och [system-API](sys.md).

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
`--mir-opt=off` behåller kedjan utan optimeringspassen;
`--mir=off` stänger av hela gränsen för jämförelse.
[Omfattning och gränser](optimization.md).

`--mir-ssa=on` aktiverar [`mir_ssa`](../selfhost/mir_ssa.ddr) och
[`mir_ssa_cells`](../selfhost/mir_ssa_cells.ddr); standard är fortsatt **off**
efter [aktuell v2-mätmatris](../benchmarks/2026-11-12-ssa-pruned/README.md).
V2 bygger glesa block-/tillståndspar
för verkliga beroenden och transportparametrar, delar verkliga oförändrade
ingångsvärden och använder aktiva live-mängder/händelser för interferens.
Egna ValueID:n, GP/NZCV def/use, blockparametrar och kantargument ger en
SSA-grund över hela CFG:n. Privata åttabytesceller med
fullbreddsskrivningar och inneslutna 1/2/4/8-bytesvyer kan få placering i
GP0–GP17 mot fysisk liveness och fulla clobbers, med separat kontrollerad
`source_inputs`-läsvy. Ursprungliga minnesskrivningar behålls; okända ingångs-/post-call-minnesversioner stannar
i minnet. Frame, SP och etikett-/importnamespace ändras inte. Budgeten är
4 194 304 enheter, med gles rollback och obligatorisk slutverifiering.
Generellt SSA-baserat operationsval, uttrycksstackvärden, heap-CSE, LICM och
direkt fysisk x86-registerallokering/Wasm-lokalplacering återstår.

Källproducentens privata NoModRef, sex exakta utgångar utan observationer
och anrops-ABI1 är separata versionsbundna premisser. Råa API-fabriker
tillskriver inte godtycklig MIR dessa egenskaper. Rå-, minnes- och
terminalkontexter ger ingen rätt att ändra ett okänt anrops synliga SP eller
inkommande x25–x28. Framepasset avstår vid nåbara okända gränser utan
tillräckligt kontrakt. Den kompletta källproducenten deklarerar ABI1 enligt
[befintliga FFI-/privatminnesregler](ffi.md#objektminne-och-privata-stackceller).
Anropssynkronisering, clobbermasker och giltiga argument bevaras.
[Oberoende observerarmotexempel](../tools/test-mir-frame-exits.py).

[`mir_call_inputs`](../selfhost/mir_call_inputs.ddr) är en separat
`source_inputs`-premiss för hela källproducenten, false för råa fabriker.
GP0–GP8, GP16, GP18–GP31, alla FP-register och explicita mål behålls.
GP9–GP15/GP17 kan undantas som anropsindata endast utan semantisk
scratchdependency hos callee och fortsättning; fler argument ligger på
stacken. Råmasker, clobbers, minne och flaggor begränsas inte. FE-target X17
och Wasm-FB-target X16 är explicita roller. Rå Call läser och kan förstöra
NZCV (`flags=3`); tidigare `flags=2` var felaktig metadata, utan visad
felkompilering i dagens värdepass. Cache/kvitton binder fysisk v4 med
`call-flags1` och SSA:s `v2-pruned-inputs1-work4194304`.

Aktuell v2-QA med `flags=3` passerar 670 målbilder, 360 native-/Wasm-
körningar och 438 exakta bitjämförelser, med 232 separat redovisade luckor
i den oberoende interpretern. Källacceptansen passerar 24 målbilder och
17 native-/Wasm-körningar med workers, cache och kvitton. Linux är endast
bildbyggt i dessa matriser. Full verifiering, inklusive bootstrapens
byteidentiska självbyggen med en och fyra workers, passerar.

Med SSA på i båda kompilatorerna minskar v2 kompilatorkorpusets byggmedian
27,65 procent mot v1; Tabell förbättras 8,48 procent i median men får
20,41 procent högre p95. Samma nya kompilator med SSA av/på ger en
byggpremie på 21,26 procent för kompilatorkorpuset och 29,47 procent för
Tabell. Runtime/C-geomedlet blir 1,1841 med passet på mot 1,1883 av;
text/sort ligger på 1,3376/1,2649. Ingen samlad runtimevinst är belagd och
C-gränserna återstår. Standardlägets byggmedianer ändras 0,28–0,69 procent
mot förra kompilatorn med samma källor och byteidentiska konsumenter.

Den historiska [v1-mätningen](../benchmarks/2026-11-12-ssa-foundation/README.md)
visade 70,92 procent högre byggmedian för kompilatorkorpuset och 42,34
procent för Tabell; runtime/C-geomedlet var 1,181 mot 1,178 med passet av.
En separat SSA-optimerad kompilatorbinär gav ingen meningsfull byggvinst
med oförändrad användarpolicy. Dessa siffror gäller v1, före gles v2 och
`source_inputs`, och är inga aktuella v2-resultat.
[SSA-orakel](../tools/test-mir-ssa.py) ·
[Källprov](../tools/test-ssa-source.py) ·
[Omfattning](optimization.md#ssa-för-privata-framevärden).

Den taskbaserade kompilatorn behåller frontend, typning, nåbarhet,
diagnostik, cache-I/O och inlinebudget i en enda Session. Workers returnerar
ägda MIR-funktioner; föräldern sänker och länkar i källordning. `--jobs`
stöder 1–256 på ARM64/macOS-ingången. En worker, `check`, `--mir=off` och
`--profile-compiler` använder samma seriella Session-väg. Workerantalet
ändrar varken passbudgetar, cacheidentitet eller programbilder.

Den tidigare [30-rundorsmätningen av parallell kompilering](../benchmarks/2026-11-12-parallel-compiler/README.md),
före SSA-grunden, ger med samma binär genererad byggmedian
953,718→609,106 ms när workerantalet ökar från en till fyra. Standardkandidatens genererade p95 är 619,768 ms,
163 003 källrader/s. Runtimebilderna är identiska. C-geomedlet 1,178× och
största kvoten 1,308× i den rapporten ligger över målen 1,10× respektive
1,25×. C-målet är fortfarande öppet; rapporten är inget mätresultat för SSA.

Unika fulluttryckstemporärer av vanliga records kan överföras med generisk
normalisering av nästlade arraykapaciteter. Detta tar bort extra kopior i
bland annat kompilatorns jobb/resultat utan att flytta lokaler eller fält,
ändra värdesemantiken eller dela managed-lagring över taskgränsen.

Den fulla kodgeneratorn kan utelämna extra strängoperandägare när hela
högerledet saknar relevanta effekter. En validerad funktionsrepresentation
korrigerar instruktioner, etiketter och relokeringar atomiskt.
`--ownership-opt=off` gör mekanismen jämförbar separat från skalärpasset.
Fasur och allokeringstrafik kan profileras separat utan att vanlig emission
instrumenteras. [Profilering](incremental.md#mät-byggkedjans-faser).

Tabell är ett fristående CSV-projekt med eget manifest, gruppering,
Unicode, sortering och kontrollerat heltalsspill. Grafikexemplen använder
samma filimporter och typade värdanrop som externa program. macOS använder
AppKit och Metal, Linux använder Vulkan och webbläsaren använder WebGPU
med Dunder-skrivna vertexdata. [Tabell](../projects/tabell/README.md),
[Metal](macos-metal.md), [Vulkan](linux-vulkan.md), [WebGPU](../dogfood/webgpu/README.md)
och [macOS-appar](app-bundles.md).

## Körda Wasm-kontrakt

Den självbyggda kompilatorn byggde varje portabel grupp i `dogfood/tests.ddr`
till en separat Wasm-modul. Node körde 124 av de 138 grupperna med godkänt
resultat. De fjorton övriga har nåbara nativa systemanrop.

| Grupp | Nativt beroende |
| --- | --- |
| `dogfood/filesystem.ddr::files_round_trip_through_the_os` | Läsning och skrivning av filer |
| `dogfood/filesystem.ddr::errors_remain_values` | Fel från filåtkomst |
| `dogfood/enums.ddr::value_enums_work_in_records_and_arrays` | Filmetadata |
| `selfhost/compile.ddr::string_operand_leases_are_removed_only_across_effect_free_rhs` | Kompilatorns filåtkomst |
| `selfhost/compile.ddr::ownership_proofs_do_not_change_validation_or_binding_records` | Kompilatorns filåtkomst |
| `selfhost/compile.ddr::rune_literals_emit_the_same_code_as_scalar_constants` | Kompilatorns filåtkomst |
| `selfhost/compile.ddr::compiler_checks_types_before_emitting_an_image` | Kompilatorns filåtkomst |
| `selfhost/compile.ddr::native_compiler_rejects_unsupported_aggregate_shapes_and_immutable_mutation` | Kompilatorns filåtkomst |
| `selfhost/compile.ddr::inline_producer_preserves_authoritative_diagnostics_and_scalar_off_behavior` | Den nativa kompilatorns fil- och cachekod |
| `selfhost/compile.ddr::inline_producer_does_not_duplicate_source_binding_reports` | Den nativa kompilatorns fil- och cachekod |
| `selfhost/compile.ddr::inline_fragments_restore_backward_dependencies_charges_and_private_body_identity` | Skrivning och återställning av cachefiler |
| `selfhost/compile.ddr::inline_fragment_keys_preserve_cold_growth_frontiers_across_partial_cache_hits` | Ändring och återställning av cachefiler |
| `selfhost/compile.ddr::all_emitted_source_functions_use_mir_without_changing_diagnostics_or_bindings` | Den nativa kompilatorns fil- och cachekod |
| `selfhost/compile.ddr::whole_function_optimizer_policy_and_actual_counters_follow_pass_execution` | Den nativa kompilatorns fil- och cachekod |

Wasm-värden kontrollerar dessa gruppnamn och diagnostiken för nativa anrop.
Bland de körda grupperna finns SHA-256 med referensvektorer och olika
utfyllnadsgränser, UTF-8, samlingar, Vulkan-scenens geometri och delar av
kompilatorn. Ett separat värdtest läser WebGPU-exemplets 24 flyttal ur
exporterat minne och provar typade callbacks och C-layout på 32-bitarsmålet.
Tio längre program för strängar, arraykopior, nästlade fält och
scope-städning kördes två gånger vardera i samma instans. Det linjära
minnet hade samma storlek efter första och andra körningen.

```sh
dunder_bin="$(./tools/bootstrap.sh)"
"$dunder_bin" test dogfood/tests.ddr
node tools/test-wasm.mjs "$dunder_bin"
node tools/test-wasm-scalars.mjs "$dunder_bin"
node tools/test-wasm-abi.mjs "$dunder_bin"
python3 tools/runtime-contracts.py "$dunder_bin" --target wasm32
python3 tools/test-diagnostics.py "$dunder_bin"
```

Wasm-program som behöver filer, processer eller grafik måste deklarera
motsvarande värdimporter. Det publika ABI:t använder fyra byte för
pekare och `usize`. Internt använder kompilatorn 64-bitars registerceller.
Tokens, lånekontroll och logiken för kopiering och frigöring delas med
nativmålen. [Wasm-ABI och utskrifter](targets.md#webassembly)
