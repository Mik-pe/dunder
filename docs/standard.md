# Standardbiblioteket

Biblioteket är skrivet i Dunder. [stdlib/modules.txt](../stdlib/modules.txt)
anger vilka moduler som följer med kompilatorn. Importer som
`import std.map as map` laddar inbäddad källkod utan installerade `.ddr`-filer
eller suffix i importen. Endast explicita beroenden läses. Delade importer
och alias har samma typidentitet. En lokal fil väljs med en uttrycklig
relativ sökväg, till exempel `import .std.map as local`.

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
Element kan vara `i32`, `i64`, `f32`, `f64`, `bool`, `rune`, `string` eller
en enum utan payload. Annoterade literaler, även `[]`, får elementtypen
från kontexten. Nästlade arrayelement, structelement och data-enums som
element är ännu inte implementerade.

| Operation | Beteende |
| --- | --- |
| `a.len`, `a.len()`, `len(a)` | Antal initierade element som `i32`. |
| `a.cap()` | Tillgänglig kapacitet i element. |
| `a[i]`, `a[i] = value` | Typad åtkomst med gränskontroll; skrivning kräver muterbar lagring. |
| `a.push(value)` | Lägger till ett element; en lagrad strängägare lämnas som `a.push(move value)`. Kapaciteten växer geometriskt. |
| `a.pop()` | Tar bort och returnerar sista elementet. Tom array ger status 103. |
| `a.swap(i, j)` | Byter två element på plats utan kloning. Båda index kontrolleras före bytet; fel ger status 103. |
| `a.reserve(capacity)` | Säkerställer minst denna kapacitet utan att ändra längden. |
| `a.clear()` | Nollställer längden och behåller bufferten för återanvändning. |
| `for item in a` | Läslånar samlingen under loopen, utan arraykopia. |

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
| `json.parse_bytes(move input)` | Dokumentet tar över bytebufferten utan kopia. |
| `json.parse_bytes(input.clone())` | Begär uttryckligen ett dokument med en självständig snapshot. |
| `json.parse_text(text)` | Lånar strängen och bygger dokumentets UTF-8-buffert. |
| `net.set_insert(&mut streams, move stream)` | Flyttar en stream till mängden. |

`process.set_insert(&mut children, &mut child)` är ett fallibelt
överföringsförsök: framgång lämnar barnet i mängden och tömmer källhandtaget;
`-1` lämnar det oförändrat hos anroparen. Detta kräver ett skrivlån.

Sockets, socketmängder, barnprocesser och barnprocessmängder är
[`@resource`-typer](memory.md#unika-externa-resurser). De flyttas och kan
inte klonas, inte heller genom en omgivande struct eller enum. De använder
fortfarande explicita stängnings- och processoperationer.

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

Detta första stöd begränsar referenser till funktionsparametrar och explicita
argument. De kan inte returneras, lagras i structar/enums/arrayer eller bindas
som lokala referenser. `copy := values.clone()` skapar en ägd kopia av det lånade värdet. Ett anrop får inte ha överlappande muterbara borrows av
samma rotvariabel; även olika fält i samma struct räknas konservativt som
samma rot. Ändringar som kan flytta en arraybuffert måste beräknas före
en elementtilldelning eller ett anrop som lånar elementet.

Det är en prövad, begränsad lånemodell. Generella livstidsannotationer återstår. Ägda allokeringar återvinns automatiskt; se
[minnesmodellen](memory.md).

## Moduler

| Import | Innehåll |
| --- | --- |
| `std.array` | `ints`, `strings`; `sum`, `sort`, `reverse`, `append` för `[]i64`; `sort_strings` för `[]string`. Heltal använder insertion sort; strängar använder heapsort, O(n log n), på plats och utan extra array. Strängordningen är lexikografisk över UTF-8-byte, inte lokalanpassad. |
| `std.map` | `Map` från `string` till `i64`; `new`, `length`, `reserve`, `put`, `get`, `get_or`, `contains`, `remove`, `clear`, `keys`. |
| `std.set` | `Set` av strängar; `new`, `length`, `reserve`, `insert`, `contains`, `remove`, `clear`, `items`. |
| `std.hash` | Stabil polynomial hash över UTF-8-byte, använd av map/set. |
| `std.text` | `starts_with`, `ends_with`, `find`, `split`, `trim`. |
| `std.bytes` | Växande bytebuffertar (`[]u8` och `[]i32`), textappend, nollfyllning, little-/big-endian-kodning (`u16_le`, `u32_le`, `i64_le`, `u64_le`), `hex` / `hex_u8`, kontrollerad UTF-8-avkodning. |
| `std.wire` | Allokeringsfri binär serialisering till `[]u8` (`u8`, `bool`, fasta little-endian-tal, `f32`/`f64`, LEB128 `uvarint`, ZigZag `varint`, längdprefixade `bytes` och `str` samt `Reader`). |
| `std.json` | Strömmande JSON-skrivare direkt till `[]u8` samt platt span-tape RFC 8259-tolkare utan DOM- eller map-allokeringar (`parse_bytes`, `parse_text`, `field`, `at`, `get_str`, `get_i64`, `get_u64`, `get_bool`). |
| `std.sha256` | `digest(&bytes)` och `text(string)` ger 32 digestbyte. Implementerad i Dunder. |
| `std.parse` | `integer(string)` ger `{ ok, value: i64, error }`; strikt ASCII-decimal med tecken och separatorer mellan siffror. |
| `sys` / `std.sys` | macOS-/Linux-filer, argument, byteutdata, UTF-8-avkodning, klocka, `sleep_ms`, process-ID samt `unsafe` Linux-primitiverna `envp` och `linux_syscall`; se [sys](sys.md). |
| `std.dl` | Dynamisk laddning av systembibliotek och uppslagning av symboler via Dunders typade [FFI](ffi.md). |
| `std.fs` | Delegerade läs-/skriv-/kataloghandtag samt katalog- och filsystemshjälpare (`cwd`, `realpath`, `mkdir_all`, `temp_dir`, `list_dir`, `remove_all`, `symlink`, `hard_link`, `is_symlink`). Se [filsystemets kapabiliteter](ffi.md). |
| `std.process` | Kapabilitetsstyrd processkörning (`capability process`): `Command`, `run`, `run_in`, `status`, `exec`, `spawn` (`Child` och `ChildSet` för parallella processpooler med icke-blockerande `poll`/pipes, `read_stdout_line`, `terminate`, `kill`, `wait`), delegerade `Executable`-handtag (`grant`, `run_granted`) samt miljö- och värdfrågor (`env`, `host_os`, `host_arch`, `host_target`, `host_cpus`). På Linux körs den via direkta systemanrop (`sys.linux_syscall`) och start-`envp` (`sys.envp`) helt utan `libc` eller `libdl` (statisk ELF). |

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

Map och set har dessa konkreta elementtyper i första versionen. Generiska
`Map<K, V>`/`Set<T>` återstår. Dynamiska `[]T` typkontrolleras redan per
elementtyp utan vtables.

`text.find` returnerar ett byteindex eller -1. `split` bevarar tomma delar;
en tom separator ger en del med hela texten. `trim` tar ASCII-mellanslag,
tabb, CR och LF. `parse.integer`, `parse.hex`, `parse.i64_range`, `parse.u64_range`
och `parse.hex_u64_range` ger fel 1 för stavning och 2 för overflow; både
i64- och u64-gränser stöds. `bytes` innehåller även zero-copy-hjälpare
(`decode_range`, `find_byte`, `find_crlf`, `find_double_crlf`, `equals_text`,
`equals_ascii_ci`, `extend_range`, `consume_prefix`, `append_decimal`).
`std.net` ger kapabilitetsstyrd TCP och `PollSet`/`StreamSet` ovanpå
`capability network` (`listen`, `connect`, `accept`, `poll`, icke-blockerande I/O).

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
