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

## Utvärdera plattformsvillkor före länkning

Med 0.11.2 på macOS ARM64 lägger `if target_is("windows") && enabled` till `kernel32.dll` i Mach-O-filens laddningskommandon. Bygget lyckas men programmet avslutas innan `main`. Två nästlade `if` ger en körbar fil utan DLL-importen. Prova båda formerna med `otool -L`, kör programmen och kontrollera att ett Windowsbygge fortfarande kan anropa `GetCurrentProcessId`. [Minimalt program](https://github.com/Mik-pe/dunder-tui/blob/0a9b6dfc4c29ed1fd2d40f8289fc897c54d9e034/tests/dogfood/compiler/target_condition.ddr)

## Visa källkodens typnamn i fel för konstanter

En färgskala med två `u32`-fält ger i 0.11.2 felet E3007 vid `const INK: Scale`. Både `check` och `build` visar `__dunder_module_0` som typnamn. Bevara namnet `Scale` i diagnostiken och ange vilka konstanttyper som stöds. Prova även en importerad posttyp, och kontrollera att samma värden i en fast array fortfarande går att bygga och läsa. [Posten som utlöser felet](https://github.com/Mik-pe/dunder-tui/blob/9e5e89a4e3a6c48d6c3ab509243dd8234d900f34/tests/dogfood/compiler/record_constant.ddr) · [Fungerande array](https://github.com/Mik-pe/dunder-tui/blob/9e5e89a4e3a6c48d6c3ab509243dd8234d900f34/tests/dogfood/compiler/array_constant.ddr) · [Konstanternas typregler](syntax.md#konstanter-på-modulnivå)

## Härled mottagartypen över en modulgräns

I 0.11.2 avslutas `dunder check` med kod 103 utan diagnostik när `holder := factory.make()?` följs av `holder.get(7)`. Fabriken returnerar `Result<Holder<i32>,string>` och metoden är generisk. Med typen `factory.Holder<i32>` utskriven går programmet att bygga och köra. Kör båda varianterna och kontrollera returvärdet 7. [Tvåmodulsprov](https://github.com/Mik-pe/dunder-tui/tree/adc3814204ec460685054aa39b439efcc22921ba/tests/dogfood/compiler/inferred_receiver)

## Specialisera generiska metoder för varje mottagartyp

På 0.11.2 fungerar `Runner<i32>.outer<i32>` och `Runner<bool>.outer<i32>` var för sig. Om båda anropar den generiska metoden `inner` i samma program får det andra anropet E2003 oavsett ordning. Kör båda mottagartyperna i båda ordningarna och kontrollera varje returvärde. [Minimalt program](https://github.com/Mik-pe/dunder-tui/blob/v0.10.0/tests/dogfood/compiler/nested_methods.ddr)

## Låt en lokal vy läsa från två ägare

En iterator över ändrade celler behöver läsa både föregående och aktuell bild. I 0.11.2 går det att lagra två delade lån från samma array i en post. Två lokala ägare ger E4003 även när de båda lever längre än posten. Utvärdera om en lokal vy kan knytas till båda ägarna. Prova tidigt avslutad iteration samt att fortsatt användning av vyn håller båda ägarna lånade och att båda blir skrivbara efter sista läsningen. [Två ägare](https://github.com/Mik-pe/dunder-tui/blob/v0.13.0/tests/dogfood/compiler/paired_views.ddr) · [Gemensam ägare](https://github.com/Mik-pe/dunder-tui/blob/v0.13.0/tests/dogfood/compiler/paired_views_one_owner.ddr)

## Packa upp ett lyckat Result utan returvärde

På 0.11.2 ger `operation().unwrap()` E1002 när `operation` returnerar `Result<(),string>`. Samma program går att köra med `.is_ok()` eller `match`. Prova att uppackning av `.Ok` fortsätter till nästa sats och att `.Err` följer samma felväg som för andra resultattyper. [Tvåradersprov](https://github.com/Mik-pe/dunder-tui/blob/v0.10.0/tests/dogfood/compiler/unwrap_unit.ddr)

## Styr en separat terminal via dess handtag

I 0.11.2 läser `std.terminal.size()` från standardutmatningen och `RawMode::enter()` ändrar standardinmatningen. En process som förbereder en annan terminal för ett barn behöver fortfarande egna termios-anrop. Ge båda operationerna ett alternativ som tar ett öppet handtag. Prova två pseudoterminaler med olika storlek, återställning vid tidig retur och oförändrade föräldraströmmar. På Windows ska storleken gå att läsa från vald skärmbuffert och konsolläget återställas på valt indatahandtag. [Terminalresursen](../stdlib/terminal.ddr) · [Konsolprovet](../tests/fixtures/windows-terminal.ddr)

## Dela UTF-16-kodningen mellan Windowsanrop

I 0.11.2 bildar Dunders sökvägskod och TUI:ts konsolskrivare samma surrogatpar för tecken över U+FFFF. Lägg den gemensamma kodningen i standardbiblioteket och låt anroparen återanvända en `[]u16`. En konsolsträng ska kunna innehålla U+0000. Sökvägsfunktionen ska fortsätta avvisa inre nollor och lägga till en avslutande nolla. Prova tom text, `Ö🌍` och `a\0b` för båda anropssätten. [Sökväg och befintliga prov](../stdlib/sys/winpath.ddr) · [Konsolskrivaren](https://github.com/Mik-pe/dunder-tui/blob/v0.10.0/src/windows.ddr#L162-L186)

## Hämta lokal kalendertid för ett givet ögonblick

I 0.11.2 kan `std.time` omvandla en Unix-tidsstämpel till UTC. En kalender som markerar dagens datum behöver även operativsystemets lokala tid. Med tidsstämpeln `1711924200` gav provet 31 mars 2024 i UTC och 1 april i Europe/Stockholm på macOS och Linux. Exponera den lokala omvandlingen i `std.time` och ange hur fel och datum utanför systemets intervall rapporteras. Prova även `1704067200` i Pacific/Honolulu (31 december 2023), båda sidor av en sommartidsövergång samt en Windowsinstallation med vald lokal tidszon. [Dagens tidsfunktioner](../stdlib/time.ddr) · [Körbart datumprov](https://github.com/Mik-pe/dunder-tui/blob/v0.12.0/tests/calendar_clock.ddr)

## Dela oföränderliga lokala uppslagstabeller

I Dunder 0.11.2 på macOS ARM64 gav 1 048 576 uppslag i en lokal literal med 32 heltal 4 194 305 allokeringsanrop, varav 4 194 300 återanvände poolminne. Samma tabell som modulkonstant gav ett anrop för hela processen. Båda varianterna gav summan 581 894 144. Undersök om en lokal fast array med konstanta element kan läsas direkt ur programbilden när den bara används för uppslag. Mät med `--runtime-profile` och prova att skrivning i en muterbar array fortfarande är lokal för varje anrop. [Körbart prov med lägena `local` och `shared`](https://github.com/Mik-pe/dunder-tui/blob/10628cbee55d47e28e8914c594f3a0fce7af85c1/benchmarks/language/constant_tables.ddr)

## Minska kostnaden för kortlivade Option-värden

I ett färgprov på ARM64/macOS med 0.11.2 gav 65 536 direkta läsningar ur en förallokerad palett 259 allokeringsanrop inklusive uppstart. När samma färg skickades som `Some(color)` och matchades direkt blev det 65 795 anrop, varav 65 535 återanvände poolminne. Båda gav kontrollsumman 549 755 781 120. Undersök om kompilatorn kan undvika mellanobjektet när värdet stannar i anropet. Kontrollera båda varianterna av `Option`, samt bevarat ägarskap när resultatet sparas. Mät även bildtid i den ursprungliga diagramvyn. [Körbart färgprov](https://github.com/Mik-pe/dunder-tui/blob/v0.9.0/benchmarks/language/colors.ddr)

## Följ upp mätningar för SIMD

Arbetslasten i [layoutmätningen](../benchmarks/layout/2026-11-22/README.md) har ett oberoende numeriskt facit för både punktposter och separata kolumner. ARM64-serien kördes på fysisk maskin. Upprepa x86-64-serien på fysisk hårdvara inför ett beslut om vektorisering eller instruktionsschemaläggning. Behåll låneprovet för kolonntillväxt vid en framtida representationsändring.

## Profilera bildritning på fysisk x86-64

Sex kalendrar gav samma celler i 1 100 bilder i Dunder 0.11.2 och Ratatui. I en pseudoterminal tog en oförändrad bild i median 27 respektive 41 µs på Linux ARM64. På samma värd med x86-64-översättning tog den 265 respektive 68 µs. Kör de låsta programmen på fysisk Linux x86-64 och jämför både ritning i minnet och utmatning till terminal. Om skillnaden kvarstår, profilera den långsamma bildvägen och isolera det dyraste anropet innan kodgeneratorn eller biblioteket ändras. [Körkommandon och arbetslast](https://github.com/Mik-pe/dunder-tui/blob/v0.12.0/benchmarks/calendar.md) · [Mätvärden och spridning](https://github.com/Mik-pe/dunder-tui/blob/v0.12.0/benchmarks/results/calendar-0.12.0.tsv)
