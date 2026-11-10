# Standardbiblioteket

Biblioteket är skrivet i Dunder och följer med kompilatorn. Importer som
`import std.map as map` laddar inbäddad källkod utan installerade `.ddr`-filer
eller suffix i importen. Endast explicita beroenden läses. Delade importer
och alias har samma typidentitet. En lokal fil väljs med en uttrycklig
relativ sökväg, till exempel `import .std.map as local`.

```dunder
import std.map as map
import std.set as set
import std.array as array

fn main() {
    var counts := map.new()
    map.put(&mut counts, "dunder", 42)
    print(map.get_or(&counts, "dunder", 0))

    var names := set.new()
    added := set.insert(&mut names, "å 🌍")
    assert(added)
    assert(set.contains(&names, "å 🌍"))

    var values: []i64 = [3, 1, 2]
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
| `a.push(value)` | Lägger till ett element; kapaciteten växer geometriskt. |
| `a.pop()` | Tar bort och returnerar sista elementet. Tom array ger status 103. |
| `a.reserve(capacity)` | Säkerställer minst denna kapacitet utan att ändra längden. |
| `a.clear()` | Nollställer längden och behåller bufferten för återanvändning. |
| `for item in a` | Itererar över en oberoende kopia av elementen vid loopstart. |

Arrayer har värdesemantik. `copy := values` ger en oberoende buffert, också
när arrayen ligger i en struct eller enum. Returer och vanliga värdeparametrar
kopieras. Reservkapacitet ingår inte i värdet och behöver inte bevaras vid
kopiering. Använd borrows för anrop som ska läsa eller uppdatera samma samling.

## Begränsade borrows

```dunder
fn append(values: &mut []i64, value: i64) {
    values.push(value)
}

fn first(values: &[]i64) -> i64 {
    values[0]
}
```

`append(&mut values, 42)` lånar muterbar lagring under anropet;
`first(&values)` lånar för läsning. Referensparametrar derefereras automatiskt
vid läsning, fältåtkomst och indexering. En läsreferens kan inte modifieras.
Oföränderliga bindningar kan inte lånas med `&mut`.

Detta första stöd begränsar referenser till funktionsparametrar och explicita
argument. De kan inte returneras, lagras i structar/enums/arrayer eller bindas
som lokala referenser. `copy := values` inne i en sådan funktion kopierar
det lånade värdet. Ett anrop får inte ha överlappande muterbara borrows av
samma rotvariabel; även olika fält i samma struct räknas konservativt som
samma rot. Ändringar som kan flytta en arraybuffert måste beräknas före
en elementtilldelning eller ett anrop som lånar elementet.

Det är en prövad, begränsad lånemodell. Generella livstider, ägare som kan
flyttas och återvinning av enskilda allokeringar återstår.

## Moduler

| Import | Innehåll |
| --- | --- |
| `std.array` | `ints`, `strings`; `sum`, `sort`, `reverse`, `append` för `[]i64`; `sort_strings` för `[]string`. Heltal använder insertion sort; strängar använder heapsort, O(n log n), på plats och utan extra array. Strängordningen är lexikografisk över UTF-8-byte, inte lokalanpassad. |
| `std.map` | `Map` från `string` till `i64`; `new`, `length`, `reserve`, `put`, `get`, `get_or`, `contains`, `remove`, `clear`, `keys`. |
| `std.set` | `Set` av strängar; `new`, `length`, `reserve`, `insert`, `contains`, `remove`, `clear`, `items`. |
| `std.hash` | Stabil polynomial hash över UTF-8-byte, använd av map/set. |
| `std.text` | `starts_with`, `ends_with`, `find`, `split`, `trim`. |
| `std.bytes` | Växande bytebuffertar som `[]i32`, textappend, nollfyllning, little-/big-endian-kodning, `hex`, kontrollerad UTF-8-avkodning. |
| `std.sha256` | `digest(&bytes)` och `text(string)` ger 32 digestbyte. Implementerad i Dunder. |
| `std.parse` | `integer(string)` ger `{ ok, value: i64, error }`; strikt ASCII-decimal med tecken och separatorer mellan siffror. |
| `sys` / `std.sys` | macOS-/Linux-filer, argument, byteutdata, UTF-8-avkodning och process-ID; se [sys](sys.md). |
| `macos` / `std.macos` | Ett AppKit-fönster, Metal-triangelista, händelser och Dunder-skrivna geometrihjälpare; se [macOS/Metal](macos-metal.md). |
| `std.vulkan` | Vulkan-triangelista på Linux, headless-rendering, RGBA-readback och SDL-fönster; se [Linux/Vulkan](linux-vulkan.md). |

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
tabb, CR och LF. `parse.integer` ger fel 1 för stavning och 2 för overflow;
både i64-minimum och i64-maximum stöds. `bytes.utf8` returnerar ett
`sys.TextResult`; ogiltig UTF-8 ger 92, ett värde utanför byteintervallet
ger 22 och otillräcklig strängarena ger 12.

Heltal har explicita `.bit_and`, `.bit_or`, `.bit_xor`, `.bit_not`, `.shl`
och logisk `.shr`. Skiftantal är `i32` och måste ligga inom typens bitbredd;
annars ges status 103. `.wrapping_add`, `.wrapping_sub`, `.wrapping_mul`
väljer omslag uttryckligt; vanlig aritmetik behåller sina overflowkontroller.
Bytekodning och SHA-256 dogfoodar dessa operationer.

## Lagring och verifiering

Dynamiska buffertar använder en separat **512 MiB processarena**, mappad vid första behov.
Geometrisk tillväxt och kopiering allokerar nya buffertar. Gamla buffertar
återvinns vid processlut; `clear` återanvänder den aktuella bufferten.
Full arena, misslyckad mappning eller för stor/negativ arrayreserv ger
status 109. Detta är ännu inget generellt allokerar- eller deallokerar-API.
Strängarna har fortsatt sin separata 16 MiB arena.

```sh
cargo run --release -- run dogfood/standard.ddr
# 4
# 6
# stdlib ok
cargo run --release -- test
# 49 testgrupper i det sammanslagna trädet
cargo test --test collections --test system --test selfhost
```

Tester omfattar 6 000 map-/setoperationer jämförda med Rusts oberoende
samlingar, kollisioner, borttagning, tombstones, Unicode/NUL, växande arrayer,
olika elementbredder, djupa värdekopior, callbacks och aliasfel. Ett verkligt
program över 1 MiB verifierar långa adresser och hopp till runtimefel.
SHA-256 kontrolleras mot referensvektorer och används av den
[Dunder-skrivna kompilatorns](../selfhost/README.md) Mach-O-signering.
