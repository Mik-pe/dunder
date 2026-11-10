# Dunder i Dunder

**Dunder-kompilatorn kan nu bygga sig själv för ARM64/Linux.** Stage 1
bygger stage 2 från dessa källfiler; stage 2 bygger en byteidentisk stage 3.
Stage 3 bygger dessutom fristående program som körs och jämförs med
oberoende förväntade resultat. Hela kedjan är verifierad på Linux/x86-64
med QEMU för ARM64-körning.

Lexer, modulladdare, parser, typkontroll, kodgenerering, runtime och
ELF-skrivare är Dunder-kod. Kompilatorn anropar ingen extern kompilator,
assembler eller länkare. QEMU används bara för att köra ARM64-binärer
på en annan processor.

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
värdplattform. Den äldre ingången `selfhost/main.ddr` finns kvar för den
skalära ARM64/macOS-byggvägen. Självbyggd macOS- och x86-64-kodgenerering
återstår; Rust-bootstrapen stöder redan alla tre målen.

## Vad kompilatorn klarar

Självbygget använder punktimporter, alias, privata/exporterade symboler,
cykliska modulgrafer, structar med djup värdekopiering, fält och lån,
dynamiska arrayer, enkla enums, UTF-8-strängar, strängsnitt och iteration.
Funktioner stöder framåtanrop, rekursion, aggregatreturer och fler än åtta
parametrar. Heltalsaritmetik, bitoperationer, kontrollflöde, `print`,
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

Detta är ännu en mindre implementation än Rust-bootstrapen. Bara nåbara
funktionskroppar kompileras och kontrolleras; `@test`-block tas bort vid
modulladdning. Fullt stöd för fasta arrayer, payload-enums/`match`, flyttal,
funktionsvärden, generell lånekontroll och alla standardbiblioteksoperationer
återstår. Självhosting betyder att kompilatorn bygger sin egen källkod;
det betyder ännu inte full språkparitet.

macOS-/Metal-intrinsics och `.app`-paketering finns tills vidare bara
i Rust-bootstrapens byggväg. Metal-exemplen byggs med den vanliga `dunder`.

## Filer

| Fil | Ansvar |
| --- | --- |
| `lexer.ddr` | Tokenisering, UTF-8, escapes, positioner och interning. |
| `modules.ddr` | Modulgraph, namn, synlighet, deduplicering och källmappning. |
| `bundled.ddr`, `embed.ddr` | Inbäddat standardbibliotek och dess Dunder-generator. |
| `compile.ddr` | Typer, uttryck, kontrollflöde, värdesemantik och kodgenerering. |
| `arm64.ddr` | Maskinord, register, stack, etiketter och hoppfixering. |
| `runtime.ddr`, `runtime_sys.ddr` | Minne, samlingar, strängar och Linux-systemanrop. |
| `elf.ddr`, `linux.ddr` | ELF-format och kompilatorns Linux-ingång. |
| `macho.ddr`, `main.ddr` | Den äldre signerade Mach-O-byggvägen. |

## Verifiering

```sh
cargo test --test selfhost -- --test-threads=1
target/release/dunder test
```

Linux-testet bygger stage 1 och stage 2. Med `qemu-aarch64` tillgängligt
kör det även stage 2 → stage 3, jämför samtliga byte och kör program byggda
av stage 3. Ange vid behov `DUNDER_QEMU_AARCH64=/sökväg/till/qemu-aarch64`.
Testet rapporterar när QEMU-delen hoppas över. På ARM64/Linux körs binärerna
direkt. Kompilatorprocesserna körs med tom miljö och utan externa verktyg i
`PATH`.

Regressionerna täcker variabla bitförskjutningar, heltal, nästlade structar,
oberoende arraykopior, UTF-8, moduler, diagnostik och skydd av källfiler.
Stage 3 bygger även det fristående CSV-projektet Tabell och dess rapport
jämförs med förväntad CSV. En lokal körning på inventeringen av Blixts
källfiler gav också samma gruppsummor som den oberoende referensen.
Ett separat test ser till att det inbäddade standardbiblioteket är aktuellt.
De tidigare macOS-testerna för 800 genererade uttryck, runtimefel och
signaturer finns kvar och körs på ARM64/macOS.

Behåll Rust-bootstrapens källkod som återställningsväg. En ren checkout
kan byggas med Rust utan någon befintlig Dunder-binär. Versionsmärkta
bootstrap-binärer och deras källrevisioner kan senare göra installationen
enklare; självhosting ersätter inte behovet av en första fungerande binär.
