# Standardbiblioteket

Biblioteket är skrivet i Dunder. [stdlib/modules.txt](../stdlib/modules.txt)
anger vilka moduler som följer med kompilatorn. Importer som
`import std.map as map` laddar inbäddad källkod utan installerade `.ddr`-filer
eller suffix i importen. Endast explicita beroenden läses. Delade importer
och alias har samma typidentitet. En lokal fil väljs med en uttrycklig
relativ sökväg, till exempel `import .std.map as local`.

För applikationer finns även [HTTP-server och TLS-klient](http.md) i
`std.http`/`std.http_client` samt [SQLite med bundna parametrar](sqlite.md)
i `std.sqlite`. Dessa moduler använder operativsystemets sockets och de
angivna systembiblioteken; de kräver ingen C-shim eller extern kompilator.

```dunder
import std.map as map
import std.set as set
import std.array as array

fn main() {
    mut counts := map.new()
    map.put(&mut counts, "dunder", 42)
    print(map.get_or(&counts, "dunder", 0))

    mut names := set.new()
    added := set.insert(&mut names, "å 🌍")
    assert(added)
    assert(set.contains(&names, "å 🌍"))

    mut values: []i64 = [3, 1, 2]
    values.push(4)
    array.sort(&mut values)
    print(array.sum(&values))
}
```

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
| `a.swap(i, j)` | Byter två element på plats utan kloning. Båda index kontrolleras före bytet; fel ger status 103. |
| `a.reserve(capacity)` | Säkerställer minst denna kapacitet utan att ändra längden. |
| `a.clear()` | Nollställer längden och behåller bufferten för återanvändning. |
| `for item in a` | Läslånar samlingen under loopen, utan arraykopia. |

En växande strängarray tar ägarskap om elementet. `a.push(move text)`
flyttar en befintlig strängägare. `a.push(text.clone())` behåller `text`
och ger arrayen en egen referens till samma oföränderliga text.
Färska värden som `a.push(f"rad {i}")` lämnas direkt.

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
| `map.get`, `map.get_or`, `map.contains`, `set.contains` | Lånar nyckeln. |
| `map.put(&mut table, move key, value)`, `set.insert(&mut set, move key)` | Tar över nyckeln; nyckeln förbrukas även om den redan finns. |
| `map.keys(table)`, `set.items(set)` | Returnerar en ny array; tabellen behåller sina strängägare. |
| `process.command(move program, move args)`, `add_arg`, `add_args`, `set_cwd`, `set_env`, `set_stdin` | Tar över lagrade strängar och buffertar. |
| `process.run(program, args)`, `process.exec(command)` | Lånar under den synkrona körningen. |
| `process.replace(program, &args)`, `process.replace_as(program, argv0, &args)` | Ersätter den aktuella processen utan en ny PID. Vid fel är miljö, arbetskatalog och standardströmmar orörda. `replace_as` väljer uttryckligen `argv[0]` för multicall-program. |
| `json.Doc::parse_bytes(move input)` | Dokumentet tar över bytebufferten utan kopia. |
| `json.Doc::parse_bytes(input.clone())` | Begär uttryckligen ett dokument med en självständig snapshot. |
| `json.Doc::parse_text(text)` | Lånar strängen och bygger dokumentets UTF-8-buffert. |
| `net.set_insert(&mut streams, move stream)` | Flyttar en stream till mängden. |

`Command::spawn` och `Command::exec` nollställer signalmasken i barnprocessen
före programstart, så blockerad signalpollning i föräldern inte gör kommandon
immuna mot SIGTERM eller SIGINT. Förälderns mask ändras inte.
`replace` och `replace_as` ersätter samma process och behåller dess signalmask.

`process.set_insert(&mut children, &mut child)` är ett fallibelt
överföringsförsök: framgång lämnar barnet i mängden och tömmer källhandtaget;
`-1` lämnar det oförändrat hos anroparen. Detta kräver ett skrivlån.
`ChildSet.try_insert(move child)` tar emot ägaren och returnerar
`Result<i32, Child>`. `.Ok` innehåller platsen i mängden. `.Err` ger
tillbaka barnprocessens handtag till anroparen.

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
deklarationsblockets slut. `&values[start..end]` ger en lånad `&[]T` och
`&text[start..end]` en UTF-8-kontrollerad bytevy, `&[]u8`, utan ny buffert.
Referenser kan ännu inte returneras eller lagras i structar/enums/arrayer;
lokala skrivlån stöds inte. `copy := values.clone()` skapar en ägd kopia av
det lånade värdet. Ett anrop får inte ha överlappande muterbara borrows av
samma rotvariabel; även olika fält i samma struct räknas konservativt som
samma rot. Ändringar som kan flytta en arraybuffert måste beräknas före
en elementtilldelning eller ett anrop som lånar elementet.

Generella livstidsannotationer återstår. Ägda allokeringar återvinns
automatiskt; se [minnesmodellen](memory.md#lokala-läslån-och-intervallvyer).

## Moduler

| Import | Innehåll |
| --- | --- |
| `std.array` | `ints`, `strings`, `sum`, `sort`, `reverse`, `append` och `sort_strings`. Båda sorteringarna använder heapsort på plats, O(n log n). Strängar jämförs lexikografiskt över UTF-8-byte. |
| `std.map` | `Map` från `string` till `i64` med metoder via `impl Map` (`Map::new`, `m.len`, `m.length`, `m.reserve`, `m.put`, `m.get`, `m.get_or`, `m.contains`, `m.remove`, `m.clear`, `m.keys`) samt modulhjälpare (`map.new`, `map.put`, ...). |
| `std.set` | `Set` av strängar med metoder via `impl Set` (`Set::new`, `s.len`, `s.length`, `s.reserve`, `s.insert`, `s.contains`, `s.remove`, `s.clear`, `s.items`) samt modulhjälpare (`set.new`, `set.insert`, ...). |
| `std.hash` | Stabil polynomial hash över UTF-8-byte, använd av map/set. |
| `std.text` | `starts_with`, `ends_with`, `find`, `split`, `trim`. |
| `std.bytes` | Packade bytebuffertar (`[]u8`), textappend, nollfyllning, little-/big-endian-kodning (`append_le`, `append_be`, `set_be`, `u16_le`, `u16_be`, `i16_be`, `u32_le`, `u32_be`, `i32_be`, `i64_le`, `i64_be`, `u64_le`, `u64_be`), `hex_u8` och kontrollerad UTF-8-avkodning. `append_text`, `utf8` och `hex` finns också för kompilatorns kvarvarande `[]i32`-buffertar. |
| `std.wire` | Binär serialisering till anroparens `[]u8` (`u8`, `bool`, fasta little-endian-tal, `f32`/`f64`, LEB128 `uvarint`, ZigZag `varint`, längdprefixade `bytes` och `str` samt `Reader::new` med `.read_*`/`.remaining`/`.finish`). |
| `std.json` | `Writer` för att skriva JSON samt lågnivåskrivare till `[]u8`. Läs dokument med `Doc::parse_bytes` eller `Doc::parse_text` och hämta fält med bland annat `.get_str`, `.get_i64`, `.get_u64`, `.get_f64` och `.get_bool`. |
| `std.sha256` | `digest(&bytes)` och `text(string)` ger 32 digestbyte. Implementerad i Dunder. |
| `std.parse` | `Integer::parse`/`Integer::hex`/`Integer::range`, `Unsigned::range`/`Unsigned::hex_range`, `Float32::parse`/`Float64::parse` och deras `range`-varianter läser ASCII-tal. `parse` accepterar understreck mellan siffror. |
| `sys` / `std.sys` | macOS-/Linux-filer, argument, byteutdata, UTF-8-avkodning, klocka, `sleep_ms`, process-ID, `Metadata::kind`, `IoError::from_code`/`.code`/`.message` samt `unsafe` Linux-primitiverna `envp` och `linux_syscall`; se [sys](sys.md). |
| `std.dl` | Dynamisk laddning av systembibliotek och uppslagning av symboler via Dunders typade [FFI](ffi.md). |
| `std.fs` | Delegerade läs-/skriv-/kataloghandtag med `impl` (`ReadFile::grant`/`.read`, `WriteFile::grant`/`.write`, `ReadDir::grant`/`.resolve_read`/`.list`) samt filsystemshjälpare (`cwd`, `realpath`, `mkdir_all`, `temp_dir`, `list_dir`, `remove_all`, `symlink`, `hard_link`, `is_symlink`). Se [filsystemets kapabiliteter](ffi.md). |
| `std.process` | Kapabilitetsstyrd processkörning (`capability process`): `Command::new` / `.exec` / `.spawn` / `.add_arg`, `Output::succeeded` / `.stdout_text`, `Executable::grant` / `.exec` / `.run` / `.spawn`, `Child` (`.wait`, `.try_wait`, `.read_stdout`, `.write_stdin`, `.terminate`, `.kill`), `ChildSet::new` samt modulfunktioner och miljö-/värdfrågor (`env`, `host_os`, `host_arch`, `host_target`, `host_cpus`). På Linux körs den via direkta systemanrop (`sys.linux_syscall`) och start-`envp` (`sys.envp`) helt utan `libc` eller `libdl` (statisk ELF). |

Map och set använder öppen adressering, linjär sondering och tombstones.
Tillväxt och omhashning är Dunder-kod. Uppdatering av en befintlig mapnyckel
ändrar inte antalet poster. `set.insert` returnerar sant för ett nytt värde;
`remove` returnerar sant när något togs bort. Iterationsordningen hos
`map.keys` och `set.items` är inte ett sorteringskontrakt.

`map.get` ger `Lookup { Missing, Found(i64) }`, så noll är ett vanligt värde.
`get_or` använder ett explicit reservvärde. Nycklar jämförs som UTF-8-byte,
inklusive tomma strängar och NUL; ingen Unicode-normalisering görs.
Representationsfälten är ännu synliga enligt språkets nuvarande structregler;
ändra samlingarna genom modulernas funktioner.

De konkreta typerna och modulhjälparna används också av kompilatorn.
Använd `Map<K, V>` och `Set<T>` för andra elementtyper.

`text.find` returnerar ett byteindex eller -1. `split` bevarar tomma delar;
en tom separator ger en del med hela texten. `trim` tar ASCII-mellanslag,
tabb, CR och LF. `parse.integer`, `parse.hex`, `parse.i64_range`, `parse.u64_range`
och `parse.hex_u64_range` ger fel 1 för stavning och 2 för overflow; både
i64- och u64-gränser stöds. `bytes` har även hjälpare för sökning och buffertar
(`decode_range`, `find_byte`, `find_crlf`, `find_double_crlf`, `equals_text`,
`equals_ascii_ci`, `extend_range`, `consume_prefix`, `append_decimal`).
`std.net` ger kapabilitetsstyrd TCP och `PollSet`/`StreamSet` ovanpå
`capability network` med både `impl`-metoder (`TcpListener::listen`/`.accept`/`.close`,
`TcpStream::connect`/`.read_append`/`.write_all`/`.close`, `PollSet::new`,
`StreamSet::new`) och modulhjälpare (`listen`, `connect`, `accept`, `poll`, icke-blockerande I/O).

## Generiska samlingar

`Map<K, V>` och `Set<T>` byggs med konkreta Dunder-typer och direkta
anrop för varje specialisering. De kan även skrivas `map.Map<K, V>`
och `set.Set<T>` efter import av standardmodulerna.

```dunder
fn main() {
    mut table := Map<string, []i32>::new()
    samples: []i32 = [21, 42]
    table.put("values", move samples)
    assert(table.at("values")[1] == 42)
    match table.take("values") {
        .Ok(values) => { assert(values.len == 2) },
        .Err => { assert(false) },
    }
}
```

Strängar, heltal, `bool` och `rune` kan användas direkt som nycklar.
Andra nyckeltyper definierar `pub fn key_hash(&self) -> u64` och
`pub fn key_eq(&self, other: Self) -> bool`. Likvärdiga nycklar måste
ha samma hash. Hash och likhet ska vara stabila medan nyckeln lagras.
Nycklar och värden använder samma lagringsregler som arrayelement.

`put` och `insert` tar över sina ägda argument. Vid en dubblett behåller
`put` den lagrade nyckeln och ersätter värdet. `insert` returnerar
`false` och släpper den inkommande nyckeln. Omhashning flyttar bara
index. `take` överför det lagrade värdet eller setelementet i `.Ok`
och ger `.Err` vid en saknad nyckel. `remove` och `clear` släpper
borttagna ägare via de vanliga cleanupreglerna. En `.clone()` av hela
samlingen kräver klonbara nycklar och värden.
`Set.take_at(index)` tar ut ett ägt element via dess index.

`Map.index_of(key)` ger postens index eller `-1`. `key_at(index)`
och `value_at(index)` lånar posten. `Map.at(key)` lånar värdet och
kräver att nyckeln finns. `Set.at(index)` lånar elementet vid indexet.
Skrivning till samlingen kräver att lånen har upphört. Använd intervallet
`0..len()` för att läsa poster i aktuell ordning. Borttagning kan byta
plats på den sista kvarvarande posten och den borttagna posten.

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
    encoded := writer.finish()
    assert(encoded.ok)
    print(encoded.value)
}
```

`Writer` sköter skiljetecken för objekt och arrayer. Anropa `key` före
värdet i ett objekt och `end` för varje öppnad behållare. `finish().ok`
kräver precis ett komplett rotvärde och en giltig följd av anrop.
`decimal`, `write_f64` och `field_f64` avvisar NaN och oändlighet.
`Doc.as_f64` och `Doc.get_f64` ger `ok: false` för saknade värden,
värden av annan JSON-typ och tal utanför det ändliga binary64-området.

## Bitoperationer

Heltal har explicita `.bit_and`, `.bit_or`, `.bit_xor`, `.bit_not`, `.shl`
och logisk `.shr`. Skiftantal är `i32` och måste ligga inom typens bitbredd;
annars ges status 103. `.wrapping_add`, `.wrapping_sub`, `.wrapping_mul`
väljer omslag uttryckligt; vanlig aritmetik behåller sina overflowkontroller.
Bytekodning och SHA-256 dogfoodar dessa operationer.

## Lagring och verifiering

Dynamiska buffertar använder en gemensam **512 MiB heap** på native, mappad vid första behov. Buffertarna har unika ägare; uttryckliga kloner
är oberoende. Tillväxt frigör den gamla bufferten efter överföringen.
Tilldelning, scope-slut och retur frigör övergivna buffertar, även i nästlade
structar och aktiva enumpayloads. `clear` frigör elementens strängreferenser
och behåller själva bufferten för nästa användning.
Full pool, misslyckad mappning eller för stor/negativ arrayreserv ger
status 109. Detta är ännu inget generellt allokerar- eller deallokerar-API.
Strängar har referensräkning och delar heap med arrayer.
Allokatorn återanvänder block per storleksklass. Läs [kostnader och gränser](memory.md).

```sh
dunder test tests/suite.ddr
python3 tools/test-runtime-contracts.py "$(command -v dunder)"
```

Tester jämför map, set och sortering mot Pythons referensresultat. De
provar även Unicode och NUL, olika elementbredder, djupa värdekopior,
callbacks och aliasfel.
SHA-256 kontrolleras mot referensvektorer och används av den
[Dunder-skrivna kompilatorns](../selfhost/README.md) Mach-O-signering.
