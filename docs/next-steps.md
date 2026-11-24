# Nästa steg för Dunder

## Verifiera Windows som kompilatorvärd

Dunder distribuerar en korsbyggd x86-64-kompilator för Windows. Kör följande på en riktig Windowsinstallation och dokumentera Windowsversion, källrevision och resultat.

Tre fel i 0.11.2 har reproducerats på Windows Server 2025 x64.

- **Bygg paket utan Git.** `dunder test --locked --offline` ger `Git dependencies require a native compiler host` även med noll beroenden. Utanför manifestet bygger samma kompilator testfilen och alla 59 test passerar. Beroendelösaren öppnar Git-lagret innan den läser paketets beroenden. Prova ett tomt beroendeträd, ett lokalt sökvägsberoende och ett låst Git-beroende från cache på Windows. [Nativ körning](https://github.com/Mik-pe/dunder-tui/actions/runs/45302071651/job/138136790534) · [Beroendelösare](../selfhost/driver/dependencies.ddr)
- **Läs process-ID med rätt bredd.** Efter `CreateProcessW` läser `std.process` process-ID och tråd-ID som ett gemensamt 64-bitarsvärde. Konverteringen till `i32` avslutar föräldern med kod 101. När bara process-ID:ts DWORD lästes fungerade både `exec` och `spawn` följt av `wait` på samma Windowsvärd. Ändra fältläsningen i standardbiblioteket och behåll ett nativt prov för barnets utmatning och slutstatus. [Minimalt program](https://github.com/Mik-pe/dunder-tui/blob/748c099/tests/dogfood/compiler/windows_child.ddr) · [Kontrollerad jämförelse](https://github.com/Mik-pe/dunder-tui/actions/runs/45304861929/job/138146099450) · [Win32-anropet](../stdlib/sys/winprocess.ddr)
- **Skriv slutrapporten för `--runtime-profile`.** Windowsbygget avslutar riktextprovet med kod 0 men saknar JSON-posten på stderr. Även [hello.ddr](../examples/hello.ddr) lämnar stderr tom i Wine. Rapportören försöker blockera SIGPIPE med ett POSIX-systemanrop. Windowsbakänden ger −38 och rapportören hoppar då över utskriften. Kör hello med öppet, omdirigerat och stängt stderr på Windows. Kontrollera en giltig rapport vid öppen ström samt bevarad stdout och slutstatus i alla tre fallen. [Rapportör](../selfhost/runtime/runtime.ddr) · [x86-bakände](../selfhost/backend/x86.ddr) · [Nativ mätning](https://github.com/Mik-pe/dunder-tui/actions/runs/45303881483/job/138142828516)

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

## Förklara lånet när en metod matchar på ett eget fält

I 0.11.2 ger `match self.mode` följt av `self.grow()` i en gren E4003 om mutation under iteration. Programmet har varken en samling eller en loop. Visa vilket fält matchningen lånar och var det muterande anropet sker. Utvärdera om lånet kan avslutas före anropet i en gren som inte binder några variantdata. Prova även en gren som fortfarande använder ett lånat fält efter anropet. [Minimalt program](https://github.com/Mik-pe/dunder-tui/blob/2758201f70c69e44e7453f73f15dc41df1c6a968/tests/dogfood/compiler/match_receiver.ddr) · [Fungerande lokal kopia](https://github.com/Mik-pe/dunder-tui/blob/2758201f70c69e44e7453f73f15dc41df1c6a968/tests/dogfood/compiler/match_receiver_copy.ddr)

Prova också en lokal enum med två varianter utan nyttolast. I 0.11.2 ger `match state` följt av `state = .Done` i grenen samma E4003. En kopia av diskriminanten före matchningen räcker för att bygga programmet. Pröva om den direkta tilldelningen kan tillåtas när grenen saknar lånade variantdata. [Minimalt program](https://github.com/Mik-pe/dunder-tui/blob/8c41a4e821a21bae7d966bc0f222c65e4383ad99/tests/dogfood/compiler/match_local.ddr)

## Låna två skilda fält i samma metodanrop

I 0.11.2 avvisar både `check` och `build` anropet `self.cursor.advance(self.source)` med E4003. Metoden ändrar ett heltal i `cursor` och läser längden på den separat ägda sträng som `source` lånar. En lokal kopia av markören kan utföra anropet och sedan flyttas tillbaka. Låt lånekontrollen skilja på dessa åtkomstvägar. Prova även två lån av samma fält och ett lån av hela mottagaren. [Minimalt program](https://github.com/Mik-pe/dunder-tui/blob/f9c8089da7b0cac67db941e1b36186edc733b4bb/tests/dogfood/compiler/disjoint_fields.ddr)

## Låt kortlivade iteratorposter stanna i loopen

När `Iterator.next` i ett prov med 1 024 element returnerade en post med ett lån, ett index och en inre post rapporterade 0.11.2 totalt 5 312 postkopior och 7 489 allokeringsanrop. Direkt läsning av samma byte och vikt gav noll postkopior och ett allokeringsanrop. Båda gav summan 110 080 på macOS och Linux ARM64. Pröva att eliminera kopior genom `Option` och loopbindningen när elementet förbrukas på plats. Kontrollera även ägda element som sparas och att livstidskontrollen fortfarande avvisar lån som lämnar sin ägare. [Mätprogram](https://github.com/Mik-pe/dunder-tui/blob/f3ecfc12e64df502da617593e7197483582b92fe/benchmarks/language/iterator_item.ddr)

För `std.text.graphemes` ger ett andra `next()` E4003 i 0.11.2 om det första elementets text läses efter anropet. Att läsa det första elementet direkt fungerar. Pröva en returanknytning till iteratorns lånade källtext som låter markören gå vidare medan båda elementen lever. Källan ska förbli lånad till sista läsningen. [Två grafem från samma sträng](https://github.com/Mik-pe/dunder-tui/blob/a94c584/tests/dogfood/compiler/retained_graphemes.ddr)

## Visa hur en loop kan ändra elementen i en lånad array

För `for cue in &mut cues { cue.played = true }` föreslår 0.11.2 att byta `&mut` mot `&` (E4003). Då avvisas tilldelningen med E4002. Föreslå den fungerande indexloopen i diagnostiken. I macOS-provet uppdaterade den båda posterna med noll array- och postkloner. Pröva även direkt iteration över muterbara element med ett lån per varv. En växande array måste behålla sin lagring medan elementet är lånat. [Reproduktionsfall](https://github.com/Mik-pe/dunder-tui/blob/421eb9f/tests/dogfood/compiler/mutable_elements.ddr) · [Fungerande indexloop](https://github.com/Mik-pe/dunder-tui/blob/421eb9f/tests/dogfood/compiler/indexed_mutable_elements.ddr)

## Låt en lokal vy läsa från två ägare

En iterator över ändrade celler behöver läsa både föregående och aktuell bild. I 0.11.2 går det att lagra två delade lån från samma array i en post. Två lokala ägare ger E4003 även när de båda lever längre än posten. Utvärdera om en lokal vy kan knytas till båda ägarna. Prova tidigt avslutad iteration samt att fortsatt användning av vyn håller båda ägarna lånade och att båda blir skrivbara efter sista läsningen. [Två ägare](https://github.com/Mik-pe/dunder-tui/blob/v0.13.0/tests/dogfood/compiler/paired_views.ddr) · [Gemensam ägare](https://github.com/Mik-pe/dunder-tui/blob/v0.13.0/tests/dogfood/compiler/paired_views_one_owner.ddr)

## Returnera en valfri delvy av en lånad sträng

I 0.11.2 ger både `check` och `build` E3099 för `.Some(&value[1..value.len])` i en funktion med returtypen `Option<&str> from value`. En post som lagrar källvyn och två byteindex fungerar. Låt en valfri delvy bära sitt intervall och behålla lånet från anroparen. Prova en tom träff, en utebliven träff och UTF-8 efter ett ASCII-prefix. En vy in i en lokal ägd sträng ska fortfarande avvisas. [Minimalt kompilatorprov](https://github.com/Mik-pe/dunder-tui/blob/f3890e7fa9b24e6a9e8dbc3c5d3e3a0011b85b9d/tests/dogfood/compiler/optional_text_view.ddr) · [Körbar variant med byteindex](https://github.com/Mik-pe/dunder-tui/blob/f3890e7fa9b24e6a9e8dbc3c5d3e3a0011b85b9d/benchmarks/language/line_views.ddr)

## Läs rader ur lånad UTF-8-text

Lägg till stegvis radläsning i `std.text`. På 3 073 rader gav `split` i 0.11.2 på macOS ARM64 3 073 strängsnitt och 8 194 kopierade byte även när anroparen stannade efter första raden. Vid läsning av hela texten gav `split`, en lånad iterator med radposter och en markör med byteindex samma kontrollsumma, 10 243. De gjorde 3 103, 12 314 respektive 19 allokeringsanrop inklusive uppstart och testdata. Kontrollera både tidigt avbrott och full genomläsning, med LF, CRLF, tomrader och ett ensamt CR i sista raden. [Mätprogram med tre läsare](https://github.com/Mik-pe/dunder-tui/blob/f3890e7fa9b24e6a9e8dbc3c5d3e3a0011b85b9d/benchmarks/language/line_views.ddr) · [Anrop som bara använder första raden](../stdlib/process.ddr#L496-L503)

## Jämför två lånade arrayintervall direkt

Med `(&values[0..2]) == (&values[2..4])` ger både `check` och `build` E4002 i 0.11.2 och föreslår ett `&` som redan står där. Om vyerna binds till två lokala namn först fungerar samma jämförelse. Profilen visar noll arraykloner och noll kopierade byte. Tillåt jämförelsen direkt eller visa den fungerande bindningen i diagnostiken. Prova lika, olika och tomma intervall. [Direkt jämförelse](https://github.com/Mik-pe/dunder-tui/blob/7b6c29a/tests/dogfood/compiler/inline_view_equality.ddr) · [Namngivna vyer](https://github.com/Mik-pe/dunder-tui/blob/7b6c29a/tests/dogfood/compiler/named_view_equality.ddr)

## Byt två ägda värden utan ett mellanvärde

Två lokala ägare kan byta plats med tre flyttar. Samma kod bakom två `&mut`-parametrar ger E4003. Körlistan och kompilatorns `exchange_data_into` använder därför `replace` med ett mellanvärde. I 0.11.2 gav 4 095 byten av poster med två arrayer 12 304 allokeringsanrop, mot 19 för lokala flyttar och 21 för arrayens `swap`. Av de 12 285 extra anropen var ett en färsk allokering. macOS och Linux ARM64 gav samma slutordning och profiler. Pröva platsbyte bakom två muterbara lån utan `Default` eller allokering. Täck flyttbara resurser och överlappande lån. [Mätprogram](https://github.com/Mik-pe/dunder-tui/blob/52313cc/benchmarks/language/swap_storage.ddr) · [Avvisat lån](https://github.com/Mik-pe/dunder-tui/blob/52313cc/tests/dogfood/compiler/swap_borrowed.ddr) · [Kompilatorns användning](../selfhost/mir/mir_lower.ddr#L518-L528)

## Lägg till ägda element i en array

Komplettera `std.array.append` med en variant som tar över en ägd källarray och behåller elementens ordning. Dunder 0.11.2 klarar detta med `T: Storable`, en reservation och `for item in move source`. I ett prov med 1 024 rader om 24 heltal gjorde den lånade varianten 1 024 arraykopior om totalt 98 304 byte. Den flyttande varianten gjorde noll. Alla 24 576 värden verifierades på macOS och Linux ARM64. Täck även tom källa, tom destination och element som bara kan flyttas. [Körbart prov med båda varianterna](https://github.com/Mik-pe/dunder-tui/blob/70f877d54df0b9c2d3bc5cf2dd8f38aeef838c67/benchmarks/language/append_arrays.ddr)

## Packa upp ett lyckat Result utan returvärde

På 0.11.2 ger `operation().unwrap()` E1002 när `operation` returnerar `Result<(),string>`. Samma program går att köra med `.is_ok()` eller `match`. Prova att uppackning av `.Ok` fortsätter till nästa sats och att `.Err` följer samma felväg som för andra resultattyper. [Tvåradersprov](https://github.com/Mik-pe/dunder-tui/blob/v0.10.0/tests/dogfood/compiler/unwrap_unit.ddr)

## Styr en separat terminal via dess handtag

I 0.11.2 läser `std.terminal.size()` från standardutmatningen och `RawMode::enter()` ändrar standardinmatningen. En process som förbereder en annan terminal för ett barn behöver fortfarande egna termios-anrop. Ge båda operationerna ett alternativ som tar ett öppet handtag. Prova två pseudoterminaler med olika storlek, återställning vid tidig retur och oförändrade föräldraströmmar. På Windows ska storleken gå att läsa från vald skärmbuffert och konsolläget återställas på valt indatahandtag. [Terminalresursen](../stdlib/terminal.ddr) · [Konsolprovet](../tests/fixtures/windows-terminal.ddr)

Låt storleksfrågan på Unix också exponera pixelbredd och pixelhöjd från samma systemanrop. I ett körbart prov ändrades pixelmåtten från 0 × 0 till 813 × 719 medan cellmåtten låg kvar på 45 × 8. Prova även en ensam känd axel, 65 535 och skillnaden mellan en terminal utan pixeluppgift och ett handtag som är en pipe. [Prov på macOS och Linux](https://github.com/Mik-pe/dunder-tui/blob/1c92bea6f93c819e98b64ce3f807f862596d4865/tests/window_terminal.ddr)

## Mät grafem och sammanhängande text i terminalkolumner

I 0.11.2 ger `std.text.graphemes` ett grafem för både 😶‍🌫️ och 😶‍🌫. I dunder-tui och Ratatui tar de två respektive tre kolumner. Exponera en breddfunktion i `std.terminal` som kan läsa grafemets lånade UTF-8-vy. Ange Unicode-version, presentationsregler och hur kontrolltecken hanteras. Verifiera bredden mot de 5 244 fallen i Unicode 18:s emojitest och de 742 registrerade presentationsvarianterna. Låt TUI-biblioteket prova funktionen vid klippning och när markören flyttas efter en emoji. [Prov med Dunders grafemiterator](https://github.com/Mik-pe/dunder-tui/blob/v0.18.1/tests/stdlib_text.ddr) · [Referensprov med terminalceller](https://github.com/Mik-pe/dunder-tui/blob/v0.18.1/tests/text_reference.ddr)

Låt även en lånad sträng mätas som helhet. För `لا` ger grafemiteratorn två grafem som tar en cell var när de ritas separat. Ratatui mäter samma följd till en kolumn i ett sammanhängande spann och två när en spanngräns delar bokstäverna. Prova båda bredderna och markörens position när nästa spann ritas. [Körbart prov med Dunder 0.11.2](https://github.com/Mik-pe/dunder-tui/blob/8c41a4e821a21bae7d966bc0f222c65e4383ad99/tests/stdlib_text.ddr) · [Prov i pseudoterminal](https://github.com/Mik-pe/dunder-tui/blob/8c41a4e821a21bae7d966bc0f222c65e4383ad99/tests/posix_terminal.ddr)

## Dela UTF-16-kodningen mellan Windowsanrop

I 0.11.2 bildar Dunders sökvägskod och TUI:ts konsolskrivare samma surrogatpar för tecken över U+FFFF. Lägg den gemensamma kodningen i standardbiblioteket och låt anroparen återanvända en `[]u16`. En konsolsträng ska kunna innehålla U+0000. Sökvägsfunktionen ska fortsätta avvisa inre nollor och lägga till en avslutande nolla. Prova tom text, `Ö🌍` och `a\0b` för båda anropssätten. [Sökväg och befintliga prov](../stdlib/sys/winpath.ddr) · [Konsolskrivaren](https://github.com/Mik-pe/dunder-tui/blob/v0.10.0/src/windows.ddr#L162-L186)

## Hämta lokal kalendertid för ett givet ögonblick

I 0.11.2 kan `std.time` omvandla en Unix-tidsstämpel till UTC. En kalender som markerar dagens datum behöver även operativsystemets lokala tid. Med tidsstämpeln `1711924200` gav provet 31 mars 2024 i UTC och 1 april i Europe/Stockholm på macOS och Linux. Exponera den lokala omvandlingen i `std.time` och ange hur fel och datum utanför systemets intervall rapporteras. Prova även `1704067200` i Pacific/Honolulu (31 december 2023), båda sidor av en sommartidsövergång samt en Windowsinstallation med vald lokal tidszon. [Dagens tidsfunktioner](../stdlib/time.ddr) · [Körbart datumprov](https://github.com/Mik-pe/dunder-tui/blob/v0.12.0/tests/calendar_clock.ddr)

## Läs stegvis ur en bytekö

Med 0.11.2 på macOS ARM64 tog det 6,8, 26 och 105 ms att tömma en kö på 4, 8 respektive 16 KiB med `std.bytes.consume_prefix(&mut data,1)`. Varje anrop flyttar alla efterföljande byte. Med en läsposition och en lånad vy tog samma arbete 13, 26 och 53 µs. Siffrorna är medianer för sju körningar med växlande ordning och samma kontrollsummor. Ge strömmande läsare ett exempel eller ett återanvändbart API för att läsa en vy, avancera och återanvända bufferten. Prova även en ofullständig UTF-8-sekvens följd av ny indata och att en tom kö kan fyllas igen. [Körbart köprov](https://github.com/Mik-pe/dunder-tui/blob/9605bd1be79057fdb5f11dc7470cd2a8ab1da26b/benchmarks/language/byte_queue.ddr) · [Befintlig byteoperation](../stdlib/bytes.ddr)

## Dela oföränderliga lokala uppslagstabeller

I Dunder 0.11.2 på macOS ARM64 gav 1 048 576 uppslag i en lokal literal med 32 heltal 4 194 305 allokeringsanrop, varav 4 194 300 återanvände poolminne. Samma tabell som modulkonstant gav ett anrop för hela processen. Båda varianterna gav summan 581 894 144. Undersök om en lokal fast array med konstanta element kan läsas direkt ur programbilden när den bara används för uppslag. Mät med `--runtime-profile` och prova att skrivning i en muterbar array fortfarande är lokal för varje anrop. [Körbart prov med lägena `local` och `shared`](https://github.com/Mik-pe/dunder-tui/blob/10628cbee55d47e28e8914c594f3a0fce7af85c1/benchmarks/language/constant_tables.ddr)

Tillåt även fasta strängtabeller som modulkonstanter. I 0.11.2 avvisas `const ROWS: [string; 4]` med E3007. Vid 65 536 uppslag på macOS ARM64 gav en lokal tabell 131 073 allokeringsanrop inklusive uppstart, medan `match` med samma fyra literaler gav ett. Båda summerade UTF-8-längderna till 131 072. Verifiera indexering vid körning med tom text, ASCII och tecken som tar flera UTF-8-byte. Uppslag ska kunna dela konstantdata utan allokering per läsning, som för de fasta teckentabellerna i [TUI-logotypen](https://github.com/Mik-pe/dunder-tui/blob/v0.22.0/src/artwork.ddr). [Minimalt kompilatorprov](https://github.com/Mik-pe/dunder-tui/blob/9808955/tests/dogfood/compiler/constant_string_rows.ddr) · [Mätprogram för `--runtime-profile`](https://github.com/Mik-pe/dunder-tui/blob/9808955/benchmarks/language/string_rows.ddr)

## Minska kostnaden för kortlivade Option-värden

I ett färgprov på ARM64/macOS med 0.11.2 gav 65 536 direkta läsningar ur en förallokerad palett 259 allokeringsanrop inklusive uppstart. När samma färg skickades som `Some(color)` och matchades direkt blev det 65 795 anrop, varav 65 535 återanvände poolminne. Båda gav kontrollsumman 549 755 781 120. Undersök om kompilatorn kan undvika mellanobjektet när värdet stannar i anropet. Kontrollera båda varianterna av `Option`, samt bevarat ägarskap när resultatet sparas. Mät även bildtid i den ursprungliga diagramvyn. [Körbart färgprov](https://github.com/Mik-pe/dunder-tui/blob/v0.9.0/benchmarks/language/colors.ddr)

## Återanvänd lyckade resultat vid vidarebefordran

I Dunder 0.11.2 på macOS ARM64 gav 65 536 anrop med `Result<(),string>` 65 543 allokeringsanrop inklusive uppstart. Ett mellanled som skickade vidare fel men skapade en ny `.Ok` gav 126 983. Skillnaden är exakt ett poolåterbruk för vart och ett av de 61 440 lyckade anropen. Båda körningarna gav samma 4 096 fel och sju färska allokeringar. Undersök om kompilatorn kan återanvända resultatet när ingen annan äger det. Kontrollera felmeddelandet, ett resultat som sparas av anroparen och en typ med resurs som nyttolast. [Körbart resultatprov](https://github.com/Mik-pe/dunder-tui/blob/a7cb0342938912b20b315b55338ffbe27fcf018e/benchmarks/language/unit_result.ddr)

## Undvik kopior vid läsning av koordinatpar

I 0.11.2 gav två lokala bindningar till element i en array av `(f64,f64)` 209 000 tupelkopior och 3 344 000 kopierade byte i ett diagramprov. Med uttryckliga lån till elementen blev båda räknarna noll. Kontrollsumman var 836 000 i båda fallen. Undersök om kompilatorn kan läsa fälten direkt när bindningen bara används lokalt och arrayen förblir oförändrad. Prova också att ett kopierat element behåller sitt värde om programmet sedan skriver till arrayen. [Körbart prov för båda varianterna](https://github.com/Mik-pe/dunder-tui/blob/9972020733fea428c7d9600f3ac7c9259b5266cf/benchmarks/language/borrowed_points.ddr)

## Håll lokala tupelfält i skalära variabler

I 0.11.2 gav `if skip { position(i) } else { (0,0) }` 65 536 tupelkonstruktioner när bara två heltalsfält lästes efteråt. Med två lokala heltal och uppackning i den tagna grenen blev antalet 4 096. Kontrollsumman var 67 096 575 i båda fallen. Mät om kompilatorn kan bära fälten genom förgreningen var för sig. Prova också en tupel som lämnar funktionen och en gren med sidoeffekt. I riktextprovet försvann 75 900 poolåterbruk på 1 100 bilder. [Minimalt prov](https://github.com/Mik-pe/dunder-tui/blob/v0.20.1/benchmarks/language/conditional_pair.ddr) · [Textvägen](https://github.com/Mik-pe/dunder-tui/blob/v0.20.1/src/text.ddr#L473-L487)

## Följ upp mätningar för SIMD

I ett prov med 96 celler och växlande bakgrund tog en bitmask över två lånade heltalsarrayer 3,18 ns per cell på nativ Linux x64 och 1,05 ns på macOS ARM64. En loop som återanvände senast beräknade stil tog 3,96 respektive 1,31 ns. I det symboliserade x64-bygget av 0.11.2 innehåller maskens loopkropp tio jämförelser mot stackgränsen `0x3fff000`. Pröva att slå samman de kontroller som täcker samma stackintervall. Verifiera befintliga stackfel, indexfel och cellerna utanför intervallet. [Radprov](https://github.com/Mik-pe/dunder-tui/blob/v0.20.1/benchmarks/language/fill_rows.ddr) · [Nativ mätning](https://github.com/Mik-pe/dunder-tui/actions/runs/45314935607/job/138179388595) · [x64-bakändens stackkontroll](../selfhost/backend/x86.ddr#L66-L73)

Arbetslasten i [layoutmätningen](../benchmarks/layout/2026-11-22/README.md) har ett oberoende numeriskt facit för både punktposter och separata kolumner. ARM64-serien kördes på fysisk maskin. Upprepa x86-64-serien på fysisk hårdvara inför ett beslut om vektorisering eller instruktionsschemaläggning. Behåll låneprovet för kolonntillväxt vid en framtida representationsändring.

## Profilera bildritning på fysisk x86-64

Sex kalendrar gav samma celler i 1 100 bilder i Dunder 0.11.2 och Ratatui. I en pseudoterminal tog en oförändrad bild i median 27 respektive 41 µs på Linux ARM64. På samma värd med x86-64-översättning tog den 265 respektive 68 µs. Kör de låsta programmen på fysisk Linux x86-64 och jämför både ritning i minnet och utmatning till terminal. Om skillnaden kvarstår, profilera den långsamma bildvägen och isolera det dyraste anropet innan kodgeneratorn eller biblioteket ändras. [Körkommandon och arbetslast](https://github.com/Mik-pe/dunder-tui/blob/v0.12.0/benchmarks/calendar.md) · [Mätvärden och spridning](https://github.com/Mik-pe/dunder-tui/blob/v0.12.0/benchmarks/results/calendar-0.12.0.tsv)

På en Xeon 6975P-C med Linux x86-64 under KVM gav en riktextruta med ligaturer samma celler i 1 100 bilder. En oförändrad bild tog 365 µs i median i Dunder och 57 µs i Ratatui i minnet, respektive 370 och 61 µs med pseudoterminal. I Callgrind gick 25,74 % av Dunderprocessens instruktioner till `Frame::fill_cells` och 21,11 % till `Frame::same_row`. Isolera fyllning och jämförelse av rader med parallella heltalsarrayer. Mät instruktioner per cell och jämför maskinkoden för båda arkitekturerna innan kodgeneratorn ändras. [Mätning](https://github.com/Mik-pe/dunder-tui/actions/runs/45301551701/job/138135086401) · [Instruktionsprofil](https://github.com/Mik-pe/dunder-tui/actions/runs/45304057607/job/138143422641)

På AMD EPYC 9V74 gick samma riktextvy från 231 till 183 µs per oförändrad bild i pseudoterminal när biblioteket begränsade strängjämförelserna till berörda kolumner och slutade leta efter breda tecken på smala teckenrader. Båda Dunderbyggena använde 0.11.2 och samma arbetslast på samma maskin. I den nya instruktionsprofilen står radjämförelsen för 1,63 % och radfyllningen för 19,81 %. Prioritera därför fyllningsloopen i det fristående kompilatorprovet. [Mätning och profil](https://github.com/Mik-pe/dunder-tui/actions/runs/45308458992/job/138157952246)
