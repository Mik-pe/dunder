# Nästa steg för Dunder

## Verifiera Windows som kompilatorvärd

Dunder distribuerar en korsbyggd x86-64-kompilator för Windows. Kör följande på en riktig Windowsinstallation och dokumentera Windowsversion, källrevision och resultat.

1. Hämta den publicerade exe-filen, kontrollera SHA-256 och bygg kompilatorn två gånger med ett jobb. Jämför filerna byte för byte. Upprepa med flera jobb.
2. Kör [provet för `check`, `build`, `run`, `test` och språkservern](../tools/test_lsp_and_dwarf.ddr) med kompilatorsökvägen som argument. Det använder ett lokalt paket i en sökväg med mellanslag och svenska tecken. Kör även [provet för miljö, standardfel och processavbrott](../tests/fixtures/process-unicode.ddr) samt [Win64-callbacken](../tests/fixtures/ffi/windows.ddr) med argumenten `"sök väg" "" "🦔"`.
3. Installera och uppgradera i en tom katalog. Prova även bytet medan den installerade filen används. [Installation och distribution](releases.md) · [Plattformskontrakt](targets.md)

Kör även `dunder run tests/fixtures/windows-terminal.ddr` i en konsol.
Provet jämför konsolläget före och efter tidig retur och läser tillbaka två barns utmatning från separata skärmbuffertar.

Privata filer på Windows behöver ett kontrakt för Windows åtkomstkontrollistor.
Verifiera åtkomsten som ett annat användarkonto och utred vilka garantier ett
beständigt katalogbyte kan ge när datorn startas om. [Filrättigheter och synkning](sys.md)

## Specialisera generiska metoder för varje mottagartyp

På 0.11.2 fungerar `Runner<i32>.outer<i32>` och `Runner<bool>.outer<i32>` var för sig. Om båda anropar den generiska metoden `inner` i samma program får det andra anropet E2003 oavsett ordning. Kör båda mottagartyperna i båda ordningarna och kontrollera varje returvärde. [Minimalt program](https://github.com/Mik-pe/dunder-tui/blob/v0.10.0/tests/dogfood/compiler/nested_methods.ddr)

## Packa upp ett lyckat Result utan returvärde

På 0.11.2 ger `operation().unwrap()` E1002 när `operation` returnerar `Result<(),string>`. Samma program går att köra med `.is_ok()` eller `match`. Prova att uppackning av `.Ok` fortsätter till nästa sats och att `.Err` följer samma felväg som för andra resultattyper. [Tvåradersprov](https://github.com/Mik-pe/dunder-tui/blob/v0.10.0/tests/dogfood/compiler/unwrap_unit.ddr)

## Styr en separat terminal via dess handtag

I 0.11.2 läser `std.terminal.size()` från standardutmatningen och `RawMode::enter()` ändrar standardinmatningen. En process som förbereder en annan terminal för ett barn behöver fortfarande egna termios-anrop. Ge båda operationerna ett alternativ som tar ett öppet handtag. Prova två pseudoterminaler med olika storlek, återställning vid tidig retur och oförändrade föräldraströmmar. På Windows ska storleken gå att läsa från vald skärmbuffert och konsolläget återställas på valt indatahandtag. [Terminalresursen](../stdlib/terminal.ddr) · [Konsolprovet](../tests/fixtures/windows-terminal.ddr)

## Minska kostnaden för kortlivade Option-värden

I ett färgprov på ARM64/macOS med 0.11.2 gav 65 536 direkta läsningar ur en förallokerad palett 259 allokeringsanrop inklusive uppstart. När samma färg skickades som `Some(color)` och matchades direkt blev det 65 795 anrop, varav 65 535 återanvände poolminne. Båda gav kontrollsumman 549 755 781 120. Undersök om kompilatorn kan undvika mellanobjektet när värdet stannar i anropet. Kontrollera båda varianterna av `Option`, samt bevarat ägarskap när resultatet sparas. Mät även bildtid i den ursprungliga diagramvyn. [Körbart färgprov](https://github.com/Mik-pe/dunder-tui/blob/v0.9.0/benchmarks/language/colors.ddr)

## Följ upp mätningar för SIMD

Arbetslasten i [layoutmätningen](../benchmarks/layout/2026-11-22/README.md) har ett oberoende numeriskt facit för både punktposter och separata kolumner. ARM64-serien kördes på fysisk maskin. Upprepa x86-64-serien på fysisk hårdvara inför ett beslut om vektorisering eller instruktionsschemaläggning. Behåll låneprovet för kolonntillväxt vid en framtida representationsändring.
