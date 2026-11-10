# Dunder-status

2026-11-10: första typade nativekompilatorn finns på `language/typed-native`
i `/Users/mikpe/dev/dunder-language`.

## Implementerat

- Standardbibliotek i Dunder: `std/map`, `std/set`, `std/array`, `std/text`,
  `std/hash`, `std/bytes`, `std/sha256`, `std/parse` och sys. Map har strängnycklar
  och i64-värden, set innehåller strängar. Modulerna är inbäddade och importeras
  utan suffix. Hashning, sondering, omhashning och algoritmer är Dunder-kod.
- Dynamiska `[]T`, geometrisk tillväxt, reserve/push/pop/clear, kontrollerade
  index och iteration. Oberoende djupa värdekopior, också inne i aggregat.
  `&T` och `&mut T` lånar under funktionsanrop; referenser kan inte lagras
  eller returneras. Se [standardbibliotekets kontrakt](standard.md).
- Nästlade structar, fasta/dynamiska arrayfält, enumfält och funktionsfält.
  Inline-cykler avvisas och returvärden kopieras till anroparägd lagring.
- Explicita heltalsmetoder för bitoperationer, kontrollerade skiftantal och
  wrapping-aritmetik. SHA-256 och bytekodning använder dem som dogfood.
- `.ddr` som filändelse för källkod; uppdaterade exempel, manifest och CLI-hjälp.
  Användarens val ersätter den tidigare tvåbokstavsändelsen. Importer saknar fortsatt suffix.
- En fil per modul: `import math` eller `import "util/math" as math`, utan
  suffix. Privata `fn` och exporterade `pub fn`, alias och kvalificerade namn.
  Samlade signaturer stödjer cykliska importer och ömsesidig rekursion.
  Kanoniska filsökvägar deduplicerar delade beroenden och symboliska länkar.
  Importerade funktioner är vanliga typade funktionsvärden.
- Nominella `struct`/`pub struct`, typade fält, struct-uttryck med vanliga
  uttryck som fältvärden, kvalificerade typer och konstruktorer över moduler.
  Structar kopieras vid bindning, tilldelning och anrop; `var` medger fältändring.
- `enum`/`pub enum` med egna i32-värden, kontextuell `.Variant`, varianter med
  typade payloads och uttömmande `match`. Enkla enums fungerar i structfält
  och arrayer. Data-enums kan innehålla structar, arrayer, funktionsvärden
  och andra enums; de kopieras och returneras med värdesemantik.
  Taggar och direkta grenar används utan vtables. Se [enumreglerna](enums.md).
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
- Giltig, oföränderlig UTF-8 med typen `string`, rune-literaler och typen
  `rune`, escapes, NUL, utskrift, sammanslagning, innehållsjämförelse,
  kontrollerade byteindex och strängsnitt. `.len` räknar byte; `.bytes()`
  och `.runes()` är återanvändbara vyer med indexering och `for`.
  Heltal, bool och runes har `.str()`. Strängar kan returneras, gå genom
  callbacks och lagras i structfält och arrayer. Dynamiska strängar använder
  en begränsad processarena utan GC; se begränsningarna nedan.
- `f32`/`f64`, kontexttypade decimal- och exponentliteraler, IEEE-aritmetik
  och jämförelser, NaN, oändlighet, signerad noll och subnormala tal.
  Explicita numeriska konverteringar; till heltal trunkeras och kontrolleras
  intervallet. Grundläggande matematik och klassificering, råbitkonvertering,
  flyttalsfält/-arrayer, callbacks, returer och oberoende FP-/heltalsregister
  med stackspill. Decimalutskriften är skriven i `runtime/float.ddr`, bäddas
  in vid behov och använder exakta intervall för korta roundtrip-värden.
- Medföljande `import sys` för ARM64/macOS: textläsning/-skrivning, append,
  metadata, kataloger, rename, borttagning, process-ID och felmeddelanden.
  Nominella resultatstructar har `.ok`/`.error`, läst text finns i `.value`.
  Vanliga I/O-fel avbryter inte processen. UTF-8 valideras före exponering.
  Biblioteket är inbäddat; det kräver inga lokala biblioteksfiler.
- `sys.read` ger `ReadResult { Ok(string), Err(IoError) }`. Namngivna fel,
  `Other(i32)` för okända koder, felkonverteringar och `FileKind` är skrivna
  i Dunder ovanpå det första sys-API:t.
- Byteutdata, kontrollerad UTF-8-avkodning, programargument, identifiering
  av samma fil genom hårda/symboliska länkar och explicit executable-läge.
- Kodgenereringen tar bort onåbara funktioner och oanvänd flyttalsformatering.
  Små lokala stackåtkomster använder färre instruktioner; minneskopiering
  använder hela ord där det går. ASCII-källfiler behöver ingen separat
  teckenindextabell. Långa hopp/adresser fungerar över 1 MiB och stora
  signaturer får tillräcklig virtuell lagring i Mach-O.
- [En Dunder-skriven kompilator](../selfhost/README.md) bygger den skalära
  delmängden till signerad ARM64/Mach-O utan extern kompilator. Lexer,
  parser, typkontroll, kodgenerering, filformat och signering är Dunder-kod.
  Den bygger ännu inte hela sin egen källkod; självhosting är inte uppnådd.
- Structar och fasta arrayer kan returneras med kopiering till anroparens
  lagring, även genom callbacks och rekursion. Ingen pekare till avslutad
  stacklagring returneras.
- `dunder check`, `dunder build`, `dunder run`, `dunder test`, enkelt dunder.toml
  med valfri `test_root`, atomisk
  publicering av utdata och `--timings`. `main` ger exitkod 0 eller en explicit
  i32-retur; `run` för vidare exitkoden och kör aldrig gammal utdata vid typfel.
- Direkt kodgenerering från typade funktioner till ARM64. Egen Mach-O-skrivare,
  SHA-256 och ad hoc-signering. Inga externa crates. Den normala byggvägen
  startar inga andra verktyg. Körbara filer behöver ingen språkinterpreter.

## Verifiering

`cargo test`: 80 testfunktioner passerar (två signerings-/bildtester, sexton
dogfood-, elva språk-, nio modul-, åtta sträng-, åtta flyttals-, tio system-,
sex enum-, sju collection- och tre tester av den Dunder-skrivna kompilatorn).
`cargo clippy --all-targets -- -D warnings`, `cargo fmt`
och `cargo build --release` är genomförda.

Releasekompilatorn har dessutom byggt hello och fibonacci med `PATH` satt
till en katalog utan verktyg. Båda binärerna kördes med tom miljö och gav
förväntad utdata. Kör samma väg med `./build/dunder build examples/hello.ddr
-o build/hello` efter att ha byggt/kopierat kompilatorn till build/dunder.

`dunder test` kör nu 44 Dunder-testgrupper i dogfood-trädet: rekursion,
moduler, insertion sort, intervall, arrayiteratorer, callbacks, indexbaserad
arena, tabellfall, structargument, UTF-8, en liten tokenizer, numeriska program
och filsystem, namngivna enumvärden, nästlade payload-varianter, standardbibliotek
och kompilatorns första Dunder-skrivna delar.
sys-bibliotekets felmeddelanden och typade felkoder har egna kopplade testblock.
Varje grupp kompileras till en riktig
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

Strängtesten kör svenska, grekiska, japanska, arabiska, emoji, kombinerande
tecken, NUL och Unicode-gränsvärden. Bytevärden, kodpunkter, längder och
snitt jämförs mot oberoende Rust-resultat. Körda fall omfattar returnerade
strängar, callbacks, structar, arrayer, stackparametrar, iterationsvyer,
heltalsformattering inklusive i64-minimum och avsiktligt felaktiga testresultat.
Felaktig UTF-8, surrogates och feltyper avvisas vid kompilering; negativa
index, fel snittgränser, ogiltiga rune-konverteringar och full strängarena
ger verifierade felstatusar. Textstatistik och tokenizern använder `find`,
`replace`, ordräkning och UTF-8-hjälpare skrivna i Dunder.

Flyttalstesten jämför 648 native-aritmetikresultat med oberoende Rust-resultat
och återläser hundratals utskrivna f32/f64-bitmönster med Rusts parser.
De täcker även direkt decimalavrundning till f32, båda registerbankerna,
spill till stack, callbacks, rekursion, array-/structlayout, klassificering,
NaN-jämförelser, signerad noll, gradvis underflöde och felaktiga konverteringar.
Det numeriska Dunder-trädet provar omskalad hypotenusa, kompenserad summering,
rotberäkning med bisektion och fjärde ordningens Runge–Kutta-integration.
Alla exempel, importer och testfixturer använder nu `.ddr`.

Systemtesterna kör verklig fil-I/O i isolerade temporära kataloger och
kontrollerar innehåll med Rusts oberoende fil-API. Verifierat: Unicode och NUL
i text, 64-bitars filstorlek, append, trunkering, rename, metadata, borttagning,
symboliska länkar, saknade filer, icke-tomma kataloger, NUL/längd i sökvägar,
ogiltig UTF-8, för stora filer och full strängarena. Hundratals lyckade och
misslyckade läsningar lämnar oförändrat antal öppna handtag.
Binärens signatur verifieras oberoende. En byte-reproducerbar binär fungerar
med tom miljö efter att källfilerna tagits bort. Returproven omfattar structar, fasta/tomma arrayer,
callbacks, rekursion, blandade parametrar och upprepade anrop.

Enumtesten kontrollerar nominell identitet och privata typer över moduler,
taggvärden och kontrollerad heltalskonvertering, saknade/dubbla matchfall,
payloadtyper, oföränderliga bindningar och kopior av matchvärdet. Nativeprov
omfattar strängar, f32/f64, structar, arrayer, nästlade enums, funktionsvärden,
rekursion, callbacks, stackargument och verkliga `Ok`/`Err`-filresultat.

`cargo test` kör typkontroll och negativa felprov samt verkliga ARM64-binärer
på denna Mac. Fristående nativeprov verifierar signaturen oberoende med
`codesign --verify` och körs med tom processmiljö, utan källfil och utan
kompilator i sin körväg. CLI-proven bygger och kör med PATH utan externa
verktyg; samma väg är verifierad med releasekompilatorn för hela testsamlingen
och structprogrammet. UTF-8-programmet, tokenizern, numerikprogrammet,
filsystemsprogrammet, enumprogrammet, standardbiblioteket och alla 44 Dunder-testgrupper är också körda med
releasekompilatorn utan verktyg i PATH.

Exempel: hello → `42`, `true`; fibonacci(20) → `6765`, `true`.
Modulprogrammet i examples/modules/main.ddr laddar tre filer och skriver
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
Stage 1 bygger skalära program men avvisar ännu importer och aggregat i
sin indata. Nästa steg är funktionerna som krävs för att kompilera dess egen
källkod och standardbibliotek; därefter stage 2/stage 3-verifiering.
Rust-bootstrapen behålls som återställningsväg.
Arrayelement är begränsade till primitiva skalärer och enums utan payload.
Fasta arrayer och inline-aggregat får vara högst 64 KiB. Nästlade aggregat
fungerar som structfält och enumpayloads. Arrayslices, generiskt allokerings-API
och generell ägar-/lånekontroll återstår. Borrows är tills vidare begränsade
till funktionsparametrar. Dynamiska arraybuffertar använder en separat
512 MiB processarena och återvinns inte individuellt.
Strängliteraler bor i programbilden. Dynamiska strängar bor i en separat
16 MiB arena, som mappas vid första behov och återlämnas vid processlut.
Enskilda strängar återvinns ännu inte under körning; upprepade operationer
kan fylla arenan. Detta är en första dogfood-körmodell utan GC, inte färdig
ägande-/lånekontroll. Normalisering, grapheme-kluster, interpolation och
fullständig Unicode-klassificering återstår.
Dogfood-arenan är bump-allokering över en fast stackbuffert med index som
handtag; den är ännu inget allmänt allokerings- eller låne-API.
Iteratorer är inbyggda arraykopior, inte ett användardefinierat protokoll.
Globala konstanter, paketregister och dynamiska bibliotek återstår.
Ingen inkrementell cache.
Enums saknar ännu generics, likhet för data-enums, mönsterguards och generell
`match` som uttryck. Direkt rekursiva inlinevärden kräver indirection;
arena-dogfood använder indexhandtag. Se [enumreglerna](enums.md).
sys riktar sig bara till macOS/ARM64. Textläsning kräver vanliga UTF-8-filer,
högst 4 MiB per fil, och delar strängarenans 16 MiB. Binärläsning, kataloglistning,
rekursiva katalogverktyg och filströmmar återstår. Se [sys-kontraktet](sys.md).
Flyttal saknar än så länge fullständigt matematikbibliotek, trigonometriska
funktioner, exp/log, strängparser och valbar utskriftsprecision. Den Dunder-skrivna
formatteraren prioriterar korrekthet; prestanda är inte uppmätt.
Anonyma funktioner och closures med infångade lokaler återstår.
Overflow/division med noll, gränsfel, assertions, full strängarena och ogiltiga
Unicode-operationer ger definierad exitstatus;
källpositioner och värden i runtimefel återstår. Backend använder stackplatser
och saknar ännu registerallokering och avancerade optimeringar.
[Byggmätningarna](../benchmarks/README.md) avser ett genererat stresskorpus;
de bevisar inte hela 100k-rader/s-kontraktet eller C-paritet.

Samlingstesterna jämför 6 000 operationer med oberoende Rust-map/set,
inklusive Unicode/NUL, kollisioner, tombstones och uppdateringar. De provar
elementbredder, djupa kopior, snapshots vid anrop, inbäddade aggregat och
begränsade borrows. Nativeprogram över 1 MiB provar långa adresser/hopp;
en 9 MiB-bild med flersidig signatur verifieras och körs av macOS.
Den Dunder-skrivna kompilatorn jämförs med bootstrapen för 800 genererade
uttryck samt kontrollflöde och felstatusar. macOS verifierar dess signaturer.

Den gamla GLM-körningen stoppades efter kvotfelet. Dess ofärdiga fröarbete
ligger kvar orört i `/Users/mikpe/dev/dunder-dogfood`.
