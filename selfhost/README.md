# Kompilatordelar skrivna i Dunder

**Dunder är ännu inte självhostat.** Här finns en körbar Dunder-skriven
kompilator för en kontrollerad skalär delmängd. Den bygger riktiga
ARM64/macOS-program med egen lexer, parser, typkontroll, kodgenerering,
Mach-O-skrivning och SHA-256-signering. Den delegerar inte till Rust,
GCC, LLVM, assembler, länkare eller extern signerare.

```sh
# Körning av de genererade programmen och codesign kräver ARM64/macOS.
# Rust-bootstrapen bygger en kompilator vars källkod är Dunder.
cargo run --release -- build selfhost/main.ddr -o build/dunder-ddr

# Den binären bygger andra Dunder-program på egen hand.
PATH=/no-external-tools ./build/dunder-ddr build examples/hello.ddr -o build/hello-ddr
./build/hello-ddr
# 42
# true

PATH=/no-external-tools ./build/dunder-ddr build examples/fibonacci.ddr -o build/fibonacci-ddr
./build/fibonacci-ddr
# 6765
# true

# Oberoende kontroll; codesign används inte i byggvägen.
codesign --verify --strict build/fibonacci-ddr
```

## Hur bootstrap fungerar

En körbar kompilator kan läsa sin egen källkod och skriva en ny binär.
Den första binären måste redan finnas när kedjan startar:

1. Bygg Rust-bootstrapkompilatorn, stage 0.
2. Låt stage 0 kompilera den Dunder-skrivna kompilatorn till stage 1.
3. Låt stage 1 kompilera samma kompilatorkällkod till stage 2.
4. Kör tester och låt stage 2 bygga stage 3. För ett deterministiskt bygge
   ska stage 2 och stage 3 vara byteidentiska och ge samma resultat i testerna.

Vi har nått steg 2 och verifierat att stage 1 bygger andra program.
**Steg 3 misslyckas fortfarande vid kompilatorns importer.** Prototypen
stöder ännu inte alla funktioner som dess egen källkod och standardbibliotek
använder. Testerna kontrollerar också denna gräns; ingen fallback döljer den.

## Återställning om en kompilatorbinär försvinner

Behåll Rust-bootstrapens versionsmärkta källkod även när självhosting finns.
Med Rust-verktygskedjan installerad kan binärerna byggas från en ren checkout:

```sh
git clone --branch main https://github.com/Mik-pe/dunder.git
cd dunder
cargo build --release
cargo run --release -- build selfhost/main.ddr -o build/dunder-ddr
cargo test
```

Det kräver ingen befintlig Dunder-binär. En framtida självhostad release bör
dessutom behålla en känd fungerande bootstrap-binär per målplattform och
dess källrevision/checksumma. Endast självhostad källkod räcker inte för att
återskapa en kompilator när både alla kompilatorbinärer och bootstrap-källan
är borta; då behövs en ny första implementation eller annan bootstrapväg.

## Ansvar och täckning

| Fil | Ansvar |
| --- | --- |
| [lexer.ddr](lexer.ddr) | UTF-8-text, escapes, strängar/runes, tal, kommentarer, tokenpositioner och interning med std.map och std.set. |
| [compile.ddr](compile.ddr) | Signaturpass, scope, muterbarhet, typkontroll, uttryck, kontrollflöde och direkt generering. |
| [arm64.ddr](arm64.ddr) | Maskinord, etiketter, hoppfixering, register/stack och runtime för skalärer. |
| [macho.ddr](macho.ddr) | Mach-O-layout, CodeDirectory och sidvis SHA-256 med std.bytes och std.sha256. |
| [main.ddr](main.ddr) | Argument, läsning, diagnostik och publicering via temporär fil och rename. |

Stage 1 stöder `i32`, `i64`, `bool`, UTF-8-strängliteraler och stränghandtag,
typade funktioner med högst åtta parametrar, framåtanrop och rekursion,
`:=`, explicita typer, `var`, blockscope, `return` och slututtryck,
slutliga if/else-grenar, while, halvöppna i32-for-loopar, break/continue,
kortslutande booleska operatorer, heltalsaritmetik/jämförelser,
heltalskonvertering, `print` och `assert`.

Vanlig heltalsaritmetik har overflowkontroller och division med noll ger
samma definierade statusar som bootstrapen. Namn är begränsade till ASCII
i denna parserstage; textliteraler har UTF-8 och Unicode-escapes.
Funktionsramar har en gräns på 510 lokala slotar. För djupa uttryck,
block och if-kedjor avvisas med diagnostik. Strängkonkatenering,
strängjämförelser och strängmetoder i indata stöds ännu inte.

Moduler, aggregattyper, referenser/borrows, dynamiska och fasta arrayer,
enums/match, funktionsvärden, flyttal och `@test` i indata återstår i denna
kompilator. Dessa funktioner fungerar redan i Rust-bootstrapen och används
för att skriva stage 1; det är stödet som behöver porteras för självhosting.
Även macOS-/Metal-intrinsics och `.app`-paketering finns tills vidare bara
i Rust-bootstrapens byggväg. Metal-exemplen byggs med den vanliga `dunder`.
Det finns ingen utfästelse om featureparitet eller optimerad körprestanda.

Nästa acceptans är en modulladdare, ett typregister och de samlings-/borrowoperationer
som behövs för att stage 1 ska kunna bygga standardbiblioteket och dessa
källfiler. Därefter krävs en verklig stage 2/stage 3-jämförelse.

## Verifiering

```sh
cargo run --release -- test selfhost/compile.ddr
# 9 passed, 0 failed
cargo test --test selfhost
```

Native-resultat för 800 genererade räkne-/jämförelseuttryck, rekursion,
loopar, Unicode/NUL-utskrift, bool och i64-gränser jämförs med bootstrapens
binärer. Felaktiga program, oföränderlighet, saknade returvärden och
argumenttyper kontrolleras före publicering. Tidigare utdata bevaras vid
kompileringsfel och indata skyddas även genom symlänkar/hårda länkar.
Genererade binärer körs med tom miljö, signaturerna kontrolleras av macOS,
och upprepade byggen ger identiska byte.
