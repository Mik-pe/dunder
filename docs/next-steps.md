# Nästa steg för Dunder

## Verifiera Windows som kompilatorvärd

Dunder distribuerar en korsbyggd x86-64-kompilator för Windows. Kör följande på en riktig Windowsinstallation och dokumentera Windowsversion, källrevision och resultat.

Med den publicerade 0.11.3-kompilatorn under Wine 8 klarade ett paket utan externa beroenden sina 71 test med `--locked --offline`. Både `exec` och `spawn` följt av `wait` läste barnets slutstatus. Körlistan skrev en JSON-profil till omdirigerat stderr. Upprepa på nativ Windows x64 med ett sökvägsberoende, ett låst Git-beroende från cache och stängt stderr. [Paketsvit](https://github.com/Mik-pe/dunder-tui/blob/b9d76a2ab0224ad24a6abf2c5bebf33c9dbb3aa9/tests/suite.ddr) · [Processprov](https://github.com/Mik-pe/dunder-tui/blob/b9d76a2ab0224ad24a6abf2c5bebf33c9dbb3aa9/tests/dogfood/compiler/windows_child.ddr) · [Tidigare nativa reproduktioner](https://github.com/Mik-pe/dunder/blob/v0.11.3/docs/next-steps.md#verifiera-windows-som-kompilatorvärd)

1. Hämta den publicerade exe-filen, kontrollera SHA-256 och bygg kompilatorn två gånger med ett jobb. Jämför filerna byte för byte. Upprepa med flera jobb.
2. Kör [provet för `check`, `build`, `run`, `test` och språkservern](../tools/test_lsp_and_dwarf.ddr) med kompilatorsökvägen som argument. Det använder ett lokalt paket i en sökväg med mellanslag och svenska tecken. Kör även [provet för miljö, standardfel och processavbrott](../tests/fixtures/process-unicode.ddr) samt [Win64-callbacken](../tests/fixtures/ffi/windows.ddr) med argumenten `"sök väg" "" "🦔"`.
3. Installera och uppgradera i en tom katalog. Prova även bytet medan den installerade filen används. [Installation och distribution](releases.md) · [Plattformskontrakt](targets.md)

Kör även `dunder run tests/fixtures/windows-terminal.ddr` i en konsol.
Provet jämför konsolläget före och efter tidig retur och läser tillbaka två barns utmatning från separata skärmbuffertar.

Privata filer på Windows behöver ett kontrakt för Windows åtkomstkontrollistor.
Verifiera åtkomsten som ett annat användarkonto och utred vilka garantier ett
beständigt katalogbyte kan ge när datorn startas om. [Filrättigheter och synkning](sys.md)

## Omprovat med Dunder 0.11.3

På macOS ARM64 går de tidigare minimiproven för plattformsvillkor, generiska mottagare över modulgränser, specialisering per mottagartyp, matchgrenar, separata fältlån och uppackning av ett enhetsresultat nu att bygga och köra. `otool -L` visar ingen Windowsimport i plattformsprovet. Behåll proven vid ändringar i typkontrollen, lånekontrollen och länkningen. [Minimiprov](https://github.com/Mik-pe/dunder-tui/blob/b9d76a2ab0224ad24a6abf2c5bebf33c9dbb3aa9/tests/dogfood/compiler) · [Tidigare utfall](https://github.com/Mik-pe/dunder/blob/v0.11.3/docs/next-steps.md#utvärdera-plattformsvillkor-före-länkning)

`std.utf16.append` används nu i TUI-bibliotekets Win32-utmatning. Det lokala provet bevarade både U+0000 och ett surrogatpar i samma buffert. [UTF-16](../stdlib/utf16.ddr) · [Konsolskrivaren](https://github.com/Mik-pe/dunder-tui/blob/b9d76a2ab0224ad24a6abf2c5bebf33c9dbb3aa9/src/windows.ddr)

## Låt kortlivade iteratorposter stanna i loopen

När `Iterator.next` i ett prov med 1 024 element returnerade en post med ett lån, ett index och en inre post rapporterade 0.11.2 totalt 5 312 postkopior och 7 489 allokeringsanrop. Direkt läsning av samma byte och vikt gav noll postkopior och ett allokeringsanrop. Båda gav summan 110 080 på macOS och Linux ARM64. Pröva att eliminera kopior genom `Option` och loopbindningen när elementet förbrukas på plats. Kontrollera även ägda element som sparas och att livstidskontrollen fortfarande avvisar lån som lämnar sin ägare. [Mätprogram](https://github.com/Mik-pe/dunder-tui/blob/f3ecfc12e64df502da617593e7197483582b92fe/benchmarks/language/iterator_item.ddr)

För `std.text.graphemes` ger ett andra `next()` E4003 även i 0.11.3 om det första elementets text läses efter anropet. Att läsa det första elementet direkt fungerar. Pröva en returanknytning till iteratorns lånade källtext som låter markören gå vidare medan båda elementen lever. Källan ska förbli lånad till sista läsningen. [Två grafem från samma sträng](https://github.com/Mik-pe/dunder-tui/blob/a94c584/tests/dogfood/compiler/retained_graphemes.ddr)

## Ändra element i en lånad array

0.11.3 föreslår en indexloop för `for cue in &mut cues`. Indexloopen i provet uppdaterar båda posterna. Pröva även direkt iteration med ett muterbart elementlån per varv. En växande array måste behålla sin lagring medan elementet är lånat. [Reproduktionsfall](https://github.com/Mik-pe/dunder-tui/blob/b9d76a2ab0224ad24a6abf2c5bebf33c9dbb3aa9/tests/dogfood/compiler/mutable_elements.ddr) · [Fungerande indexloop](https://github.com/Mik-pe/dunder-tui/blob/b9d76a2ab0224ad24a6abf2c5bebf33c9dbb3aa9/tests/dogfood/compiler/indexed_mutable_elements.ddr)

## Låt en lokal vy läsa från två ägare

En iterator över ändrade celler behöver läsa både föregående och aktuell bild. I 0.11.2 går det att lagra två delade lån från samma array i en post. Två lokala ägare ger E4003 även när de båda lever längre än posten. Utvärdera om en lokal vy kan knytas till båda ägarna. Prova tidigt avslutad iteration samt att fortsatt användning av vyn håller båda ägarna lånade och att båda blir skrivbara efter sista läsningen. [Två ägare](https://github.com/Mik-pe/dunder-tui/blob/v0.13.0/tests/dogfood/compiler/paired_views.ddr) · [Gemensam ägare](https://github.com/Mik-pe/dunder-tui/blob/v0.13.0/tests/dogfood/compiler/paired_views_one_owner.ddr)

## Returnera en valfri delvy av en lånad sträng

I 0.11.2 ger både `check` och `build` E3099 för `.Some(&value[1..value.len])` i en funktion med returtypen `Option<&str> from value`. En post som lagrar källvyn och två byteindex fungerar. Låt en valfri delvy bära sitt intervall och behålla lånet från anroparen. Prova en tom träff, en utebliven träff och UTF-8 efter ett ASCII-prefix. En vy in i en lokal ägd sträng ska fortfarande avvisas. [Minimalt kompilatorprov](https://github.com/Mik-pe/dunder-tui/blob/f3890e7fa9b24e6a9e8dbc3c5d3e3a0011b85b9d/tests/dogfood/compiler/optional_text_view.ddr) · [Körbar variant med byteindex](https://github.com/Mik-pe/dunder-tui/blob/f3890e7fa9b24e6a9e8dbc3c5d3e3a0011b85b9d/benchmarks/language/line_views.ddr)

## Mät stegvis radläsning

`std.text.lines` i 0.11.3 hanterar LF, CRLF, tomrader och ett ensamt avslutande CR. Jämför dess tid och allokeringar med indexläsaren över 3 073 rader, både vid första träffen och vid full genomläsning. [Mätprogram](https://github.com/Mik-pe/dunder-tui/blob/b9d76a2ab0224ad24a6abf2c5bebf33c9dbb3aa9/benchmarks/language/line_views.ddr) · [Standardbibliotekets läsare](../stdlib/text.ddr)

## Jämför två lånade arrayintervall direkt

Direkta arrayvyer går nu att jämföra med `==`. På Dunder 0.11.3 tog 24 radjämförelser om 96 `i64` vardera 9,85 µs med `==`, 9,91 µs med `std.array.equal` och 1,53 µs med `std.bytes.equal_memory` på macOS ARM64. På Linux ARM64 i en lokal VM var motsvarande tider 8,43, 10,12 och 1,14 µs. Pröva specialiserad jämförelse för tätt lagrade heltal så att lånade intervall kan använda samma snabba kod utan råa adresser hos anroparen. Verifiera tomma och förskjutna intervall, tidig och sen skillnad samt användardefinierad `Eq` för andra elementtyper. [Körbart prov med tre lägen](https://github.com/Mik-pe/dunder-tui/blob/b9d76a2ab0224ad24a6abf2c5bebf33c9dbb3aa9/benchmarks/language/row_equality.ddr) · [Lika, olika och tomma vyer](../tests/fixtures/local_borrows/views.ddr)

## Byt två ägda värden utan ett mellanvärde

Två lokala ägare kan byta plats med tre flyttar. Samma kod bakom två `&mut`-parametrar ger E4003. Körlistan och kompilatorns `exchange_data_into` använder därför `replace` med ett mellanvärde. I 0.11.2 gav 4 095 byten av poster med två arrayer 12 304 allokeringsanrop, mot 19 för lokala flyttar och 21 för arrayens `swap`. Av de 12 285 extra anropen var ett en färsk allokering. macOS och Linux ARM64 gav samma slutordning och profiler. Pröva platsbyte bakom två muterbara lån utan `Default` eller allokering. Täck flyttbara resurser och överlappande lån. [Mätprogram](https://github.com/Mik-pe/dunder-tui/blob/52313cc/benchmarks/language/swap_storage.ddr) · [Avvisat lån](https://github.com/Mik-pe/dunder-tui/blob/52313cc/tests/dogfood/compiler/swap_borrowed.ddr) · [Kompilatorns användning](../selfhost/mir/mir_lower.ddr#L518-L528)

## Följ upp flyttande arraytillägg

`std.array.append_move` används nu för att flytta textrader i ett publicerat bibliotek. Jämför allokeringsprofilen när färdig kapacitet räcker och när destinationen behöver växa. Behåll prov med element som bara kan flyttas. [Anropet i Text](https://github.com/Mik-pe/dunder-tui/blob/b9d76a2ab0224ad24a6abf2c5bebf33c9dbb3aa9/src/text.ddr) · [Arrayoperationen](../stdlib/array.ddr)

## Verifiera terminalhandtag på nativ Windows

`std.terminal.window_size(handle)` och `RawMode::enter_on(handle)` finns i 0.11.3. Det lokala Unixprovet kontrollerar bland annat ändrade pixelmått, en separat styrterminal och omdirigerade strömmar. Kör Win32-provet med två skärmbuffertar och kontrollera att rätt indatahandtag återställs vid tidig retur. [Unixprov](https://github.com/Mik-pe/dunder-tui/blob/b9d76a2ab0224ad24a6abf2c5bebf33c9dbb3aa9/tests/window_terminal.ddr) · [Win32-prov](../tests/fixtures/windows-terminal.ddr)

## Jämför standardbibliotekets terminalbredder med referensen

I 0.11.3 ger `std.terminal.columns` bredderna 2 och 3 för 😶‍🌫️ och 😶‍🌫, 1 för ett kombinerat `é` och 1 för det sammanhängande spannet `لا`. Kör breddfunktionen över samtliga 5 244 emoji och skriftspecifika ligaturer från referensprovet. Täck även kontrolltecken, tom text och spann som delar en ligatur. [Ratatui-prov](https://github.com/Mik-pe/dunder-tui/blob/b9d76a2ab0224ad24a6abf2c5bebf33c9dbb3aa9/benchmarks/reference/examples/text.rs) · [Breddmätning](../stdlib/terminal/width.ddr)

## Ange datumintervallet för lokal tid på Windows

`std.time.local_from_unix` i 0.11.3 gav samma kalenderdatum som det tidigare Unix-anropet i UTC, Europe/Stockholm och Pacific/Honolulu. Under Wine i UTC gav Windowsbygget `.None` för −1 och −315619200. Win32-anropet `SystemTimeToTzSpecificLocalTime` gav 1969-12-31 respektive 1960-01-01. Båda gav 1970-01-01 vid tidsstämpeln noll. Verifiera gränserna på nativ Windows x64 och dokumentera intervallet eller utöka stödet för datum före 1970. [Kör datumprovet med `--compare`](https://github.com/Mik-pe/dunder-tui/blob/b9d76a2ab0224ad24a6abf2c5bebf33c9dbb3aa9/tests/calendar_clock.ddr) · [Standardbiblioteket](../stdlib/time.ddr)

## Följ upp standardbibliotekets prestanda i 0.11.3

- `std.bytes.ByteQueue` klarar delad UTF-8 och återfyllning av en tom kö. Mät kostnaden för att läsa från en filbeskrivare via en mellanbuffert. TUI-parsern måste också kunna ta bort ett markörsvar mitt bland väntande tangenter. [Bytekö](../stdlib/bytes.ddr) · [Inmatningsvägen](https://github.com/Mik-pe/dunder-tui/blob/b9d76a2ab0224ad24a6abf2c5bebf33c9dbb3aa9/src/posix.ddr)
- `std.array.resize` använder nu `truncate` för krympning. Mät helbildstid och allokeringar vid växling mellan smal och bred buffert på fysisk x86-64. [Storleksprov](https://github.com/Mik-pe/dunder-tui/blob/b9d76a2ab0224ad24a6abf2c5bebf33c9dbb3aa9/benchmarks/buffer-resize.md) · [Arrayoperationen](../stdlib/array.ddr)
- På Apple M5 anger `MonotonicClock::resolution_ns` 1 000 ns för `Deadline` och 42 ns för `Raw`. Kontrollera klockornas beteende över systemvila och dokumentera hur det påverkar tidsgränser. [Klockorna](../stdlib/time.ddr) · [Mätprov](https://github.com/Mik-pe/dunder-tui/blob/b9d76a2ab0224ad24a6abf2c5bebf33c9dbb3aa9/benchmarks/language/monotonic_clock.ddr)

## Dela oföränderliga lokala uppslagstabeller

I Dunder 0.11.2 på macOS ARM64 gav 1 048 576 uppslag i en lokal literal med 32 heltal 4 194 305 allokeringsanrop, varav 4 194 300 återanvände poolminne. Samma tabell som modulkonstant gav ett anrop för hela processen. Båda varianterna gav summan 581 894 144. Undersök om en lokal fast array med konstanta element kan läsas direkt ur programbilden när den bara används för uppslag. Mät med `--runtime-profile` och prova att skrivning i en muterbar array fortfarande är lokal för varje anrop. [Körbart prov med lägena `local` och `shared`](https://github.com/Mik-pe/dunder-tui/blob/10628cbee55d47e28e8914c594f3a0fce7af85c1/benchmarks/language/constant_tables.ddr)

Tillåt även fasta strängtabeller som modulkonstanter. I 0.11.2 avvisas `const ROWS: [string; 4]` med E3007. Vid 65 536 uppslag på macOS ARM64 gav en lokal tabell 131 073 allokeringsanrop inklusive uppstart, medan `match` med samma fyra literaler gav ett. Båda summerade UTF-8-längderna till 131 072. Verifiera indexering vid körning med tom text, ASCII och tecken som tar flera UTF-8-byte. Uppslag ska kunna dela konstantdata utan allokering per läsning, som för de fasta teckentabellerna i [TUI-logotypen](https://github.com/Mik-pe/dunder-tui/blob/v0.22.0/src/artwork.ddr). [Minimalt kompilatorprov](https://github.com/Mik-pe/dunder-tui/blob/9808955/tests/dogfood/compiler/constant_string_rows.ddr) · [Mätprogram för `--runtime-profile`](https://github.com/Mik-pe/dunder-tui/blob/9808955/benchmarks/language/string_rows.ddr)

## Minska kostnaden för kortlivade Option-värden

I 0.11.3 gav 65 536 färguppslag på macOS och Linux ARM64 259 allokeringsanrop både för lånad färg och för färg som skickades direkt. Med omedelbart matchad `Some(color)` blev det 65 795. Alla gav summan 549 755 781 120. Eliminera mellanallokeringen när värdet stannar i anropet. Verifiera samma allokeringsantal för alla tre vägar samt korrekt ägarskap för ett sparat resultat och `None`. Mät därefter bildtiden i diagramvyn. [Körbart färgprov](https://github.com/Mik-pe/dunder-tui/blob/d1765d8/benchmarks/language/colors.ddr) · [Tillämpning i skuggritaren](https://github.com/Mik-pe/dunder-tui/blob/d1765d8/benchmarks/shadows.md)

## Återanvänd lyckade resultat vid vidarebefordran

I Dunder 0.11.2 på macOS ARM64 gav 65 536 anrop med `Result<(),string>` 65 543 allokeringsanrop inklusive uppstart. Ett mellanled som skickade vidare fel men skapade en ny `.Ok` gav 126 983. Skillnaden är exakt ett poolåterbruk för vart och ett av de 61 440 lyckade anropen. Båda körningarna gav samma 4 096 fel och sju färska allokeringar. Undersök om kompilatorn kan återanvända resultatet när ingen annan äger det. Kontrollera felmeddelandet, ett resultat som sparas av anroparen och en typ med resurs som nyttolast. [Körbart resultatprov](https://github.com/Mik-pe/dunder-tui/blob/a7cb0342938912b20b315b55338ffbe27fcf018e/benchmarks/language/unit_result.ddr)

## Undvik kopior vid läsning av koordinatpar

I 0.11.2 gav två lokala bindningar till element i en array av `(f64,f64)` 209 000 tupelkopior och 3 344 000 kopierade byte i ett diagramprov. Med uttryckliga lån till elementen blev båda räknarna noll. Kontrollsumman var 836 000 i båda fallen. Undersök om kompilatorn kan läsa fälten direkt när bindningen bara används lokalt och arrayen förblir oförändrad. Prova också att ett kopierat element behåller sitt värde om programmet sedan skriver till arrayen. [Körbart prov för båda varianterna](https://github.com/Mik-pe/dunder-tui/blob/9972020733fea428c7d9600f3ac7c9259b5266cf/benchmarks/language/borrowed_points.ddr)

## Håll lokala tupelfält i skalära variabler

I 0.11.2 gav `if skip { position(i) } else { (0,0) }` 65 536 tupelkonstruktioner när bara två heltalsfält lästes efteråt. Med två lokala heltal och uppackning i den tagna grenen blev antalet 4 096. Kontrollsumman var 67 096 575 i båda fallen. Mät om kompilatorn kan bära fälten genom förgreningen var för sig. Prova också en tupel som lämnar funktionen och en gren med sidoeffekt. I riktextprovet försvann 75 900 poolåterbruk på 1 100 bilder. [Minimalt prov](https://github.com/Mik-pe/dunder-tui/blob/v0.20.1/benchmarks/language/conditional_pair.ddr) · [Textvägen](https://github.com/Mik-pe/dunder-tui/blob/v0.20.1/src/text.ddr#L473-L487)

## Följ upp mätningar för SIMD

I ett prov med 96 celler och växlande bakgrund tog en bitmask över två lånade heltalsarrayer 3,18 ns per cell på nativ Linux x64 och 1,05 ns på macOS ARM64. En loop som återanvände senast beräknade stil tog 3,96 respektive 1,31 ns. I det symboliserade x64-bygget av 0.11.2 innehåller maskens loopkropp tio jämförelser mot stackgränsen `0x3fff000`. Pröva att slå samman de kontroller som täcker samma stackintervall. Verifiera befintliga stackfel, indexfel och cellerna utanför intervallet. [Radprov](https://github.com/Mik-pe/dunder-tui/blob/v0.20.1/benchmarks/language/fill_rows.ddr) · [Nativ mätning](https://github.com/Mik-pe/dunder-tui/actions/runs/45314935607/job/138179388595) · [x64-bakändens stackkontroll](../selfhost/backend/x86.ddr#L66-L73)

[Layoutmätningen](../benchmarks/layout/2026-11-22/README.md) innehåller nu
30 körningar per representation på fysisk x86-64 och ARM64. Samtliga gav
kontrollsumman 16 774 246. Behåll låneprovet för kolonntillväxt vid en
framtida representationsändring.

## Profilera bildritning på fysisk x86-64

Den låsta kalendervyn i v0.12.0 gav identiska celler i 1 100 bilder på
fysisk Linux x86-64 med Intel Core i5-1035G7. I minnet tog en oförändrad
bild 544 µs i Dunder 0.11.2 och 79 µs i Ratatui 0.30.2. I pseudoterminal
tog den 544 respektive 89 µs. Callgrind mätte 4 725 633 996 instruktioner
för hela scenen. Det dyraste anropet var `Frame::styled_rune` med 11,40 %
av instruktionerna. [Arbetslast och metod](https://github.com/Mik-pe/dunder-tui/blob/v0.12.0/benchmarks/calendar.md)

På en Xeon 6975P-C med Linux x86-64 under KVM gav en riktextruta med ligaturer samma celler i 1 100 bilder. En oförändrad bild tog 365 µs i median i Dunder och 57 µs i Ratatui i minnet, respektive 370 och 61 µs med pseudoterminal. I Callgrind gick 25,74 % av Dunderprocessens instruktioner till `Frame::fill_cells` och 21,11 % till `Frame::same_row`. Isolera fyllning och jämförelse av rader med parallella heltalsarrayer. Mät instruktioner per cell och jämför maskinkoden för båda arkitekturerna innan kodgeneratorn ändras. [Mätning](https://github.com/Mik-pe/dunder-tui/actions/runs/45301551701/job/138135086401) · [Instruktionsprofil](https://github.com/Mik-pe/dunder-tui/actions/runs/45304057607/job/138143422641)

På AMD EPYC 9V74 gick samma riktextvy från 231 till 183 µs per oförändrad bild i pseudoterminal när biblioteket begränsade strängjämförelserna till berörda kolumner och slutade leta efter breda tecken på smala teckenrader. Båda Dunderbyggena använde 0.11.2 och samma arbetslast på samma maskin. I den nya instruktionsprofilen står radjämförelsen för 1,63 % och radfyllningen för 19,81 %. Prioritera därför fyllningsloopen i det fristående kompilatorprovet. [Mätning och profil](https://github.com/Mik-pe/dunder-tui/actions/runs/45308458992/job/138157952246)
