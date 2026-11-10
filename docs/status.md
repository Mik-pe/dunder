# Dunder-status

2026-11-10: första typade nativekompilatorn finns på `language/typed-native`
i `/Users/mikpe/dev/dunder-language`.

## Implementerat

- `.dn` som filändelse för källkod; uppdaterade exempel, manifest och CLI-hjälp.

- Användarens syntax: `x := 42`, `x: i32 = 42`, oföränderlighet som standard.
  `var x := 0` väljer muterbarhet och `x = ...` tilldelar.
- `i32`, `i64`, `bool`, explicita funktionssignaturer, lokala typinferenser,
  blockscope, framåtanrop, rekursion, if/else, while, break/continue.
- Typade funktionsvärden, callbacks och funktioner som returvärden med
  strukturell signaturkontroll. Indirekta anrop, inklusive `choose()(42)`,
  och fler än åtta parametrar med typade stackargument.
- Typfel med fil/rad/kolumn och felkod. Kontrollerad heltalsaritmetik och
  konvertering, samt kortslutande booleska uttryck.
- `dunder check`, `dunder build`, `dunder run`, enkelt dunder.toml, atomisk
  publicering av utdata och `--timings`. `main` ger exitkod 0 eller en explicit
  i32-retur; `run` för vidare exitkoden och kör aldrig gammal utdata vid typfel.
- Direkt kodgenerering från typade funktioner till ARM64. Egen Mach-O-skrivare,
  SHA-256 och ad hoc-signering. Inga externa crates. Den normala byggvägen
  startar inga andra verktyg. Körbara filer behöver ingen språkinterpreter.

## Verifiering

`cargo test`: tolv testfunktioner passerar (en hashreferens och elva
integrationstester). `cargo clippy --all-targets -- -D warnings`, `cargo fmt`
och `cargo build --release` är genomförda.

Releasekompilatorn har dessutom byggt hello och fibonacci med `PATH` satt
till en katalog utan verktyg. Båda binärerna kördes med tom miljö och gav
förväntad utdata. Kör samma väg med `./build/dunder build examples/hello.dn
-o build/hello` efter att ha byggt/kopierat kompilatorn till build/dunder.

`cargo test` kör typkontroll och negativa felprov samt verkliga ARM64-binärer
på denna Mac. Varje native-test verifierar signaturen oberoende med
`codesign --verify`. Programmen körs med tom processmiljö, utan källfil och
utan kompilator i sin körväg.

Exempel: hello → `42`, `true`; fibonacci(20) → `6765`, `true`.
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
Inga poster, strängar, moduler, arrayer/slices eller ägande/lån ännu.
Anonyma funktioner och closures med infångade lokaler återstår.
Overflow/division med noll ger definierad exitstatus;
källpositioner i runtimefel återstår. Backend använder stackplatser och
saknar ännu optimeringar. Inga anspråk på 100k rader/s eller C-paritet.

Den gamla GLM-körningen stoppades efter kvotfelet. Dess ofärdiga fröarbete
ligger kvar orört i `/Users/mikpe/dev/dunder-dogfood`.
