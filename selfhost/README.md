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
SHA-256 och bygger dagens källfiler. Det kräver byteidentiska efterföljande
självbyggen innan det publicerar kompilatorn i `build/selfhost/<värdmål>/`.
Rust-kompilatorn har tagits ur byggkedjan. De frysta startkompilatorerna och
Dunder-källorna är återställningsvägen. [Fröbinärer och återbyggnad](../bootstrap/README.md)

`selfhost/main.ddr`, `linux.ddr` och `x86_linux.ddr` väljer värdens
standardmål. `--target` väljer vilket mål som helst för det byggda programmet.
Kör korsbyggd Linux-kod på motsvarande Linux-system eller med QEMU och
målarkitekturens användarland.

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
en uttrycklig värdimport. Se [måldokumentationen](../docs/targets.md)
och [WebGPU-exemplet](../dogfood/webgpu/README.md).

## Filer

| Fil | Ansvar |
| --- | --- |
| `lexer.ddr` | Tokenisering, UTF-8, escapes, positioner och interning. |
| `modules.ddr` | Modulgraph, namn, synlighet, deduplicering och källmappning. |
| `bundled.ddr`, `embed.ddr` | Inbäddat standardbibliotek och dess Dunder-generator. |
| `compile.ddr` | Typer, uttryck, kontrollflöde, värdesemantik och kodgenerering. |
| `x86.ddr`, `x86_linux.ddr` | x86-64-kodgenerering och Linux-ingång. |
| `wasm_native.ddr`, `wasm_flow.ddr` | Modulformat, funktionsanrop, grundblock och relokeringar för Wasm. |
| `wasm_abi.ddr`, `wasm_alu.ddr`, `wasm_memory.ddr` | Typade värdanrop, aritmetik och linjärt minne för Wasm. |
| `arm64.ddr` | Maskinord, register, stack, etiketter och hoppfixering. |
| `runtime.ddr`, `runtime_sys.ddr` | Minne, samlingar, strängar och Darwin- och Linux-systemanrop. |
| `driver.ddr` | Gemensam modulladdning, felrapportering och publicering av filer. |
| `elf.ddr`, `linux.ddr` | ELF-format och kompilatorns Linux-ingång. |
| `macho.ddr`, `main.ddr` | Signerad Mach-O och kompilatorns macOS-ingång. |

## Verifiering

```sh
dunder_bin="$(./tools/bootstrap.sh)"
"$dunder_bin" test dogfood/tests.ddr
python3 tools/test-diagnostics.py "$dunder_bin"
python3 tools/runtime-contracts.py "$dunder_bin"
node tools/test-wasm.mjs "$dunder_bin"
node tools/test-wasm-abi.mjs "$dunder_bin"
node tools/test-wasm-scalars.mjs "$dunder_bin"
python3 tools/runtime-contracts.py "$dunder_bin" --target wasm32
```

De körda programmen har förväntade värden, utskrifter och felutfall.
Diagnostikfallen använder både giltiga exempel och källor som ska avvisas.
Testgrupperna får varsin process. För en enskild grupp kan kompilatorn
skriva en egen startpunkt med `build-tests`.

```sh
"$dunder_bin" build-tests dogfood/tests.ddr --list
"$dunder_bin" build-tests dogfood/tests.ddr --group 0 --target wasm32 -o group.wasm
node tools/test-wasm.mjs --execute group.wasm
```
