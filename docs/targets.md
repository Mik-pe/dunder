# Målplattformar och korskompilering

Dunder väljer normalt den aktuella maskinens mål. `--target` väljer uttryckligen
vilken körbar fil som ska skrivas, oberoende av värddatorns OS och processor.

| Mål | Filformat | Körmiljö |
| --- | --- | --- |
| `arm64-macos` | Mach-O med ad hoc-signatur | macOS på Apple Silicon |
| `arm64-linux` | ELF64, AArch64 | Linux på ARM64 |
| `x86_64-linux` | ELF64, x86-64 | Linux på x86-64 (SSE2-basnivå) |
| `wasm32` | WebAssembly-modul | Webbläsare eller fristående Wasm-värd med bulkminnesoperationer |

```sh
dunder_bin="$(./tools/bootstrap.sh)"
"$dunder_bin" run examples/hello.ddr
"$dunder_bin" build examples/hello.ddr --target x86_64-linux
"$dunder_bin" build examples/hello.ddr --target arm64-linux
"$dunder_bin" build examples/hello.ddr --target arm64-macos
"$dunder_bin" build examples/hello.ddr --target wasm32
```

Utdata går till `build/<target>/<name>` om `-o` saknas. Wasm-filer får ändelsen `.wasm`. Samma regler gäller
projekt med `dunder.toml`. Måltripplarna `aarch64-apple-darwin`,
`aarch64-unknown-linux-gnu` och `x86_64-unknown-linux-gnu` accepteras också som
alias. Vanliga Linuxbinärer använder direkta systemanrop. Program med
nåbara externa anrop använder systemets dynamiska loader och de bibliotek
som programmet deklarerar.

Alla fyra mål ingår i samma kompilator. Dunderbyggen behöver inga installerade
målpaket, headers, sysroot, C-kompilatorer, LLVM, assemblers, länkare eller
signeringsverktyg. macOS kan därmed skriva Linuxbinärer och Linux kan skriva
macOS-binärer. Kompilering kräver ingen körning av målplattformens kod.

`run` och `test` kör bara program för den egna plattformen. Ett annat mål ger
ett tydligt fel före publicering eller körning. Använd `build` och flytta
filen till målplattformen för att köra den där. `check` behöver ingen native
backend. Windows och x86-64/macOS är ännu inte körmål.

## WebAssembly

`wasm32` och aliaset `wasm32-unknown-unknown` använder 32-bitars pekare och
`usize`. `i64` och `u64` behåller 64 bitar. `@c_layout` använder målets
fältstorlekar. Den självbyggda kompilatorn översätter samma typkontrollerade
funktioner och minnesoperationer som de nativa målen till Wasm. Modulen exporterar
`main`, `memory` och `__indirect_function_table`. Tabellen kan användas av
värden för att anropa en Dunder-callback.

`@extern("modul", "namn")` blir en typad funktion i importtabellen när
deklarationen används. Gränssnittet tar skalärer och pekare och skickar
C-layout-structar via pekare. JavaScript tar emot i64-värden som `BigInt`
och 32-bitars heltal som `Number`. Ett osignerat i32-format kan läsas med
`value >>> 0`. Värden tillhandahåller utskriftsfunktioner under importmodulen
`dunder`. Ett program som skriver ett heltal och en bool kan köras så här.

```sh
dunder build examples/hello.ddr --target wasm32 -o hello.wasm
node -e 'WebAssembly.instantiate(require("fs").readFileSync("hello.wasm"), {
  dunder: {print_i32: console.log, print_bool: x => console.log(Boolean(x))}
}).then(({instance}) => instance.exports.main())'
```

`print_string` får en pekare till en liten header med UTF-8-byternas längd i
första 32-bitarsordet och texten på offset 8. Läs `instance.exports.memory.buffer`
i varje värdanrop eftersom `memory.grow` kan ge en ny buffert. Den kompakta
Dunder-runtimen återanvänder block efter storleksklass till dynamiska arrayer,
referensräknade strängar och scratch-regioner. Adresserbara lokaler ligger på
en kontrollerad värdestack i linjärt minne. En
Wasm-fälla avslutar anropet. Skapa en ny instans om programmet ska startas
om efter ett sådant fel. Program som behöver filer eller fönster anger
värdens API via importer. `std.sys.decode_utf8` och `decode_u8` fungerar i
linjärt minne.

Med Node och en självbyggd kompilator kör
`node tools/test-wasm.mjs /sökväg/till/dunder` de portabla testgrupperna,
inklusive Vulkan-scenens geometri och SHA-256.
`node tools/test-wasm-abi.mjs /sökväg/till/dunder` kontrollerar värdanrop,
C-layout, callbacks och WebGPU-exemplets vertexdata. WebGPU-exemplet
inkluderar en webbläsarvärd och gemensamma
Dunder-vertexdata. Se [WebGPU-triangeln](../dogfood/webgpu/README.md) och
[den Dunder-skrivna Wasm-kompilatorn](../selfhost/README.md).

## Bootstrap och kodgenerering

Den självbyggda kompilatorn i `selfhost/` läser samma källspråk och
standardbibliotek för alla mål. Frysta Dunder-kompilatorer för de tre
nativa värdarna kan bygga om källorna offline. Bootstrap-skriptet
verifierar två byteidentiska självbyggen. [Återbyggnad](../bootstrap/README.md)

ARM64-backenden skriver maskininstruktioner direkt. x86-64 och Wasm
översätter samma symboliska instruktionsföljd under kompileringen.
x86-64 använder en egen registerbank och en kontrollerad värdestack.
På Wasm blir de nåbara funktionerna riktiga Wasm-funktioner, med
strukturhopp mellan grundblocken och direktanrop mellan funktioner.

Vanliga Linuxbilder innehåller egen startkod och direkta systemanrop, utan
dynamisk laddare eller libc. Nåbara `@extern`-deklarationer ger en dynamisk
ELF med deklarerade bibliotek och symboler, separata RX/RW-segment och
målets C-ABI. Vulkanexemplet använder dessa importer för att ladda och
anropa grafiken. Vulkan-körning med lavapipe är verifierad på både
ARM64/Linux och x86-64/Linux. Stacken är inte körbar och kodsegmentet
är inte skrivbart. Linux-retur och runtimefel använder `exit_group` så att
även grafikdrivrutinens arbetstrådar avslutas.
Systembiblioteket anpassar anropsnummer, argument, metadata-layout och felkoder
till målet. Dunders interna funktions-ABI är inte ett publikt C-ABI.

## Lokal verifiering

Bygg med `./tools/bootstrap.sh`, kör `dunder test dogfood/tests.ddr` på
värdplattformen och välj motsvarande testvärd för korsbyggd kod. QEMU kan
köra korsbyggda Linuxprogram och Node kan instansiera Wasm-moduler.
På macOS kontrollerar `codesign --verify --strict` signaturen på en
Mach-O-binär. [Körkommandon för kontrakt och testgrupper](../selfhost/README.md#verifiering)
