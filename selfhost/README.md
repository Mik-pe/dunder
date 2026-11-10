# Dunder i Dunder

**Dunder-kompilatorn kan bygga sig själv för ARM64/macOS, ARM64/Linux och x86-64/Linux.** Stage 1
bygger stage 2 från dessa källfiler; stage 2 bygger en byteidentisk stage 3.
Stage 3 bygger dessutom fristående program som körs och jämförs med
oberoende förväntade resultat. Fixpunkten har körts direkt på ARM64 för båda systemen och för Linux med QEMU på x86-64.

Lexer, modulladdare, parser, typkontroll, kodgenerering, runtime och
ELF- och Mach-O-skrivare är Dunder-kod. Kompilatorn anropar ingen extern kompilator,
assembler eller länkare. QEMU används bara för att köra ARM64-binärer
på en annan processor.

ARM64-backendens runtime återanvänder minnesblock per storleksklass i en
512 MiB-pool och räknar referenser till dynamiska strängar. Kompilatorn
frigör lokaler, temporära uttryck, arraybuffertar och structfält vid
tilldelning och scope-slut, inklusive retur och loopavbrott. Statiska
strängar har räknare noll. Rust-bootstrapens pooler och ABI beskrivs i
[minnesarkitekturen](../docs/memory.md).

ARM64-vägen kompilerar `f32` och `f64` med IEEE-aritmetik, konverteringar,
jämförelser, bitoperationerna `from_bits`/`to_bits`, avrundning och
klassificering. Decimalliteraler avrundas direkt till begärd precision.
`print` och `.str()` använder samma [decimalformatterare](../runtime/float.ddr)
som Rust-bootstrapen.

## Byggkedjan

Kör från repositoryts rot. På ARM64/Linux kan stage 2 och stage 3 köras
direkt; på andra processorer behövs `qemu-aarch64` för dessa steg.

```sh
# Stage 0: den befintliga Rust-bootstrapen.
cargo build --release

# Stage 1: Dunder-källor, kompilerade till värddatorns nativeformat.
target/release/dunder build selfhost/linux.ddr -o build/stage1

# Stage 2: samma Dunder-källor, kompilerade av stage 1 till ARM64/Linux.
PATH=/no-external-tools build/stage1 build selfhost/linux.ddr -o build/stage2

# Stage 3: samma källor igen, nu kompilerade av stage 2.
qemu-aarch64 build/stage2 build selfhost/linux.ddr -o build/stage3
cmp build/stage2 build/stage3

# Den självbyggda kompilatorn bygger ett annat program.
qemu-aarch64 build/stage3 build examples/hello.ddr -o build/hello-selfhost
qemu-aarch64 build/hello-selfhost
# 42
# true
```

`selfhost/linux.ddr` skriver alltid ARM64/Linux, oberoende av stage 1:s
värdplattform. `selfhost/main.ddr` skriver signerade Mach-O-binärer för ARM64/macOS.
På en Mac går samma byggkedja att köra med `main.ddr` och utan QEMU.
`selfhost/x86_linux.ddr` skriver x86-64/Linux. Alla tre använder samma
modulladdare, typkontroll och native-runtime. Dunder-backenden för x86-64
översätter den gemensamma kompilatorns instruktioner och symboliska hopp före
ELF-skrivningen. Linux-entry, registerbank, stackgränser och syscall-ABI är
specifika för x86-64. Så här verifieras fixpunkten på en x86-64/Linux-värd:

```sh
dunder build selfhost/x86_linux.ddr -o build/x86-stage1
build/x86-stage1 build selfhost/x86_linux.ddr -o build/x86-stage2
build/x86-stage2 build selfhost/x86_linux.ddr -o build/x86-stage3
cmp build/x86-stage2 build/x86-stage3
```

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
i `stdlib/*.ddr`, regenerera med Dunder själv:

```sh
target/release/dunder run selfhost/embed.ddr
```

Modulladdaren bevarar källpositioner så att fel pekar på ursprunglig fil
och rad. Den skyddar indata och importerade filer från överskrivning,
även via symboliska och hårda länkar. Utdata publiceras med temporär fil
och rename efter ett lyckat bygge.

Vid vanliga byggen kontrollerar självhosten nåbara funktionskroppar. Den
stöder skalära arrayelement och enkla enumvarianter i arrayer. Den generella
lånekontrollen, alla typer av nästlade arrayelement och en fullständig
paritetskontroll av ogiltiga program återstår. Syntax- och typfel från
självhosten rapporteras fortfarande med den gemensamma koden E1000.
Rust-bootstrapen behövs för hela CLI:t och paketeringen av en macOS-app.

## Filer

| Fil | Ansvar |
| --- | --- |
| `lexer.ddr` | Tokenisering, UTF-8, escapes, positioner och interning. |
| `modules.ddr` | Modulgraph, namn, synlighet, deduplicering och källmappning. |
| `bundled.ddr`, `embed.ddr` | Inbäddat standardbibliotek och dess Dunder-generator. |
| `compile.ddr` | Typer, uttryck, kontrollflöde, värdesemantik och kodgenerering. |
| `x86.ddr`, `x86_linux.ddr` | x86-64-kodgenerering och Linux-ingång. |
| `arm64.ddr` | Maskinord, register, stack, etiketter och hoppfixering. |
| `runtime.ddr`, `runtime_sys.ddr` | Minne, samlingar, strängar och Darwin- och Linux-systemanrop. |
| `driver.ddr` | Gemensam modulladdning, felrapportering och publicering av filer. |
| `elf.ddr`, `linux.ddr` | ELF-format och kompilatorns Linux-ingång. |
| `macho.ddr`, `main.ddr` | Signerad Mach-O och kompilatorns macOS-ingång. |

## Verifiering

```sh
cargo test --test selfhost -- --test-threads=1
target/release/dunder test
```

Det gemensamma testet bygger stage 1 och stage 2 på macOS och Linux. På
ARM64/macOS verifierar det fixpunkten och kör Tabell direkt. Med `qemu-aarch64` tillgängligt
kör det även stage 2 → stage 3, jämför samtliga byte och kör program byggda
av stage 3. Ange vid behov `DUNDER_QEMU_AARCH64=/sökväg/till/qemu-aarch64`.
Testet rapporterar när QEMU-delen hoppas över. På ARM64/Linux körs binärerna
direkt. Kompilatorprocesserna körs med tom miljö och utan externa verktyg i
`PATH`.

Regressionerna täcker variabla bitförskjutningar, heltal, nästlade structar,
oberoende arraykopior, UTF-8, moduler, diagnostik och skydd av källfiler.
Stage 3 bygger och kör de tolv icke-grafiska dogfood-programmens
`main` på ARM64/macOS och ARM64/Linux. Testet jämför stdout, stderr och
returkod med Rust-bootstrapen. Stage 3 bygger även det fristående CSV-projektet Tabell och dess rapport
jämförs med förväntad CSV. En lokal körning på inventeringen av Blixts
källfiler gav också samma gruppsummor som den oberoende referensen.
Ett separat test ser till att det inbäddade standardbiblioteket är aktuellt.
De tidigare macOS-testerna för 800 genererade uttryck, runtimefel och
signaturer finns kvar och körs på ARM64/macOS.

Behåll Rust-bootstrapens källkod som återställningsväg. En ren checkout
kan byggas med Rust utan någon befintlig Dunder-binär. Versionsmärkta
bootstrap-binärer och deras källrevisioner kan senare göra installationen
enklare; självhosting ersätter inte behovet av en första fungerande binär.
