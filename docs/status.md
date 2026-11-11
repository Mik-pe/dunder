# Dunder-status

Kompilatorn på `main` är skriven i Dunder. Den bygger sig själv på ARM64/macOS,
ARM64/Linux och x86-64/Linux och kan från varje värd skriva kod för alla tre
nativa målen och wasm32. Återbyggnaden börjar med en fryst Dunder-kompilator
och verifierar två byteidentiska självbyggen. Byggkedjan för den tidigare
Rust-kompilatorn har ersatts. [Återbyggnad](../bootstrap/README.md) och
[målplattformar](targets.md).

## Språk och runtime

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
  eller enums utan payload. `@c_layout` använder målplattformens fältstorlek
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
[Omfattning och gränser](optimization.md).

Tabell är ett fristående CSV-projekt med eget manifest, gruppering,
Unicode, sortering och kontrollerat heltalsspill. Grafikexemplen använder
samma filimporter och typade värdanrop som externa program. macOS använder
AppKit och Metal, Linux använder Vulkan och webbläsaren använder WebGPU
med Dunder-skrivna vertexdata. [Tabell](../projects/tabell/README.md),
[Metal](macos-metal.md), [Vulkan](linux-vulkan.md), [WebGPU](../dogfood/webgpu/README.md)
och [macOS-appar](app-bundles.md).

## Körda Wasm-kontrakt

Den självbyggda kompilatorn byggde varje portabel grupp i `dogfood/tests.ddr`
till en separat Wasm-modul. Node körde 78 av de 84 grupperna med godkänt
resultat. De sex övriga har nåbara anrop till filsystemet.

| Grupp | Nativt beroende |
| --- | --- |
| `dogfood/filesystem.ddr::files_round_trip_through_the_os` | Läsning och skrivning av filer |
| `dogfood/filesystem.ddr::errors_remain_values` | Fel från filåtkomst |
| `dogfood/enums.ddr::value_enums_work_in_records_and_arrays` | Filmetadata |
| `selfhost/compile.ddr::rune_literals_emit_the_same_code_as_scalar_constants` | Kompilatorns filåtkomst |
| `selfhost/compile.ddr::compiler_checks_types_before_emitting_an_image` | Kompilatorns filåtkomst |
| `selfhost/compile.ddr::native_compiler_rejects_unsupported_aggregate_shapes_and_immutable_mutation` | Kompilatorns filåtkomst |

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
