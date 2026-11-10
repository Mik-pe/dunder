# Dunder-status

2026-11-10: första typade nativekompilatorn finns på `language/typed-native`
i `/Users/mikpe/dev/dunder-language`.

## Implementerat

- `.dn` som filändelse för källkod; uppdaterade exempel, manifest och CLI-hjälp.
- En fil per modul: `import math` eller `import "util/math" as math`, utan
  suffix. Privata `fn` och exporterade `pub fn`, alias och kvalificerade namn.
  Samlade signaturer stödjer cykliska importer och ömsesidig rekursion.
  Kanoniska filsökvägar deduplicerar delade beroenden och symboliska länkar.
  Importerade funktioner är vanliga typade funktionsvärden.
- Nominella `struct`/`pub struct`, typade fält, struct-uttryck med vanliga
  uttryck som fältvärden, kvalificerade typer och konstruktorer över moduler.
  Structar kopieras vid bindning, tilldelning och anrop; `var` medger fältändring.
- Användarens syntax: `x := 42`, `x: i32 = 42`, oföränderlighet som standard.
  `var x := 0` väljer muterbarhet och `x = ...` tilldelar.
- `i32`, `i64`, `bool`, explicita funktionssignaturer, lokala typinferenser,
  blockscope, framåtanrop, rekursion, if/else, while, break/continue.
- Typade funktionsvärden, callbacks och funktioner som returvärden med
  strukturell signaturkontroll. Indirekta anrop, inklusive `choose()(42)`,
  och fler än åtta parametrar med typade stackargument.
- Sista uttrycket utan semikolon ger funktionens returvärde, även i slutliga
  `if/else`-grenar. Explicit `return` fungerar parallellt.
- Fasta typade arrayer, literaler och repetition, indexläsning/-skrivning,
  `len`, värdekopiering, gränskontroller, `for` över halvöppna `i32`-intervall,
  arrayer och ägda arrayiteratorer. `.iter()` tar en återanvändbar kopia.
- `@test fn` och kopplade `@test { (args) -> expected }`-block. Argument och
  förväntade värden är fullständiga uttryck, inklusive structargument.
  Bool-uttryck i testblock blir assertions; vanliga satser och loopar stöds.
  Testkortformen pekar alltid på den annoterade funktionen även vid lokal skuggning.
- Typfel med fil/rad/kolumn och felkod. Kontrollerad heltalsaritmetik och
  konvertering, samt kortslutande booleska uttryck.
- `dunder check`, `dunder build`, `dunder run`, `dunder test`, enkelt dunder.toml
  med valfri `test_root`, atomisk
  publicering av utdata och `--timings`. `main` ger exitkod 0 eller en explicit
  i32-retur; `run` för vidare exitkoden och kör aldrig gammal utdata vid typfel.
- Direkt kodgenerering från typade funktioner till ARM64. Egen Mach-O-skrivare,
  SHA-256 och ad hoc-signering. Inga externa crates. Den normala byggvägen
  startar inga andra verktyg. Körbara filer behöver ingen språkinterpreter.

## Verifiering

`cargo test`: 37 testfunktioner passerar (en hashreferens, sexton dogfood-,
elva språk- och nio modultester). `cargo clippy --all-targets -- -D warnings`, `cargo fmt`
och `cargo build --release` är genomförda.

Releasekompilatorn har dessutom byggt hello och fibonacci med `PATH` satt
till en katalog utan verktyg. Båda binärerna kördes med tom miljö och gav
förväntad utdata. Kör samma väg med `./build/dunder build examples/hello.dn
-o build/hello` efter att ha byggt/kopierat kompilatorn till build/dunder.

`dunder test` kör nu tolv Dunder-testgrupper i dogfood-trädet: rekursion,
moduler, insertion sort, intervall, arrayiteratorer, callbacks, indexbaserad
arena, tabellfall och structargument. Varje grupp kompileras till en riktig
nativebinär. Körningarna omfattar även förväntade fel och en tidsgräns:
ett misslyckat eller fastnat test hindrar inte nästa grupp. Tester kräver
ingen main, kör inte befintlig main och dedupliceras över importer.
Manifestets `test_root` och den vanliga programroten provas separat.

Structtesten kontrollerar typidentitet över moduler, felaktiga/saknade/dubbla
fält, oföränderlighet, kopior, fältens evalueringsordning, tomma structar,
blandad bool/i64/i32-layout och structargument på stacken. Arraytesten
kontrollerar kopior och iterationskopior, blandade elementtyper, fler än
åtta parametrar, gränsfel och intervall vid i32-gränserna. Kortformen provas
mot felaktiga resultat och lokal skuggning så att testfallen verkligen kan fallera.

`cargo test` kör typkontroll och negativa felprov samt verkliga ARM64-binärer
på denna Mac. Fristående nativeprov verifierar signaturen oberoende med
`codesign --verify` och körs med tom processmiljö, utan källfil och utan
kompilator i sin körväg. CLI-proven bygger och kör med PATH utan externa
verktyg; samma väg är verifierad med releasekompilatorn för hela testsamlingen
och structprogrammet.

Exempel: hello → `42`, `true`; fibonacci(20) → `6765`, `true`.
Modulprogrammet i examples/modules/main.dn laddar tre filer och skriver
`42`, `true`, `42`. Det är byggt/kört även med PATH utan externa verktyg.
Modultesterna kontrollerar privat åtkomst, namn-/aliaskrockar, saknade filer,
otillåtna importsuffix, syntax-/typfel i beroenden, rot-main, callbacktyper
över modulgränser, cykler, diamanter och symlinkdeduplicering. Identiska
projekt i olika kataloger ger identiska binärer; ändrade beroenden ändrar
körresultatet. Typfel lämnar tidigare utdata orörd, och utdata får inte
skriva över någon importerad källfil.
Funktionsexemplet skriver `42`, `42` och `-42`. Funktionstesternas negativa
fall kontrollerar felaktig/saknad main, felaktiga returvärden, saknade returer,
och oförenliga callbacksignaturer med läsbara funktionstyper i diagnostiken.
Körda program verifierar ordningen funktion-före-argument och vänster-till-höger,
returnerade funktioner, void-callbacks, bool-returer, utbytbara funktioner,
10 000 indirekta anrop och rekursion med stackargument. Blandade stacktyper
inklusive bool, negativ i32, i64 och funktioner samt ett 40-argumentsanrop är körda.

Testkorpuset omfattar rekursion, register- och stackparametrar, loopstyrning, blockscope,
kortslutning, signed division/rest, i64-minimum, spill och smalnande
konverteringar. Ett genererat jämförelseprov kontrollerar native heltals-
och jämförelseresultat mot oberoende Rust-resultat för i32/i64.
Upprepade kompileringar ger byteidentiska filer. Ett typfel bevarar föregående
binär och utdata får inte skriva över källan.

## Begränsningar och nästa steg

Bootstrapkompilatorn är skriven i Rust. Självhosting är inte uppnådd.
Structfält och arrayelement är än så länge begränsade till i32/i64/bool.
Arrayer får vara högst 64 KiB. Aggregatreturer, nästlade aggregat,
strängvärden, slices, heapallokering och ägande/lån återstår.
Dogfood-arenan är bump-allokering över en fast stackbuffert med index som
handtag; den är ännu inget allmänt allokerings- eller låne-API.
Iteratorer är inbyggda arraykopior, inte ett användardefinierat protokoll.
Globala konstanter, paketregister och dynamiska bibliotek återstår.
Ingen inkrementell cache.
Anonyma funktioner och closures med infångade lokaler återstår.
Overflow/division med noll, gränsfel och assertions ger definierad exitstatus;
källpositioner och värden i runtimefel återstår. Backend använder stackplatser och
saknar ännu optimeringar. Inga anspråk på 100k rader/s eller C-paritet.

Den gamla GLM-körningen stoppades efter kvotfelet. Dess ofärdiga fröarbete
ligger kvar orört i `/Users/mikpe/dev/dunder-dogfood`.
