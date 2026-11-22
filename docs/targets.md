# Målplattformar och korskompilering

Dunder väljer normalt den aktuella maskinens mål. `--target` väljer uttryckligen
vilken körbar fil som ska skrivas, oberoende av värddatorns OS och processor.

| Mål | Filformat | Körmiljö |
| --- | --- | --- |
| `arm64-macos` | Mach-O med ad hoc-signatur | macOS på Apple Silicon |
| `arm64-linux` | ELF64, AArch64 | Linux på ARM64 |
| `x86_64-linux` | ELF64, x86-64 | Linux på x86-64 (SSE2-basnivå) |
| `wasm32` | WebAssembly-modul | Webbläsare eller fristående Wasm-värd med bulkminnesoperationer |
| `wasm32-wasi` | WebAssembly-modul (WASIp1) | WASI-värd (`wasi_snapshot_preview1`: `fd_write`, `proc_exit`) |
| `x86_64-windows` | PE32+ | Windows på x86-64 (`kernel32.dll`-importer för stdout/exit/minne) |

```sh
dunder_bin="$(./tools/bootstrap.sh)"
"$dunder_bin" run examples/hello.ddr
"$dunder_bin" build examples/hello.ddr --target x86_64-linux
"$dunder_bin" build examples/hello.ddr --target arm64-linux
"$dunder_bin" build examples/hello.ddr --target arm64-macos
"$dunder_bin" build examples/hello.ddr --target wasm32
"$dunder_bin" build examples/hello.ddr --target wasm32-wasi
"$dunder_bin" build examples/hello.ddr --target x86_64-windows
```

Utdata går till `build/<target>/<name>` om `-o` saknas. Wasm-filer får ändelsen `.wasm` och Windowsbinärer `.exe`. Samma regler gäller
projekt med `dunder.toml`. Måltripplarna `aarch64-apple-darwin`,
`aarch64-unknown-linux-gnu`, `x86_64-unknown-linux-gnu`, `wasm32-unknown-unknown`,
`wasm32-wasip1`/`wasi` och `x86_64-pc-windows-msvc`/`windows` accepteras också som
alias. Vanliga Linuxbinärer använder direkta systemanrop. Program med
nåbara externa anrop använder systemets dynamiska loader och de bibliotek
som programmet deklarerar.

Alla sex mål ingår i samma kompilator. Dunderbyggen behöver inga installerade
målpaket, headers, sysroot, C-kompilatorer, LLVM, assemblers, länkare eller
signeringsverktyg. macOS kan därmed skriva Linux-, Wasm-, WASI- och Windowsbinärer och Linux kan skriva
macOS-, WASI- och Windowsbinärer. Kompilering kräver ingen körning av målplattformens kod.

`run` och `test` kör bara program för den egna plattformen. Ett annat mål ger
ett tydligt fel före publicering eller körning. Använd `build` och flytta
filen till målplattformen för att köra den där. `check` behöver ingen native
backend. Flaggan `-g` / `--debug` (stöds för `build`, `run` och `install`)
emitterar DWARF v4-debuginformation och symboltabeller för nativa mål. `dunder lsp`
startar en JSON-RPC-språkserver över standardströmmar. x86-64/macOS är ännu inte ett körmål.

Detaljer för Windows- och WASI-målen verifieras i
[WASI- och Windowsproven](../tools/test_wasi_and_windows.ddr). På nativa mål
växer programheapen dynamiskt med extra allokeringssidor (slabs) utöver den
initiala mappningen (med upp till `1 GiB` per enskild allokering), och
`sys.read_u8`/`read_u8_limit`/`read_text_limit` stöder upp till `1 GiB` (`1 073 741 823` byte).

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

`print_string` får en pekare till strängens 64-bitars bytelängd i
little-endian-format, följd av UTF-8-texten på offset 8. Läs `instance.exports.memory.buffer`
i varje värdanrop eftersom `memory.grow` kan ge en ny buffert. Den kompakta
Dunder-runtimen återanvänder block efter storleksklass till dynamiska arrayer,
referensräknade strängar och scratch-regioner. Adresserbara lokaler ligger på
en kontrollerad värdestack i linjärt minne. En
Wasm-fälla avslutar anropet. Skapa en ny instans om programmet ska startas
om efter ett sådant fel. Program som behöver filer eller fönster anger
värdens API via importer. `std.sys.decode_u8` och `decode_u8_range` fungerar i
linjärt minne.

I `tools/verify-selfhost.sh` kör Node bland annat program för sammansatta
arrayer och byteoperationer. Proven kontrollerar både korrekta resultat och
Wasm-fällor för ogiltiga index eller intervall.
[Arrayprov](../tools/test_aggregate_arrays.ddr) · [Byte- och minnesprov](../tools/test_memory_bytes.ddr)

## Bootstrap och kodgenerering

Den självbyggda kompilatorn i `selfhost/` läser samma källspråk och
standardbibliotek för alla mål. Frysta Dunder-kompilatorer för de tre
nativa värdarna kan bygga om källorna offline. Bootstrap-skriptet
verifierar två byteidentiska självbyggen. [Återbyggnad](../bootstrap/README.md)

ARM64-backenden emitterar maskininstruktioner direkt från MIR. x86-64/Linux och
Wasm har nu också egna direkta MIR-backends: [x86-64-MIR-sänkning](../selfhost/backend/x86_mir.ddr)
sänker MIR direkt till x86-64 med en 11-registersallokator (`callee-saved` m.fl.)
 och System V-ABI, medan [Wasm-MIR-sänkning](../selfhost/backend/wasm_mir.ddr)
sänker MIR direkt till WebAssembly-lokaler och strukturerade kontrollblock.
x86_64-windows sänker samma MIR-producerade symboliska ström till Win64 via
[x86-sänkningen](../selfhost/backend/x86.ddr).
På x86-64 pekar R15 på Dunders privata registerbank. Dunder använder en
kontrollerad värdestack och nativa System V-anrop (eller Win64-anrop) använder värdstacken.
På Wasm blir de nåbara funktionerna Wasm-funktioner med strukturerade hopp
mellan grundblocken; `wasm32` använder värdimporten `dunder` medan `wasm32-wasi`
kopplar standardutskrift och exit till `wasi_snapshot_preview1`. Callback-bryggan
sparar tillståndet över nästlade anrop. Allokerarregistren X19–X21
behåller sina nya värden så att anropets allokeringar finns kvar.
[x86-64-MIR](../selfhost/backend/x86_mir.ddr) · [Wasm-MIR](../selfhost/backend/wasm_mir.ddr) · [PE32+-skrivare](../selfhost/backend/pe.ddr)

Vanliga Linuxbilder innehåller egen startkod och direkta systemanrop, utan
dynamisk laddare eller libc. Compile-time-predikatet `target_is("...")` i
`if`/`else if`-grenar utvärderas vid kompilering och hoppar över onåbara
grenar innan `@extern`-symboler registreras; exempelvis bygger `std.process`
statiska ELF-binärer på Linux via `sys.linux_syscall` och `sys.envp` utan
`libc` eller `libdl`. Nåbara `@extern`-deklarationer ger en dynamisk
ELF med deklarerade bibliotek och symboler, separata RX/RW-segment och
målets C-ABI. Grafikexemplen i testbädden
[dunder-hundmat](https://github.com/Mik-pe/dunder-hundmat) använder dessa
importer för att ladda och anropa grafiken. Stacken är inte körbar och
kodsegmentet är inte skrivbart. Linux-retur och runtimefel använder
`exit_group` så att även grafikdrivrutinens arbetstrådar avslutas.
Systembiblioteket anpassar anropsnummer, argument, metadata-layout och felkoder
till målet. Dunders interna funktions-ABI är inte ett publikt C-ABI.

## Lokal verifiering

Bygg med `./tools/bootstrap.sh`, kör `dunder test tests/suite.ddr` på
värdplattformen och välj motsvarande testvärd för korsbyggd kod. QEMU kan
köra korsbyggda Linuxprogram och Node kan instansiera Wasm-moduler.
På macOS kontrollerar `codesign --verify --strict` signaturen på en
Mach-O-binär. [Körkommandon för kontrakt och testgrupper](../selfhost/README.md#verifiering)
