# Dunders arkitektur

Dunder skriver ARM64/macOS, ARM64/Linux, x86-64/Linux, `wasm32`, `wasm32-wasi`
och `x86_64-windows` (PE32+) med explicit målval. ELF, Mach-O, PE32+, systemanrop
och kodgenerering ingår i kompilatorn. Dunder 1.0 bygger på en enkelriktad,
typad kompilatorpipeline från förlustfritt AST via HIR, högnivå-CFG MIR
(`selfhost/mir2/`) och måloberoende typad SSA MIR (`selfhost/ssa/`) till
nativa målspecifika instruktionsväljare (`arm64_isel`, `x86_isel`, `wasm_isel`),
tillståndslösa kodare (`arm64_enc`, `x86_enc`, `wasm_bytes`) och en
måloberoende `ObjectModule` som matar Mach-O-, ELF64-, PE32+-, Wasm- och
DWARF-emittrarna. Läs [målplattformar och begränsningar](targets.md) och
[1.0-arkitekturspecifikationen](memory-safety-1.0.md).

Grundkraven är native maskinkod, en egen verktygskedja, minnessäkerhet utan
GC, snabb kompilering och självhosting. [Syntax](syntax.md) · [Fortsatt utveckling](next-steps.md)
Automatisk frigöring för arrayer, strängar och `@resource(dispose)`, platta
POD-värden samt verifierad SSA-optimering (Mem2Reg/SROA, enad SSA-inlining,
GVN, gränskontrollseliminering och DCE) ingår som standard.
Byggen för macOS och Linux kan bädda in DWARF v4-debuginformation (`-g`/`--debug`)
samt använda [funktionscachen](incremental.md). `dunder lsp` föreslår
`mut` vid tilldelning till en oföränderlig lokal variabel. När en importerad
modul redigeras uppdateras diagnostiken även i de öppna filer som beror på den.

## Språkets kostnadsmodell

Dunder vilar på fem grundläggande designlöften:

1. **Inga överraskningsallokeringar**: vanliga värdeoperationer — platta POD-värden på stacken och avboxade element i `[]T`, `[T; N]`, `Map` och `Set` för nästlade `Copy`-structar, tupler, `Copy`-enums och fasta arrayer `[T; N]` upp till 16 åttabyte-ord / 128 byte (inklusive `[f64; 2]`, `[f32; 4]`, `Option<(i32, i32)>`-iteratorer och `Result`-propagering med `?`) samt statiska fältlösa enumvarianter (`.None`, `.Err`) — undviker oväntade heapallokeringar.
2. **Inga överraskningskopior**: kopierbara värden kopieras direkt, medan resursägande värden (`string`, samlingar, fångande `Fn`-closures m.fl.) flyttas som standard och kräver uttrycklig `.clone()` eller `move`.
3. **Säkert som standard**: overflowkontroll, indexgränser och låne- och ägarskapsregler gäller på samma sätt även i optimerade byggen.
4. **Snabbt av konstruktion**: direkt maskinkodsgenerering, innehållsadresserad fragmentcache och behovsstyrd generisk specialisering håller kompileringstiden förutsägbar.
5. **Ett tydligt sätt**: en enhetlig modell för traits, värderepresentation och ägarskap utan parallella specialmekanismer för samma problem.

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

Lånemodellen omfattar läs- och skrivlån i funktionsparametrar, lokala
bindningar och returvärden vars livstid hör till en källparameter.

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

### Initiering, CFG-lånekontroll och drop-elaborering (`selfhost/mir2/`)

I 1.0-pipelinen sker all ägarskaps-, låne- och städkontroll på en typad
högnivå-CFG MIR ([`selfhost/mir2/`](../selfhost/mir2/mir_body.ddr)) i stället
för genom lexikal tokenskanning under parsningen:

- **Enhetlig LIFO-kontrollflödesgraf ([`mir_build.ddr`](../selfhost/mir2/mir_build.ddr))**:
  varje funktion sänks från typad HIR till basblock med uttryckliga `Place`-projektioner,
  `Call`, `Drop`, `RegionEnter`/`RegionExit`, `TaskScopeEnter`/`TaskScopeJoin` och
  delade `defer`-basblock (`unwind_target`-`SwitchInt` med loopåterställd aktivitetsflagga).
  Tidiga utgångar (`return`, `?`, `break`, `continue`) avvecklar alltid inre `task scope`
  före yttre `defer` och `Drop`.
- **Definitiv initiering och partiella flyttar ([`mir_init.ddr`](../selfhost/mir2/mir_init.ddr))**:
  ett glest kanoniskt platsträd per funktion (bitmängder `maybe_init`/`maybe_uninit`
  över CFG:n) verifierar fältflyttar, återinitiering, `replace` samt att
  `@resource(dispose)`-structar inte partiellt flyttas eller destruktureras i säker kod.
- **Polonius-lik CFG-lånekontroll ([`mir_borrowck.ddr`](../selfhost/mir2/mir_borrowck.ddr))**:
  bakåtriktad ursprungslevnad (inklusive `dropck` för `@resource(dispose)` och
  `may_dangle` för vanliga vyer) kombinerad med blockgräns-propagerade lånemängder
  (`contains[bb_entry][O]` strömmad per sats) eliminerar falska konflikter på
  sammanfallande fältnamn och åtskilda `if`/`else`-grenar. Modellen upprätthåller
  bidirektionell invarians (`O_a <-> O_b`) bakom `&mut T`/`&mut FnMut`, `TwoPhaseMut`
  för implicita `&mut self`-mottagare, `Shared`-lås i `match`-guarder, icke-lånande
  `Iterator`, `PARAM_ENV` för closures som returnerar fångade lån samt
  enkelriktad utvärderingsordning för anropsargument och binära operatorer.
- **Råpekarexponering ([`mir_escape.ddr`](../selfhost/mir2/mir_escape.ddr))**:
  ovillkorlig anropsgrafanalys över alla funktioner, traits, generiska instanser
  och closures förhindrar att scratch- och lokala lån når `addr_of` (`E4005`).
- **Drop-elaborering ([`mir_drop_elab.ddr`](../selfhost/mir2/mir_drop_elab.ddr))**:
  sänker `Drop(p)` direkt till villkorslösa anrop när platsen är garanterat initierad,
  tar bort döda `Drop` när platsen är flyttad och reserverar minimala 1-bytes
  drop-flaggor enbart för villkorligt flyttade vägar.

### Förstklassiga vyer (`&[]T`, `&str`) och isolerade uppgifter

`&[]T` och `&str` representeras i 1.0 som förstklassiga 2-ords `Copy`-värden
`(ptr, len)` (16 byte på 64-bitars nativa mål, 8 byte på `wasm32`), utan
48-bytes stackramsbeskrivare och utan dold `sret`-kopiering. Intervallgränser
kontrolleras före UTF-8-gränser (status 103 respektive 106), och eftersom
vyerna är vanliga `Copy`-värden kan `Option<&str>`, `Result<&str, E>`, tupler
och användarstructar med vyer returneras fritt med `from`.

Funktionscachen hoppar inte över dessa kontroller: källkroppar och lån
kontrolleras även när färdiga objekt kan återanvändas.
[Körbara låneprov](../tools/test_local_borrows.ddr) ·
[Fas 0-regressioner](../tools/test_phase0_soundness.ddr).

På macOS, Linux och Windows kan isolerade uppgifter köras på flera trådar med
`Sendable` + `Sync`, per-tråd-arenor, OS-sidallokerade synkroniseringsprimitiver
samt gränskontrollerad `.rodata`-tabell (`TYPE_DESC_TABLE`) för frysta
sammansatta nyttolaster. [Uppgifter och ägarskap](tasks.md)

## Enkelriktad kompilatorpipeline (från källkod till maskinkod)

```text
Källkod (.ddr)
  → Lexer + förlustfritt AST       (selfhost/frontend/lexer.ddr, parser.ddr, ast.ddr)
  → Typad HIR + monomorfisering    (selfhost/hir/hir_types.ddr, hir_body.ddr, hir_lower.ddr, hir_mono.ddr)
  → Högnivå-CFG MIR + lånekontroll (selfhost/mir2/mir_build, mir_verify, mir_init, mir_borrowck, mir_escape, mir_drop_elab)
  → Måloberoende typad SSA MIR     (selfhost/ssa/ssa_ir.ddr, ssa_lower.ddr, ssa_verify.ddr, ssa_opt.ddr)
  → Nativ LIR/ISel + regalloc      (selfhost/backend/arm64_isel.ddr, x86_isel.ddr, wasm_isel.ddr)
  → Tillståndslösa kodare          (selfhost/backend/arm64_enc.ddr, x86_enc.ddr, wasm_bytes.ddr)
  → Måloberoende ObjectModule      (selfhost/backend/object_module.ddr)
  → Binärformat & debuginfo        (macho.ddr, elf.ddr, pe.ddr, dwarf.ddr, wasm_memory.ddr, wasm_abi.ddr)
```

1. **Modulladdning och AST**: [Modulladdaren](../selfhost/driver/modules.ddr)
   löser importer, bevarar källpositioner och producerar ett gemensamt
   förlustfritt AST för `build`, `check`, `fmt` och `lsp`.
2. **Typad HIR och förseglad `TypeTable` ([`selfhost/hir/`](../selfhost/hir/hir_types.ddr))**:
   deklarationer, trait-kontrakt (`Sendable`, `Sync`, `Iterator`),
   `ParamMode::BorrowOrCopy` samt closures som typade fångststructar sänks och
   monomorfiseras i HIR innan en enda förseglad `TypeTable` fryses för
   parallella arbetare (utan `declared`/`body`-delning eller token-omskrivning).
   [Design för generics](generics-design.md)
3. **Verifierad högnivå-MIR ([`selfhost/mir2/`](../selfhost/mir2/mir_verify.ddr))**:
   varje funktion genomgår `mir_verify`, `mir_init`, `mir_borrowck`, `mir_escape`
   och `mir_drop_elab` i isolerade arbetare på den frysta HIR-bilden. Oanropade
   kroppar kontrolleras också. [Kompilatorns moduler](../selfhost/README.md) ·
   [Parallell kompilering](tasks.md#kompilatorn-som-taskkonsument)
4. **Måloberoende SSA MIR och enad optimerare ([`selfhost/ssa/`](../selfhost/ssa/ssa_opt.ddr))**:
   efter drop-elaborering sänks varje funktion till måloberoende typad SSA MIR
   med explicita `StackSlot(size, align)` (inga inbakade ARM64-register eller
   flaggor). Optimeraren kör Mem2Reg/SROA, enad SSA MIR-inlining (över structar,
   metoder, generics, `Option`/`Result` och vyer), dominatorbaserad GVN/konstantvikning,
   gränskontrollseliminering och DCE, med fullständig semantisk
   [`ssa_verify.ddr`](../selfhost/ssa/ssa_verify.ddr) efter varje pass.
   [Kodgenerering](optimization.md#gemensam-helfunktionskedja)
5. **Nativ mål-ISel, `ObjectModule` och länkning**:
   [`arm64_isel.ddr`](../selfhost/backend/arm64_isel.ddr),
   [`x86_isel.ddr`](../selfhost/backend/x86_isel.ddr) och
   [`wasm_isel.ddr`](../selfhost/backend/wasm_isel.ddr) sänker SSA MIR direkt
   till målens egna ABI:er och register/Wasm-lokaler via de tillståndslösa
   kodarna [`arm64_enc.ddr`](../selfhost/backend/arm64_enc.ddr),
   [`x86_enc.ddr`](../selfhost/backend/x86_enc.ddr) och
   [`wasm_bytes.ddr`](../selfhost/backend/wasm_bytes.ddr) in i
   [`object_module.ddr`](../selfhost/backend/object_module.ddr). Länkaren
   placerar nåbara funktioner, datasegment, `.rodata`-tabeller och importer i
   deterministisk ordning, och Mach-O-skrivaren ad hoc-signerar slutbilden.

## Verktygskedjan och målplattformarna

Native-målen är `arm64-macos`, `arm64-linux`, `x86_64-linux` och `x86_64-windows` (PE32+).
Wasm-målen är `wasm32` och `wasm32-wasi`. Dunder äger parser,
typkontroll, kodgenerering, relokeringar, bildskrivning (Mach-O, ELF64, PE32+, Wasm), DWARF v4-debuginfo, macOS ad hoc-signering
och byggdrivrutin (`dunder build`/`run`/`test`/`check`/`fmt`/`lsp`/`install`/`upgrade`). Ett vanligt Dunderbygge kör aldrig LLVM, GCC, en extern
assembler, länkare eller signerare. Vanliga Linuxbilder använder direkta
systemanrop utan libc. Ett program med nåbara `extern`-deklarationer
(`@link(...) extern "<abi>" { ... }`) får
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

Maskindelen äger instruktionskodning, linjär registerallokering och räckvidd för hopp.
Plattformslagret anpassar startpunkt, systemanrop, metadata-layout och binärformat.
ARM64-målen delar kodaren [`arm64_enc`](../selfhost/backend/arm64_enc.ddr) och
[`arm64_isel`](../selfhost/backend/arm64_isel.ddr); x86-64/Linux och x86-64/Windows
använder [`x86_isel`](../selfhost/backend/x86_isel.ddr) + [`x86_enc`](../selfhost/backend/x86_enc.ddr)
med nativ System V- respektive Win64-ABI (utan emulerad ARM64-registerbank), och
Wasm-målen använder [`wasm_isel`](../selfhost/backend/wasm_isel.ddr) direkt till
typade Wasm-lokaler, strukturerat kontrollflöde och per-signatur `call_indirect`.
Dunders interna funktions-ABI är inte ett publikt C-ABI. Se
[målplattformarna](targets.md) för verifiering och begränsningar.

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

Wasm-backenden ([`wasm_isel`](../selfhost/backend/wasm_isel.ddr) samt
[`wasm_mir`](../selfhost/backend/wasm_mir.ddr) under övergångsfasen) sänker
verifierad SSA MIR / MIR direkt till Wasm. Den omvandlar kontrollflöde till
strukturerade Wasm-block, anrop till typade Wasm-funktioner (`call` / per-signatur
`call_indirect`) och minnesåtkomster till operationer på modulens linjära minne.
Nåbara externa deklarationer blir typade importer. Filer och processer hör till
nativevärdens API.
