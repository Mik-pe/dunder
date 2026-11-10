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
cargo build --release
target/release/dunder run examples/hello.ddr
target/release/dunder build examples/hello.ddr --target x86_64-linux
target/release/dunder build examples/hello.ddr --target arm64-linux
target/release/dunder build examples/hello.ddr --target arm64-macos
target/release/dunder build examples/hello.ddr --target wasm32
```

Utdata går till `build/<target>/<name>` om `-o` saknas. Wasm-filer får ändelsen `.wasm`. Samma regler gäller
projekt med `dunder.toml`. Rusts vanliga tripplar `aarch64-apple-darwin`,
`aarch64-unknown-linux-gnu` och `x86_64-unknown-linux-gnu` accepteras också som
alias. Vanliga Linuxbinärer behöver inte glibc; Vulkan-program använder
systemets dynamiska loader och grafikbibliotek.

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
fältstorlekar. Wasm-backenden sänker typkontrollerade villkor, loopar och
`match` direkt till strukturerade Wasm-instruktioner. Modulen exporterar
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
referensräknade strängar, scratch-regioner och adresserbara lokaler. En
Wasm-fälla avslutar anropet. Skapa en ny instans om programmet ska startas
om efter ett sådant fel. Program som behöver filer eller fönster anger
värdens API via importer. `std.sys.decode_utf8` och `decode_u8` fungerar i
linjärt minne.

Kör `cargo test --test wasm` med Node installerat för körprov av bland annat
värdanrop, C-layout, callback, minnesåteranvändning, Vulkan-scenens geometri
och SHA-256. WebGPU-exemplet inkluderar en webbläsarvärd och gemensamma
Dunder-vertexdata. Se [WebGPU-triangeln](../dogfood/webgpu/README.md) och
[den Dunder-skrivna Wasm-kompilatorn](../selfhost/README.md).

## Bootstrap och kodgenerering

Rust används för att bygga bootstrapkompilatorn. Rusts egen verktygskedja
använder normalt LLVM; det ingår inte som beroende i Dunderkompilatorn eller
i Dunders byggväg. Projektet har inga externa Rust-crates. Den Dunder-skrivna
kompilatorn i `selfhost/` bygger sig själv till ARM64/Linux med byteidentisk
stage 2/stage 3; full språk- och plattformsparitet återstår.

Frontend, typkontroll och standardbibliotek delas mellan målen. ARM64-backenden
skriver instruktioner direkt. Den första x86-64-backenden sänker samma interna
instruktionsvokabulär till x86-64 under kompileringen. De vanligaste
arbetsregistren och värdestackpekaren har fasta CPU-register; övriga virtuella
register har minneslagring. En privat värdestack använder en 64 MiB virtuell minnesmappning
med skyddssida och kontrollerade stackgränser (exitkod 110 vid full stack).
Full registerallokering för lokala variabler återstår. Den färdiga filen innehåller x86-64-maskinkod, inte en
interpreter eller emulator. Dess prestanda ska mätas separat från ARM64.
Läs [maskinkodsoptimeringen](optimization.md) och dess mätunderlag.

Vanliga Linuxbilder innehåller egen startkod och direkta systemanrop, utan
dynamisk laddare eller libc. Nåbara Vulkan-anrop väljer en egen dynamisk ELF
med eager imports av libc- och loaderfunktioner, separata RX/RW-segment och
ett native C-ABI. Vulkan-körning med lavapipe är verifierad på både
ARM64/Linux och x86-64/Linux. Stacken är inte körbar och kodsegmentet
är inte skrivbart. Linux-retur och runtimefel använder `exit_group` så att
även grafikdrivrutinens arbetstrådar avslutas.
Systembiblioteket anpassar anropsnummer, argument, metadata-layout och felkoder
till målet. Dunders interna funktions-ABI är inte ett publikt C-ABI.

## Lokal verifiering

`cargo test` kör språk-, standardbiblioteks- och systemtester på värdplattformen.
Måltesterna bygger dessutom alla fyra mål med en tom verktygssökväg,
kontrollerar reproducerbarhet och ELF-/Mach-O-header samt avvisning av främmande
körmål. Ingen GitHub CI används.

ARM64/Linux kan provas på en x86-64-utvecklarmaskin med `qemu-aarch64` efter
korskompilering. QEMU är då ett separat testverktyg och krävs inte av
kompilatorn eller de färdiga programmen. En macOS-binärs faktiska körning och
OS-verifiering av signaturen måste fortfarande provas på macOS.

[tools/verify-arm64.rs](../tools/verify-arm64.rs) är en lokal, valfri verifierare
för exempel, alla dogfood-testgrupper och ARM64/Linux-specifika kantfall.
Bygg- och körinstruktioner står överst i filen; ange sökvägen till QEMU
som verifierarens första argument vid behov.
