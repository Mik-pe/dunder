# Standardbiblioteket

Biblioteket är skrivet i Dunder. [stdlib/modules.txt](../stdlib/modules.txt)
anger vilka moduler som följer med kompilatorn. Importer som
`import std.map as map` laddar inbäddad källkod utan installerade `.ddr`-filer
eller suffix i importen. Endast explicita beroenden läses. Delade importer
och alias har samma typidentitet. En lokal fil väljs med en uttrycklig
relativ sökväg, till exempel `import .std.map as local`.

## Välj API-nivå

| När du behöver | Börja med |
| --- | --- |
| Äga värden och uttrycka frånvaro eller fel | Språkets arrayer, strängar och lån samt preludiets `Option<T>`, `Result<T, E>` och traits. De finns utan import. |
| Bearbeta data i minnet | `std.text`, `std.bytes`, `std.math` och samlingarna. JSON och TOML ingår också. Kompilatorn använder båda formaten för projekt och verktyg. |
| Läsa eller skriva en ström | `std.io.Read`, `Write` och `BufRead`. Samma funktion kan ta emot en fil, stdin eller en `Cursor` i ett test. `std.fs`, `std.net` och `std.process` öppnar resurser med respektive kapabilitet. |
| Arbeta med filbeskrivare, systemets statuskoder eller FFI | `sys` och `std.dl`. Där finns även de primitiv som de typade standardmodulerna använder. |
| Använda en databas, tala HTTP eller utforma ett kommandoradsprogram | Välj `sqlite`, `http`, `http_client`, `cli` eller `log` i projektets `[dependencies]`. Paket kan ha egna systemberoenden och uppgraderas via manifestet. |

För JSON med känt schema ger `json.from_str<T>(text)?` ett typat värde.
För dynamiskt innehåll ger `json.Doc::parse(text)?` och
`json.Doc::parse_utf8(move bytes)?` ett dokument eller `JsonError` med
byteposition. Ett uppslagsanrop som `doc.get_str(...)` ger `Option<string>`.
För numerisk text kan `parse.value<i64>(text)?` på samma sätt lämna felet till anroparen.

[Projektberoenden](dependencies.md) · [SQLite](sqlite.md) · [HTTP och TLS](http.md) · [CLI](../packages/cli/) · [Loggning](../packages/log/)

```dunder
import std.map as map
import std.set as set
import std.array as array

fn main() {
    mut counts := map.Map<string, i64>::new()
    counts.put("dunder", 42)
    print(counts.get_or("dunder", 0))

    mut names := set.Set<string>::new()
    added := names.insert("å 🌍")
    assert(added)
    assert(names.contains("å 🌍"))

    mut values: []i64 = [3, 1, 2]
    values.push(4)
    array.sort<i64>(&mut values)
    print(array.sum(&values))
}
```

## Strömmande text och terminaler

`std.bytes.utf8.decode_prefix(&bytes)` ger `Prefix.Scalar(rune, antal_byte)`,
`Incomplete` eller `Invalid`. Behåll ett ofullständigt prefix tills nästa
läsning. Tom indata ger `Incomplete`. Efter en skalär kan resten av
bufferten innehålla fler tecken.

`std.text.lower_unicode(text)` följer Unicodes språkoberoende regler för gemener.
Exempelvis blir `İ` två skalärer (U+0069 U+0307), och `ΟΣ` blir `ος`.
Generera om tabellen med `dunder run tools/lowercase_data.ddr -- KATALOG stdlib/text/lowercase_data.ddr`.
Katalogen ska innehålla `UnicodeData.txt`, `SpecialCasing.txt` och `DerivedCoreProperties.txt` från Unicode 17.0.0.
[Unicode 17, avsnitt 3.13](https://www.unicode.org/versions/Unicode17.0.0/core-spec/chapter-3/#G33992) · [Databas](https://www.unicode.org/Public/17.0.0/ucd/) · [Licens](../tests/fixtures/unicode-17/LICENSE.txt)

`std.text.graphemes(text)` lånar en UTF-8-sträng. Varje element har
`start()` och `end()` som bytepositioner och `as_str()` för den lånade texten.
Iteratorn följer reglerna för utökade grafem i Unicode 17.0.0.
Tabellen kan återskapas med `dunder run tools/grapheme_data.ddr -- KATALOG stdlib/text/grapheme_data.ddr`.
Katalogen ska innehålla versionens `GraphemeBreakProperty.txt`, `DerivedCoreProperties.txt` och `emoji-data.txt`.
[Unicode Standard Annex 29](https://www.unicode.org/reports/tr29/tr29-47.html)

```dunder
import std.text
fn show(value: &str) {
    for part in text.graphemes(value) { print(part.as_str()) }
}
```

Med `std.text.lines(value)` kan läsaren stanna efter första raden. Kör
`advance()` för att läsa nästa rad och `current()` för dess lånade UTF-8-vy.
LF och CRLF avslutar rader. Ett ensamt CR i den sista raden hör till texten.

`std.bytes.ByteQueue::new()` ger en återanvändbar bytebuffert. `append`
lägger till indata, `pending()` lånar den olästa delen och `advance(n)`
returnerar `false` om antalet ligger utanför den delen. `remove(start, count)`
tar bort byte från den olästa delen och ger `false` för ogiltiga intervall. En UTF-8-läsare kan
behålla en ofullständig sekvens till nästa `append`.

`std.utf16.append(&mut units, value)` behåller inre nolltecken och skriver
surrogatpar för tecken över U+FFFF. `terminated(value)` ger en nollterminerad
sökväg och avvisar inre nollor med en tom array.

`std.terminal.columns(value)` mäter en lånad text i terminalceller.
Bredden följer Unicode 18 med smala tvetydiga tecken, presentationsväljare
och sammansatta emojier. C0- och C1-kontroller, inklusive radbrytning, ger
`.None`. Varje anrop mäter ett sammanhängande spann. Lam och alef tar en
cell tillsammans och två när de mäts i var sitt spann.

`std.time.local_from_unix(secs)` använder systemets lokala tidszon, inklusive
sommartid för det angivna ögonblicket. Den ger `.None` om systemet eller
`DateTime.year` saknar stöd för datumet. Windows använder FILETIME från
1601-01-01 UTC och systemets tidszonsregler.

`std.time.MonotonicClock.Raw.now_ns()` ger en klocka för korta intervall.
`resolution_ns()` ger den upplösning systemet anger, avrundad uppåt till hela
nanosekunder. Båda metoderna ger −1 vid fel. På macOS och Linux använder
raw-klockan `CLOCK_MONOTONIC_RAW` som går oberoende av frekvensjusteringar.
macOS räknar systemvila, Linux utelämnar den. Windows använder
prestandaräknaren för båda varianterna. `.Deadline.now_ns()` använder samma
klocka som `monotonic_ns()` och befintliga tidsgränser.

Med `capability stdio` ger `std.terminal.size()` standardutmatningens antal
kolumner och rader. `terminal.RawMode::enter()` ändrar standardinmatningens
läge och behåller ett eget handtag. Resursens `restore()` återställer läget
och kan anropas flera gånger. Om funktionen returnerar tidigt gör resursen
ett återställningsförsök vid scope-slut.

`terminal.window_size(handle)` läser cell- och pixelmått på en vald terminal.
Noll på en pixelaxel betyder att terminalen saknar uppgift för axeln.
Windows rapporterar skärmbuffertens synliga cellmått och noll för pixlarna.
`RawMode::enter_on(handle)` håller en egen kopia av handtaget för återställning.

`process.Command.spawn_with_handles(input, output, error_output)` tar tre
`Option<usize>`. `.Some(handle)` väljer en öppen POSIX-filbeskrivare eller
ett Windows-`HANDLE` för barnets ström. `.None` använder inställningen i
`set_stdio`. Anroparen äger originalet och håller det öppet under anropet.
Efter starten har barnet en egen referens.

## Växande arrayer

`[]T` är en typad dynamisk array; `[T; N]` är fortsatt en fast array.
Element kan vara primitiva värden, strängar, pekare, funktionsvärden,
structar, enums och arrayer. Annoterade literaler, även `[]`, får
elementtypen från kontexten.

| Operation | Beteende |
| --- | --- |
| `a.len`, `a.len()`, `len(a)` | Antal initierade element som `i32`. |
| `a.cap()` | Tillgänglig kapacitet i element. |
| `a[i]`, `a[i] = value` | Typad åtkomst med gränskontroll; skrivning kräver muterbar lagring. |
| `a.push(value)` | Lägger till ett element. Kapaciteten växer geometriskt. |
| `a.pop()` | Tar bort och returnerar sista elementet. Tom array ger status 103. |
| `a.remove(index)` | Flyttar ut elementet vid ett `i32`-index och flyttar efterföljande element ett steg åt vänster. Ogiltigt index ger status 103. |
| `array.swap_remove(&mut a, index)` | Flyttar ut elementet och fyller luckan med sista elementet i O(1). Den inbördes ordningen kan ändras. Ogiltigt index ger status 103. |
| `a.swap(i, j)` | Byter två element på plats utan kloning. Båda index kontrolleras före bytet; fel ger status 103. |
| `a.reserve(capacity)` | Säkerställer minst denna totalkapacitet i element och behåller längden. Om reservationen avvisas fäller programmet med status 109 på nativa mål och en trap i Wasm. |
| `a.try_reserve(capacity)` | Samma totalkapacitet. Returnerar `ReserveStatus` och behåller längd, kapacitet och element vid fel. |
| `a.clear()` | Nollställer längden och behåller bufferten för återanvändning. |
| `for item in a` | Läslånar samlingen under loopen, utan arraykopia. |

Matcha status från `try_reserve` med `.Ok`, `.InvalidCapacity` eller
`.OutOfMemory`. Negativa antal och antal vars buffert överskrider
målets blockgräns ger `.InvalidCapacity`. Poolbrist och en nekad
Wasm-minnesökning ger `.OutOfMemory`. Status lagras som en skalär enum.
Det går därför att läsa och matcha felet även när heapen är full.
Typen finns i preluden och som `std.array.ReserveStatus`.

`array.resize_with(&mut values, length, make)` anropar konstruktorn en gång
per nytt element och tar emot ägda resurser. Vid krympning släpps överskottet.
`array.repeat(move value, count)` klonar de första `count - 1` exemplaren
och flyttar det sista. Noll ger en tom array.


En växande strängarray tar ägarskap om elementet. `a.push(move text)`
flyttar en befintlig strängägare. `a.push(text.clone())` behåller `text`
och ger arrayen en egen referens till samma oföränderliga text.
Färska värden som `a.push(f"rad {i}")`, en ny fältprojektion ur ett temporärt
anrop (`a.push(make_pair().left)`) och loopbindningar från `for x in move
batch` kan lämnas direkt till `push` utan extra `move`.

Arrayer har unikt ägarskap. `next := values` flyttar ägaren; `values` kan
sedan inte läsas före återinitiering. `copy := values.clone()` ger en
oberoende buffert. Regeln följer arrayer även inne i structar och enums.
Vanliga parametrar lånar för läsning; `move`-parametrar tar över ägarskapet.
Flytt bevarar kapaciteten. En klon får egen kapacitet efter sin längd.

`swap` fungerar på dynamiska och fasta arrayer, inklusive muterbart lånade
och inline-lagrade arrayer. Indexuttrycken körs en gång från vänster till
höger; därefter används arrayens aktuella lagring. Buffert, längd och
kapacitet är oförändrade av själva bytet. Strängsorteringen använder detta
för att flytta handtag utan extra referensräkning.

`remove` kräver en lagrad dynamisk array som kan ändras via `mut` eller
`&mut`. Indexet utvärderas en gång. Borttagningen använder arrayens
aktuella lagring efter indexuttrycket, behåller bufferten och kapaciteten
och tar tid proportionell mot antalet efterföljande element.

## Läsa och lagra resurser

Bibliotekets läsoperationer lånar. Operationer som behåller indata tar över
ägaren med en `move`-parameter. En anropare som behöver två ägare skriver
`.clone()` vid anropet; arrayer får då en självständig buffert och strängar
delar oföränderlig text.

| Operation | Ägarskap |
| --- | --- |
| `Map.at`, `Map.get_or`, `Map.contains`, `Set.contains` | Lånar nyckeln. |
| `table.put(move key, move value)`, `set.insert(move key)` | Tar över nyckeln även vid en dubblett. `put` tar också över värdet. `put_new` sätter bara in om nyckeln saknas. |
| `table.keys()`, `set.values()` | Returnerar en ny array med klonade nycklar. |
| `Command::new(move program, move args)`, `add_arg`, `add_args`, `set_cwd`, `set_env`, `set_stdin` | Tar över lagrade strängar och buffertar. |
| `process.run(program, args)`, `Command::exec`, `Executable::exec` | Lånar under den synkrona körningen. |
| `process.replace(program, &args)`, `process.replace_as(program, argv0, &args)` | Ersätter den aktuella processen utan en ny PID. Vid fel är miljö, arbetskatalog och standardströmmar orörda. `replace_as` väljer uttryckligen `argv[0]` för multicall-program. |
| `json.Doc::parse_bytes(move input)` | Dokumentet tar över bytebufferten utan kopia. |
| `json.Doc::parse_bytes(input.clone())` | Begär uttryckligen ett dokument med en självständig snapshot. |
| `json.Doc::parse_text(text)` | Lånar strängen och bygger dokumentets UTF-8-buffert. |
| `StreamSet::insert(&mut streams, move stream)` | Flyttar en stream till mängden. |

`Command::spawn` och `Command::exec` nollställer signalmasken i barnprocessen
före programstart på macOS och Linux. Barnet startar med standardåtgärden för
SIGINT, SIGTERM och SIGPIPE, även när föräldern bevakar eller ignorerar dem.
`replace` och `replace_as` ersätter samma process och behåller dess signalmask
på macOS och Linux. På Windows returnerar dessa anrop `Unsupported`.
`Command::exec` och `Command::spawn` startar en ny Windowsprocess.

`ChildSet::insert(&mut children, &mut child)` är ett fallibelt
överföringsförsök: framgång lämnar barnet i mängden och tömmer källhandtaget;
`-1` lämnar det oförändrat hos anroparen. Detta kräver ett skrivlån.
`ChildSet.try_insert(move child)` tar emot ägaren och returnerar
`Result<i32, Child>`. `.Ok(slot)` innehåller platsen i mängden. `.Err(child)`
ger tillbaka barnprocessens handtag till anroparen. För resultattyper med
tom nyttolast (`Result<T, ()>`) skrivs varianten utan parenteser (`.Ok`
respektive `.Err`), och metoder på `Result` skrivs alltid med parenteser:
`.is_ok()`, `.is_err()`, `.unwrap_or(...)` och `.unwrap_or_default()`.
`ChildSet::poll_io(timeout_ms)` väntar på körbar I/O och returnerar
`Result<i32, sys.IoError>` med antalet färdiga rörbeskrivare från `poll`;
`ChildSet::wait_any` är ett alias med samma returvärde
(antal färdiga filbeskrivare, inte en enskild slotindexering).

Sockets, socketmängder, barnprocesser och barnprocessmängder är
[`@resource`-typer](memory.md#unika-externa-resurser). De flyttas och kan
inte klonas, inte heller genom en omgivande struct eller enum.
Sockets och socketmängder stängs automatiskt när ägaren släpps.
Explicita `close`-anrop rapporterar stängningens status. Barnprocesser
hanteras med `wait`, `terminate` och bibliotekets övriga processoperationer.

## Begränsade borrows

```dunder
fn append(values: &mut []i64, value: i64) {
    values.push(value)
}

fn first(values: []i64) -> i64 {
    values[0]
}
```

`append(&mut values, 42)` lånar muterbar lagring under anropet;
`first(values)` lånar för läsning; explicit `&values` fungerar också. Referensparametrar derefereras automatiskt
vid läsning, fältåtkomst och indexering. En läsreferens kan inte modifieras.
Oföränderliga bindningar kan inte lånas med `&mut`.

Läslån kan också namnges lokalt med `r := &value` och gäller då till
sista lexikala läsningen i blocket. `&values[start..end]` ger en lånad `&[]T` och
`&text[start..end]` (liksom `text.as_str()`) ger en UTF-8-kontrollerad lånad
strängvy, `&str`, utan ny allokering (`v.as_bytes()` ger motsvarande `&[]u8`).
Lokala skrivlån skrivs `r := &mut value`. En tilldelning genom `r` ändrar ägaren.
Två oberoende structfält kan lånas för skrivning samtidigt. Ett elementlån
skyddar hela arrayen. Operationer som kan flytta bufferten måste vänta tills
lånet har upphört.

En funktion kan returnera ett lån från en parameter. Returtypen `&mut T` kräver
en muterbar källa. Ange `from namn` när källan behöver särskiljas.
En lokal struct eller enum kan innehålla delade lån från flera ägare. Vid retur måste lånen komma från parametern i funktionens returkontrakt.
`Option<&T>` representerar ett sådant lån eller ett frånvarande värde.
En callback kan ge tillfällig åtkomst till ett värde som kanske saknas.
För en ägd kopia används `.clone()`, som på `&str` ger en `string`.
[Lån och intervallvyer](memory.md#lokala-läslån-och-intervallvyer)

## Moduler

Vid migrering av äldre kod med `result.ok` och `result.value` på en
`Result<T, E>` ger `result.is_ok()` ett booleskt svar. Med `match` kommer
du åt både nyttolasten i `.Ok(value)` och felet i `.Err(error)`.
`result.ok()` ger en `Option<T>`. Äldre sökkod som jämför ett index med
`-1` kan i stället matcha `text.find(texten, delen)` eller
`bytes.find_byte(&data, start, end, byte)` mot `.Some(index)` och `.None`.

| Import | Innehåll |
| --- | --- |
| Preludium (`runtime/prelude`) | Inbyggda traits `Ordering` (`Less`, `Equal`, `Greater`), `Eq`, `Ord`, `Hash`, `Display`, `Default`, `Option<T>` (`Some(T)`, `None` med `is_some`/`is_none`/`unwrap`/`unwrap_or`/`map`/`ok_or`/`?`), `Result<T, E>` (med `is_ok`/`is_err`/`unwrap`/`unwrap_or`/`unwrap_or_default`/`map`/`map_err`/`and_then`/`ok`, `.Ok`/`.Err` utan parenteser för `()`, samt villkorliga `Eq`/`Ord`/`Hash`/`Display`) och implementationer för primitiver, `[]T` och tupler. |
| `std.cmp` | Trait-generisk ordning och gränser: `min<T>`, `max<T>`, `clamp<T>` (`T: Ord + Storable`), `Ordering`/`Eq`/`Ord` (re-exporterade) samt omslagstypen `Reverse<T>` med villkorliga `Eq`, `Ord`, `Hash`, `Display`, `Default` och `Sendable`. |
| `std.array` | `ints`, `strings`, `sum` samt generiska algoritmer (`reverse`, `sort`, `sort_by`, `is_sorted`, `binary_search`, `contains`, `index_of`, `equal`, `min_index`, `max_index`, `dedup`, `append`, `fill`, `resize`, `resize_with`, `repeat`, `join`) ovanpå `Ord`/`Eq`/`Clone`/`Display`. `sort` och `sort_by` använder introsort på plats med heapsort-fallback, O(n log n); strängar jämförs som UTF-8-byte. |
| `std.map` | [`Map<K, V>`](#generiska-samlingar) (`K: Hash + Storable`, `V: Storable`) med `Default`, villkorlig `Eq` och `Sendable`. |
| `std.set` | [`Set<K>`](#generiska-samlingar) (`K: Hash + Storable`) ovanpå `Map<K, bool>` med `Default`, villkorlig `Eq` och `Sendable`. |
| `std.hash` | `hash(text)`, `mix`, `combine`, `of<T: Hash + Storable>`, `Hash` och `Hasher` (med `Default`, `.write_u64`, `.write_i64`, `.write_bytes`, `.write_str`, `.finish`). |
| `std.text` | `starts_with`, `ends_with`, `find` (`Option<i32>`), `contains`, `replace`, ASCII-`lower`/`upper`, `lower_unicode`, `pad_left`/`pad_right`, `split`, `trim`. Utfyllnaden upprepar mönstret tills strängen når önskat antal Unicode-skalärer. Ett tomt mönster ger mellanslag och en längre indatasträng behåller sin längd. |
| `std.text.view` | Nollallokerande `&str`-vyhjälpare: `starts_with`, `ends_with`, `find` (`Option<i32>`), `contains`, `trim_start`, `trim_end`, `trim`, `strip_prefix`, `strip_suffix`, `before`, `after`, `scalar_at`. |
| `std.path` | Sökvägar utan disk-I/O. Normalisering bevarar enhetsrötter som `C:/` och nätverksresurser som `//server/share`. Modulen innehåller `is_absolute`, `join`, `basename`, `dirname`, `extension` (`Option<string>`), `stem`, `with_extension` och `normalize`. |
| `std.time` | `monotonic_ns`, `monotonic_ms`, väggklocka (`unix_now`, `unix_now_secs`, `unix_now_ms`), `UnixTime`, `DateTime`, `utc_from_unix`, `unix_from_utc`, `format_rfc3339`, `format_rfc3339_ms` och `format_http_date`. |
| `std.signal` | Portabel signalhantering för tjänster (`Signal`: `Int`, `Pipe`, `Term`; `ignore_sigpipe`/`restore_sigpipe`, `install_shutdown_handler`, `shutdown_fd`, `shutdown_requested`, `take_shutdown_signal`, `SignalWatcher`). |
| `std.sync` | Trådsäkra koordineringsprimitiv utan delad muterbar heap: begränsad `Channel<T>` (`channel`, `Sender<T>`, `Receiver<T>`, `TryRecv<T>`, `SendResult<T>`, `select2`, `select2_cancel`, `select_ready`, `select_ready_cancel`), uppdaterbara frysta ögonblicksbilder (`Published<T>`, `ReadGuard<T>`), `Semaphore`/`SemaphorePermit`, `CancelToken` (kopplad till `std.signal`), `AtomicI64` och `AtomicBool`. |
| `std.base64` | RFC 4648 Base64 och URL-säker Base64 (`encode`, `encode_no_pad`, `encode_url`, `encode_url_no_pad`, `decode`, `decode_url`) med strikt kontroll av utfyllnad och avslutande bitar. |
| `std.bytes` | Packade bytebuffertar (`[]u8`), `from_text`, `from_i32`, `to_i32`, `append_utf8`, `extend`, `pad`, `zeroed`, nollfyllning, little-/big-endian-kodning (`append_le`, `append_be`, `set_be`, `u16_le`, `u16_be`, `i16_be`, `u32_le`, `u32_be`, `i32_le`, `i32_be`, `i64_le`, `i64_be`, `u64_le`, `u64_be`), `hex`, `hex_u8` och kontrollerad UTF-8-avkodning (`decode`, `decode_range`). |
| `std.wire` | Binär serialisering till anroparens `[]u8` (`u8`, `bool`, fasta little-endian-tal, `f32`/`f64`, LEB128 `uvarint`, ZigZag `varint`, längdprefixade `bytes` och `str`, `Reader::new`/`Default` samt traits `Encode`/`Decode` och `wire.encode<T>`/`wire.decode<T>`). |
| `std.json` | `Writer` (`Default`, `.finish() -> Result<string, JsonError>`), `Encode` & `Decode` (`@derive(Encode, Decode)`), `json.encode<T>`/`json.field_value<T>`/`json.to_string<T>`/`json.to_bytes<T>` för skrivning och `json.decode<T>`/`json.decode_field<T>`/`json.from_bytes<T>`/`json.from_str<T>` för läsning, samt `Doc::parse`/`Doc::parse_utf8` (`Result<Doc, JsonError>`) och `Doc::parse_bytes`/`Doc::parse_text` med `Option<T>`-fält (`get_str`, `get_i64`, `get_u64`, `get_f64`, `get_bool`), `.raw_span` och `.raw_json`. |
| `std.sha256` | `[]u8`-API:t `digest_bytes(&[]u8)`, `digest_text(string)`, `digest_range(&[]u8, start, len)`, `hex_bytes(&[]u8)` och `hex_text(string)` ger 32 byte respektive hexsträng; `digest(&[]u8)` är ett alias för `digest_bytes`. |
| `std.parse` | `FromStr` och `parse.value<T: FromStr>` (implementerad för `bool`, `string`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`, `usize`, `f32`, `f64`) samt `Integer::parse`/`Integer::hex`/`Integer::range`, `Unsigned::range`/`Unsigned::hex_range`, `Float32::parse`/`Float64::parse`. `parse` accepterar understreck mellan siffror. |
| `std.math` | `clamp`, `min`, `max`, `sin`, `cos`, `tan`, `total_cmp_f64`/`total_cmp_f32` samt fällfri `checked_*`-heltalsaritmetik; se [Matematik](#matematik). |
| `std.io` | Ström-traits (`Read`, `Write`, `BufRead: Read`, `Seek`), `SeekFrom`, `Line`, kapabilitetsstyrda `Stdin`/`Stdout`/`Stderr` (`capability stdio`) och `read_stdin`/`read_stdin_bytes`/`read_stdin_limit`, samt ägda omslag (`BufReader<R>`, `BufWriter<W>` med `FlushError<W>`, `Limited<R>`, `Cursor`, `Empty`, `Sink`) och generiska algoritmer (`copy`, `copy_limit`, `write_all`, `write_str`, `read_exact`, `read_to_end`, `read_to_end_limit`, `read_to_string`, `is_would_block`). |
| `std.alloc` | Generellt allokerings-API: `AllocError`, `Layout`, `layout_of<T>`, `layout_array<T>`, `Block`, traiten `RawAllocator` (`unsafe fn alloc`/`dealloc`/`alloc_zeroed`/`realloc`), `Global`, `System`, `Capped<A>`, `Stats`, `Tracking<A>`, `Bump<A>`, `OwnedBuf<A>` (`as_bytes`, `io.Write`), `Span`, `Arena<A>`/`FrozenArena<A>` (`freeze_arena`, `read_into_arena`) samt generationssäkra `Pool<T: Storable>`/`Id<T>`. |
| `std.net` | Kapabilitetsstyrd TCP och UDP för IPv4/IPv6 (`TcpStream` implementerar `io.Read` och `io.Write`; `TcpListener`, `TcpStream` och `UdpSocket` implementerar `Sendable`), `IpAddr`, `SocketAddr`, `PollSet`, `Waker` (`Sendable`), `StreamSet` och `resolve`. |
| `sys` / `std.sys` | macOS-/Linux-filer, argument, byteutdata, UTF-8-avkodning, klocka, `sleep_ms`, process-ID, `Metadata` (inklusive `is_symlink`, `mode`, `uid`, `gid`, `nlink`, `dev`, `ino`, `mtime`), `IoError` (`Display`/`Eq`/`Hash`, inklusive `Interrupted`, `NoSpace`, `UnexpectedEof`, `WriteZero`, `would_block`, `is_would_block`), `FileKind`, `Status`, lågnivå-FD (`fd_read`/`fd_write`/`fd_close`), samt `unsafe`-primitiverna `memory_zero`, `pool_alloc_try`, `pool_free`, `atomic_*_i64`, `envp` och `linux_syscall`; se [sys](sys.md). |
| `std.random` | Slumpbyte från operativsystemet. `bytes(antal)` skapar en buffert och `fill(&mut buffert)` fyller en befintlig. Båda returnerar `Result` med `sys.IoError`. |
| `std.crypto` | Argon2id, HMAC-SHA256, HKDF-SHA256, konstanttidslikhet, AES-256-GCM, Ed25519-signaturer, `zeroize` och `SecretBytes`. `crypto.random` hämtar byte från `std.random`. Se [Kryptografi](crypto.md). |
| `std.toml` / `std.toml.tree` | TOML-parser (`toml.parse`), DOM-träd (`Tree` med `Sendable` och `Default`) och JSON-rendering; se [TOML](toml.md). |
| `std.dl` | Dynamisk laddning av systembibliotek och uppslagning av symboler via Dunders typade [FFI](ffi.md). |
| `std.fs` | Delegerade läs-/skriv-/kataloghandtag med `impl` (`ReadFile::grant`/`.read`/`.open`, `WriteFile::grant`/`.write`/`.create`/`.open_append`, `ReadDir::grant`/`.resolve_read`/`.list`), strömmande `@resource(dispose)`-handtag (`OpenOptions` med `set_nofollow`, `FileReader`, `FileWriter`, `File` med `io.Read`/`io.Write`/`io.Seek`, `Sendable`) samt filsystemshjälpare (`cwd`, `realpath`, `mkdir_all`, `temp_dir`, `temp_file`, `list_dir`, `remove_all`, `symlink`, `hard_link`, `is_symlink`, `metadata`, `lstat`, `chmod`, `sync_dir`, `write_private_atomic`, `write_private_text_atomic`). Se [filsystemets kapabiliteter](ffi.md). |
| `std.process` | Kapabilitetsstyrd processkörning (`capability process`): `Command::new` / `.exec` / `.spawn` / `.add_arg`, `Output::succeeded` / `.stdout_text`, `Executable::grant` / `.exec` / `.run` / `.spawn`, `Child` (`.wait`, `.try_wait`, `.read_stdout`, `.write_stdin`, `.take_stdin`, `.take_stdout`, `.take_stderr`, `.terminate`, `.kill`), strömhandtagen `ChildStdin`/`ChildStdout`/`ChildStderr`, `ChildSet::new` samt modulfunktioner och miljö-/värdfrågor (`env`, `host_os`, `host_arch`, `host_target`, `host_cpus`). På Linux körs den via direkta systemanrop (`sys.linux_syscall`) och start-`envp` (`sys.envp`) helt utan `libc` eller `libdl` (statisk ELF). |
| `packages/http`, `packages/http_client`, `packages/cli`, `packages/log` | Fristående ekosystempaket i repots `packages/`: HTTP/1.1 keep-alive-server & webbprimitiv, TLS HTTP-klient, kapabilitetsfri CLI-parser (`cli.App`/`Command`/`parse`/`parse_argv`/`render_help`) samt strukturerad loggning till valfri `io.Write` (`log.Logger`, `Text`/`Json`, `Field`). |

I `std.bytes` ger `u16_le` och `u16_be` ett `i32` som rymmer hela det
osignerade 16-bitarsvärdet. Motsvarande `u32`-läsare ger `i64`.
Använd `u32(bytes.u32_le(&data, offset))` för ett typat 32-bitarsord
eller `bytes.i32_le(&data, offset)` för ett signerat bitmönster.
`wire.Reader.read_u32_le` ger `u32` och flyttar läspositionen.

Map och set använder öppen adressering, linjär sondering och tombstones.
Tillväxt och omhashning är Dunder-kod. Uppdatering av en befintlig mapnyckel
ändrar inte antalet poster. `Set.insert` returnerar sant för ett nytt värde;
`remove` returnerar sant när något togs bort. Iterationsordningen hos
`Map.keys` och `Set.values` är ospecificerad. Strängnycklar jämförs
som UTF-8-byte, inklusive tomma strängar och NUL, utan Unicode-normalisering.

Använd `Map<K, V>::new()` och `Set<K>::new()` (eller `Default::default`)
och anropa metoder direkt på värdet (`len`, `is_empty`, `put`, `put_new`,
`insert`, `contains`, `remove`, `take`, `values`). För uppslag som kan
saknas ger `index_of` ett index eller `-1`.

`at` lånar det lagrade värdet. I `antal := m.at(k); m.put(k, antal + 1)`
beräknas heltalsargumentet innan metoden uppdaterar tabellen. En senare
läsning av samma lån efter uppdateringen ger ett kompileringsfel.

`text.find` returnerar `Option<i32>` (`.Some(byteindex)` eller `.None`).
`split` bevarar tomma delar;
en tom separator ger en del med hela texten. `trim` tar ASCII-mellanslag,
tabb, CR och LF. `parse.integer`, `parse.hex`, `parse.i64_range`, `parse.u64_range`
och `parse.hex_u64_range` ger fel 1 för stavning och 2 för overflow; både
i64- och u64-gränser stöds. `bytes` har även hjälpare för sökning och buffertar
(`decode_range`, `find_byte`, `find_crlf`, `find_double_crlf`, `equals_text`,
`equals_ascii_ci`, `extend_range`, `consume_prefix`, `append_decimal`, `append_u64`).
`append_u64` lägger den osignerade decimalskrivningen sist i bufferten.
`bytes.decode(&data)` och `bytes.decode_range(&data, start, end)` ger
`Result<string, sys.IoError>` och kan användas med `?`. Ett tomt intervall
ger en tom sträng. Ett ogiltigt byteintervall ger `.InvalidArgument`.
Ogiltig UTF-8 inom ett giltigt intervall ger `.InvalidUtf8`, även när en
gräns delar ett flerbytetecken. Slutindex är exklusivt.

`std.net` ger kapabilitetsstyrd TCP och UDP för IPv4 och IPv6 ovanpå
`capability network` med adresstyperna `IpAddr` (`parse`, `Eq`, `Hash`, `Display`,
`Sendable`, `Copy`) och `SocketAddr` (`parse`, `Eq`, `Hash`, `Display`, `Sendable`,
`Copy`), resurshandtagen `TcpListener` (`listen`, `listen_addr`, `accept`,
`local_addr`, `local_port`, `close`), `TcpStream` (`connect`, `connect_addr`,
`connect_timeout`, `connect_host`, `local_addr`, `peer_addr`, `read_append`,
`peek_append`, `write_range`, `write_all`, `shutdown_read`, `shutdown_write`,
`shutdown_both`, `set_read_timeout`, `set_write_timeout`, `close`),
`UdpSocket` (`bind`, `connect`, `local_addr`, `peer_addr`, `send_to`,
`send_to_range`, `send`, `recv_from_append`, `peek_from_append`, `recv_append`,
`set_read_timeout`, `set_write_timeout`, `close`), samt `PollSet` (`new`/`Default`)
och `StreamSet` (`new`/`Default`, `peek_append`, `shutdown_read`, `shutdown_both`,
`local_addr`, `peer_addr`, timeouts) och motsvarande modulhjälpare. `peek_append` och
`peek_from_append` lämnar lästa byte kvar i socketens kö. TCP- och
UDP-operationerna använder direkta systemanrop på Linux och behåller statiska
ELF-binärer; `net.resolve` och `TcpStream::connect_host` använder `getaddrinfo`.

## Generiska samlingar

`Map<K, V>` och `Set<T>` är generiska structar skrivna i Dunder.
Kompilatorn specialiserar dem med samma regler som [egna generiska deklarationer](syntax.md#generiska-deklarationer). De kan även skrivas `map.Map<K, V>`
och `set.Set<T>` efter import av standardmodulerna.

```dunder
fn main() {
    mut table := Map<string, []i32>::new()
    samples: []i32 = [21, 42]
    table.put("values", move samples)
    assert(table.at("values")[1] == 42)
    match table.take("values") {
        .Some(values) => { assert(values.len == 2) },
        .None => { assert(false) },
    }
}
```

Nyckeltypen `K` kräver `Hash + Storable` (vilket även innefattar supertraiten
`Eq`). Strängar, heltal, `bool`, `rune`, `[]T`, tupler `(A, B)`/`(A, B, C)` och
`Result<T, E>` (med `Eq`/`Hash`-element) uppfyller detta direkt; flyttal har
avsiktligen ingen `Hash`/`Eq`-implementation. Egna typer implementerar `Eq` och
`Hash`:

```dunder
struct Key { id: i64 }

impl Eq for Key { fn eq(&self, other: &Self) -> bool { self.id == other.id } }
impl Hash for Key { fn hash(&self) -> u64 { self.id.hash() } }
```

Likvärdiga nycklar måste ha samma hash. Hash och likhet ska vara stabila medan
nyckeln lagras. Nycklar och värden använder samma lagringsregler som arrayelement.
Kraven kontrolleras redan när samlingstypen används, även för en tom samling.
[Generiska deklarationer](syntax.md#generiska-deklarationer)

`put`, `put_new` och `insert` tar över sina ägda argument. Vid en dubblett behåller
`put` den lagrade nyckeln och ersätter värdet, medan `put_new` lämnar befintlig post
orörd och returnerar `false`. `insert` returnerar `false` och släpper den inkommande
nyckeln. Omhashning flyttar bara index. `take` överför det lagrade värdet eller
setelementet i `.Some` och ger `.None` vid en saknad nyckel. `remove` och `clear`
släpper borttagna ägare via de vanliga cleanupreglerna. En `.clone()` av hela
samlingen kräver klonbara nycklar och värden.
`Set.take_at(index)` tar ut ett ägt element via dess index.

`Map.index_of(key)` ger postens index eller `-1`. `key_at(index)`
och `value_at(index)` lånar posten. `Map.at(key)` lånar värdet och
kräver att nyckeln finns. `Set.at(index)` lånar elementet vid indexet.
`Map.with_value(key, f)` ger callbacken ett läslån av värdet.
`Map.update(key, f)` ger ett skrivlån. Båda gör ett uppslag och anropar
callbacken en gång vid träff. Vid saknad nyckel ger metoden `false` och
hoppar över callbacken. Det fungerar även när värdet är en unik resurs.
`Map.get_ref(key)` ger `.Some(&värde)` vid träff och `.None` vid miss.
Lånet hör till tabellen och fungerar även för unika resurser.
`Map.get(key)` ger en ägd `Option<V>` och kräver `V: Clone`.
`at_mut` och `get_mut` ger ett direkt skrivlån och kräver en befintlig nyckel.
`Map.get_or(key, move fallback)` och `Map.get_or_default(key)` (för `V: Default + Clone`)
returnerar en klon av värdet eller ett reserv-/standardvärde vid en saknad nyckel.
`Map.keys()` och `Set.values()` klonar alla lagrade nycklar till en ny array.
Både `Map` och `Set` implementerar `Default`, `Eq` (när nyckel/värde är `Eq`) och
villkorlig `Sendable`.
Skrivning till samlingen kräver att lånen har upphört. Använd intervallet
`0..len()` för att läsa poster i aktuell ordning. Borttagning kan byta
plats på den sista kvarvarande posten och den borttagna posten.

## Matematik

`math.exp(x)` ger e upphöjt till x, `math.log(x)` ger den naturliga
logaritmen och `math.pow(x, y)` ger x upphöjt till y. Alla tre arbetar
med `f64` på nativa mål och i Wasm. `exp(-∞)` ger +0 och `exp(+∞)`
ger +∞. `log(±0)` ger −∞ och negativa argument ger NaN. En ändlig
negativ bas kräver heltalsexponent. För udda heltalsexponenter bevarar
`pow` basens tecken även vid noll och oändlighet.
`pow(x, 0)`, `pow(1, y)` och `pow(-1, ±∞)` ger 1.

Verifieringen jämför med värdsystemets matematikbibliotek och tillåter
högst åtta ULP (steg mellan representerbara tal) i referensfallen.
Proven omfattar subnormala värden, grannar kring över- och underflöde och
baser nära ett med stora exponenter.

`import std.math as math` ger `min(a, b)`, `max(a, b)` och
`clamp(value, low, high)` för `f64`. `min` väljer negativ noll och `max`
positiv noll när båda tecknen förekommer. `clamp` bevarar tecknet på
en nolla inom intervallet. NaN i något argument ger NaN. `clamp`
ger även NaN om den undre gränsen är större än den övre.

`math.sin(angle)`, `math.cos(angle)` och `math.tan(angle)` tar `f64`
i radianer. De finns på de nativa målen och i WebAssembly. `sin` och
`tan` bevarar tecknet på noll, medan `cos(0.0)` ger `1.0`.
Oändliga vinklar och NaN ger NaN.

`math.PI` är π som `f64`. `math.atan2(y, x)` ger vinkeln i radianer från
positiva x-axeln i intervallet [−π, π]. På negativa x-axeln väljer tecknet
på y mellan +π och −π, även när y är noll. NaN i någon koordinat ger NaN.

`math.total_cmp_f64(a, b)` och `math.total_cmp_f32(a, b)` ger IEEE 754 total
ordning som `Ordering` (`-NaN < -inf < ... < -0.0 < +0.0 < ... < +inf < +NaN`),
lämplig att skicka till `array.sort_by` när flyttal behöver deterministisk ordning.

För fällfri aritmetik på indata erbjuder `std.math` explicita
`checked_*`-funktioner som returnerar `Result<T, ()>` (`.Err` vid overflow
eller division/rest med noll i stället för processfälla):
`checked_add_*`, `checked_sub_*`, `checked_mul_*`, `checked_div_*` och
`checked_rem_*` för typerna `i32`, `i64`, `u32`, `u64` och `usize`, samt
`checked_neg_i32` och `checked_neg_i64`.

`x.abs()` ger absolutvärdet av ett `i32` eller `i64`. Typens minsta värde
ger samma spillfel (101) som `-x`.

Stora vinklar reduceras med binär fastpunktsaritmetik enligt
[Payne–Hanek-metoden](https://perso.ens-lyon.fr/jean-michel.muller/RangeReductionIEEETC0305.pdf).

## Skriva JSON

```dunder
import std.json as json

fn main() {
    mut writer := json.Writer::new()
    writer.object()
    writer.key("name")
    writer.string("Skrift")
    writer.key("samples")
    writer.array()
    writer.decimal(1.25)
    writer.integer(2)
    writer.end()
    writer.end()
    encoded := writer.finish().unwrap()
    print(encoded)
}
```

`Writer` (som även implementerar `Default`) sköter skiljetecken för objekt och
arrayer och returnerar `Result<string, JsonError>` från `.finish()` respektive `Result<[]u8, JsonError>` från `.finish_bytes()`. Typade structar och värden kan dessutom serialiseras och avkodas via traiten `json.Encode` och `json.Decode` (samt `@derive(Encode, Decode)`), tillsammans med hjälparna `json.encode<T>`, `json.decode<T>`, `json.field_value<T>` och `json.to_string<T>`. Anropa `key` före
värdet i ett objekt och `end` för varje öppnad behållare. `finish()`
kräver precis ett komplett rotvärde och en giltig följd av anrop.
`decimal`, `write_f64` och `field_f64` avvisar NaN och oändlighet.
`Doc::as_*` och `Doc::get_*` returnerar `Option<T>` (`None` för saknade värden,
fel JSON-typ eller flyttal utanför det ändliga binary64-området).
`Doc.raw_span(node)` returnerar källtextens spann som `string`: för objekt,
arrayer, tal, bool och `null` ges hela JSON-tokenet, medan strängnoder ger
det inre råa stränginnehållet utan omgivande citattecken (med eventuella
escape-sekvenser oförändrade). Använd `Doc.raw_json(node)` när giltig
JSON-källtext behövs för alla nodtyper, inklusive citattecken runt strängar.

`Doc.members(node)` ger par av avkodad nyckel och värdenod i källordning.
Upprepade nycklar finns med. `Doc.children(node)` ger en arrays värdenoder
i ordning. Båda iteratorerna lånar dokumentet och går igenom barnen en gång.

Med `@derive(Decode(deny_duplicate_fields))` på en struct ger andra förekomsten
av ett känt fält fel vid nyckelns inledande citattecken. Nycklar jämförs efter
avkodning av escape-sekvenser. Okända fält hoppas över. Vanlig `@derive(Decode)`
använder det sista värdet för en upprepad nyckel.

`[T; N]` och `(A, B)` kodas som JSON-arrayer. Avkodning kräver rätt antal
element och kontrollerar varje elements typ. En egen kodare kan anropa
`writer.fail(meddelande)`. Både `finish()` och `finish_bytes()` ger det första felet.

## Bitoperationer

Heltal har explicita `.bit_and`, `.bit_or`, `.bit_xor`, `.bit_not`, `.shl`
och logisk `.shr`. Skiftantal är `i32` och måste ligga inom typens bitbredd;
annars ges status 103. `.wrapping_add`, `.wrapping_sub`, `.wrapping_mul`
väljer omslag uttryckligt; vanlig aritmetik behåller sina overflowkontroller.
Bytekodning och SHA-256 dogfoodar dessa operationer.

## Strömmar och standardinmatning

För radbaserade format med en storleksgräns, använd
`reader.read_line_limit(max_bytes)`. Gränsen gäller inlästa byte per rad
inklusive eventuell LF eller CRLF. En sista rad kan sluta vid EOF.
Vid `.TooLarge` har läsaren förbrukat precis gränsens antal byte och nästa
byte ligger kvar. En negativ gräns ger `.InvalidArgument`. För binära
poster finns `read_until_limit(delim, &mut out, max_bytes)`, som räknar
nytillagda byte och behåller avgränsaren i utdata.

`std.io` definierar de generiska ström-traitsen `Read` (`fn read(&mut self, out: &mut []u8, max_bytes: i32) -> Result<i32, sys.IoError>`), `Write` (`write`, `flush`, plus standardmetoderna `write_full`, `write_str` och `write_byte`), `BufRead: Read` (`fill`, `buffered(&self) -> &[]u8 from self`, `consume`, samt standardmetoderna `read_until`, `read_line_into` och `read_line -> Result<Line, sys.IoError>`) och `Seek` (`seek(&mut self, pos: SeekFrom) -> Result<i64, sys.IoError>`).

Åtkomst till standardströmmar skyddas av `capability stdio` (`@requires(stdio)`): `io.stdin()`, `io.stdout()`, `io.stderr()` returnerar ägda `@resource`-handtag (`Stdin`, `Stdout`, `Stderr` med `.fd()` och `.is_tty()`), medan `io.read_stdin()`, `io.read_stdin_bytes()` och `io.read_stdin_limit(max_bytes)` läser hela standardinmatningen tills EOF. Strömmar byggs samman med ägda värdeomslag (`BufReader<R>`, `BufWriter<W>` med `FlushError<W>`, `Limited<R>`, `Cursor`, `Empty`, `Sink`) och generiska funktioner (`copy`, `copy_limit`, `write_all`, `write_str`, `read_exact`, `read_to_end`, `read_to_end_limit`, `read_to_string`). Samma traits implementeras av `fs.FileReader`, `fs.FileWriter` och `fs.File` i `std.fs` (via `ReadFile::open`, `WriteFile::create`/`open_append` och `File::open` med `OpenOptions`), av `net.TcpStream` i `std.net` samt av `ChildStdin`, `ChildStdout` och `ChildStderr` från `Child::take_stdin`/`take_stdout`/`take_stderr` i `std.process`.

```dunder
capability stdio
import std.io as io

fn main() {
    mut br := io.BufReader<io.Stdin>::new(io.stdin())
    mut bw := io.BufWriter<io.Stdout>::new(io.stdout())
    while true {
        match br.read_line() {
            .Ok(.Data(line)) => { _ := bw.write_str(line + "\n") },
            .Ok(.Eof) => { break },
            .Err(_) => { break },
        }
    }
    _ := bw.flush()
}
```

## Generell allokering

`std.alloc` erbjuder ett uttryckligt allokeringslager ovanpå `Layout`, `layout_of<T>()`, `layout_array<T>(count)`, `Block`, `AllocError` (`InvalidLayout`, `OutOfMemory`, `LimitExceeded`, `Unsupported`) och traiten `RawAllocator` med `unsafe fn`-metoderna `alloc`, `dealloc` samt standardmetoderna `alloc_zeroed` och `realloc`. Färdiga allokatorer är `Global` (ovanpå `sys.pool_alloc_try` / `sys.pool_free` utan fällor), `System` (OS-sidor via `mmap`/`munmap`), `Capped<A>`, `Tracking<A>` (som samlar `Stats`) och `Bump<A>` (snabb bump-allokator med LIFO-återvinning, `.reset()`, `.release()` och automatisk `dispose(&mut self)`).

För typat och lånekontrollerat minne finns tre `@resource(dispose)`-strukturer: `OwnedBuf<A>` (växande bytebuffert med `.as_bytes()`, `io.Write` och `into_allocator`/`buf_into_allocator`), `Arena<A>` / `FrozenArena<A>` med stabila `Span`-index (`push_bytes`, `push_str`, `alloc_zeroed`, `read_into_arena`, `freeze_arena`) samt generationssäkra `Pool<T: Storable>`/`Id<T>` (`Default`, och `Sendable` när `T: Sendable`).

## Lagring och verifiering

På nativa mål hämtar arrayer minne från en [heap som växer vid behov](memory.md#pool-och-kostnader).
Heapen återanvänder frigjorda block inom samma storleksklass.
Buffertarna har unika ägare och uttryckliga kloner är oberoende.
Tillväxt frigör den gamla bufferten efter överföringen.
Tilldelning, scope-slut och retur frigör övergivna buffertar, även i nästlade
structar och aktiva enumpayloads. `clear` frigör elementens strängreferenser
och behåller själva bufferten för nästa användning.
Full pool, misslyckad mappning eller för stor/negativ arrayreserv ger
status 109. För uttrycklig, fällfri blockallokering och arenor används
i stället [`std.alloc`](#generell-allokering).
Strängar har referensräkning och delar heap med arrayer.
Allokatorn återanvänder block per storleksklass. Läs [kostnader och gränser](memory.md).

```sh
dunder test tests/suite.ddr
dunder run tools/test_runtime_contracts.ddr -- "$(command -v dunder)"
dunder run tools/test_io.ddr -- "$(command -v dunder)"
dunder run tools/test_alloc.ddr -- "$(command -v dunder)"
```

Testerna jämför map, set och sortering mot förväntade värden i en
[körbar fixtur](../tests/fixtures/runtime_contracts/map_set_sort.ddr). De
provar även Unicode och NUL, olika elementbredder, djupa värdekopior,
callbacks och aliasfel.
SHA-256 kontrolleras mot referensvektorer och används av den
[Dunder-skrivna kompilatorns](../selfhost/README.md) Mach-O-signering.


I `for item in &mut values` är `item` ett exklusivt lån av det aktuella
arrayelementet. Loopen kan skriva genom lånet och använda `break` eller
`continue`. Arrayens lagring förblir låst till loopens slut.
