# Forskning: SoA-härledning, samlingar och ergonomi

Detta är ett forsknings- och anteckningsunderlag, inte implementerad funktionalitet.
Punkterna nedan kommer direkt ur erfarenheter från att skriva, testa och
integrera `dunder-skrift` (ren Dunder-typsnittsmotor), `dunder-hundmat/dogfood/skrift`
(Metal-rendering) och `dunder.toml`-beroenden.

## 1. Forskningspunkt: `SoA` (`@derive(SoA)` / `@soa`) för structar i arrayer

Bakgrunden finns i [next-steps.md §3](next-steps.md#3-bättre-ägda-samlingar) och i
[frontend_types.ddr](../selfhost/frontend/frontend_types.ddr), där `[]Struct` i dag
avvisas eftersom arraykörtiden och MIR förutsätter skalära eller pekarstora
element (`arrays of aggregate elements are not supported by this compiler`).

### Var påtvingad SoA redan ger fördelar

- **Täta sök- och strömningsloopar**: Binärsökning över `cmap` Format 4 rör bara
  `f4_end_codes[mid]` utan att dra in kalla segmentfält i cachelinjen, och
  `Outline` (`xs`/`ys`/`verbs`) samt `BitmapAtlas` packar konturer och bitmappar
  i sammanhängande buffertar med minimala allokeringar.
- **Förutsägbar livslängd**: Inga dolda destruktorkaskader per element vid
  arraytillväxt.

### Var avsaknaden av `[]Struct` eller SoA-stöd kostar kod och tid

- **Manuell kolumnsynk**: Tabeller med 4–7 fält (`FvarTable`, `NameTable`,
  `CffTable`, `cli.Options` beroenden) måste hålla flera parallella `[]`-arrayer i
  takt vid `push`, `clear` och felretur. Sortering kräver indirekta
  permutationsarrayer (`TablePack::sorted_indices`) i stället för direkt
  elementbyte.
- **Tvånivås CSR-flattening**: Så fort en logisk post själv äger en array
  (`PrivateFont` med `subrs`, `RenderedText` med `pixels: []u8`) tvingas
  författaren bygga start/slut-tabeller och gemensamma datavektorer för hand.
- **API-ergonomi**: Publika gränssnitt får returnera parallella indexmetoder
  (`axis_at(i)`) i stället för `[]VariationAxis` eller en typad samling.

### Två spår att undersöka vidare

1. **Syntetiserad SoA-container via attribut (`@derive(SoA)` / `@soa struct`)**
   - Kompilatorns frontend genererar en följeslagartyp (t.ex. `AxisSoa` eller
     `soa[] Axis`) med en typad kolumn per fält samt `new`, `reserve`, `push(T)`,
     `pop`, `get(i) -> T`, `set(i, T)`, `swap(i, j)`, `clear` och `len`.
   - **Fördel**: Sänks rakt ned till befintliga `[]skalär` / `[]string`-arrayer
     utan ändringar i array-ABI eller backend, och tillåter fortfarande direkta
     kolumnlån (`&axes.min_value`) i heta kärnor.
   - **Öppen fråga**: Hur hanteras nästlade ägda arrayfält (`pixels: []u8`) —
     förbjuds initialt i `@derive(SoA)` eller plattas automatiskt till CSR-kolumner
     (`data`, `starts`, `ends`)?
   - **Referenssemantik**: Eftersom ingen sammanhängande `T`-struct finns i minnet
     kan `&table[i]` inte ge en vanlig `&T`-referens; en explicit SoA-samlingstyp
     gör det kontraktet tydligt för användaren.

2. **Transparent `[]Struct` (AoS) där sammanhängande minne behövs**
   - Behövs fortsatt när extern C/GPU-FFI kräver en tät AoS-buffert (exempelvis
     `@c_layout struct Vertex` direkt till Metal/Vulkan utan manuell packning
     till `[]f32`).
   - En möjlig kombination är `@derive(SoA)` för interna tabeller/domändata och
     nativ värdestegning för `[]Struct` (särskilt `@c_layout` / POD från
     [pod-values-plan.md](pod-values-plan.md)).

## 2. Övriga språkliga och ergonomiska punkter från dogfood

### Typkontroll, ägarskap och syntax

- **Lån av temporärer (`&expr`) och vyer (`&[]u8`)**: `borrow a named variable,
  field or array element` kräver namngivna lokaler före `&`-anrop (`a := [1, 2];
  f(&a)`) eller extra `*_owned`-metoder som `TablePack::add_owned` och
  `BitmapAtlas::add_text`. Binärparsning måste skicka
  `(data: &[]u8, off: i32, length: i32)` i brist på lånade vyer (se
  [next-steps.md §2](next-steps.md#2-lokala-lån-och-lånade-vyer)).
- **Arrayliteral-inferens och `.push()`-ägarskap**: `[1, 2]` med `:=` härleds
  till `[]i64`, vilket ofta kräver `: []i32 = [...]` före `&[]i32`-anrop. Samtidigt
  kräver `.push(tag)` på icke-`move`-parametrar uttrycklig `.clone()` (`push requires
  'move value', .clone(), or a fresh owned expression`), vilket ger extra
  kloningsbrus i byggare och strängtabeller.
- **Uttrycksblock och flerradiga metodkedjor**: `if ... { ... } else { ... }` i
  uttrycksposition accepterar bara ett enda uttryck per gren (flersatsiga block
  avvisas och tvingar fram `mut`-initiering), och flerradiga `.method()`-kedjor
  misslyckas med `unused value` utan yttre parenteser.
- **Rune-literaler, tupler och formatangivelser**: `'A'` tolkas som `string` i
  stället för `rune` (tvingade `rune(65)` i `dunder-skrift`), flervärdesreturer
  som `(bool, f64)` stöds inte utan engångsstructar (`ParsedDecimal`), och
  `f`-strängar saknar formatangivelser som hex (krävde manuell `hex_cp`-loop för
  `U+XXXX`).
- **Reserverade identifierare och modulsökvägar**: Nyckelorden `var` och `len`
  kolliderar med vanliga modul- och parameternamn (krävde `import var as variation`
  eller `"../src/var"` samt omdöpning till `length`), och relativa citerade
  importer (`"../src/..."`) skiljer sig syntaktiskt från punktnotation (`..src`).

### Test-, check-, paket- och verktygsflöde

- **Rikare `assert`-diagnostik i `dunder test`**: Misslyckade påståenden visar i
  dag bara `FAIL ... (exit 104)` utan källfil, radnummer eller jämförda värden.
- **`dunder check` för bibliotek**: Biblioteksmoduler utan `fn main()` kan i dag
  inte typkontrolleras enbart med `dunder check <modul.ddr>`; snabb
  typkontroll under biblioteksarbete fick därför gå via `dunder test`.
- **Paketmanifest (`[dependencies]`)**: Nuvarande `dunder.toml` stöder lokala
  `namn = "sökväg"`-par (bindestreck i katalognamn hanteras via aliaset, t.ex.
  `skrift = "../dunder-skrift"`). Eventuellt inline-tabellsstöd
  (`{ path = "..." }`) och sammanställning av exempel under `examples/`
  återstår om paketsystemet byggs ut.

### Standardbibliotek (`stdlib/`)

*(Notera: `std.bytes` big-endian-hjälpare och lokala `[dependencies]` i
`dunder.toml` har redan införts i språket/verktygskedjan.)*

- **`std.parse`**: Lägga till `float` / `f64`-parsning (behövdes för
  `--axis opsz=14.5`).
- **`std.math`**: Gemensamma numeriska funktioner (`clamp`, `min`, `max`,
  trigonometri) i stället för lokala kopior per projekt i `dunder-hundmat`.
- **`std.json`**: Lågnivåfunktioner som `write_str` och `field_i64` saknar
  motsvarande flyttalsfält (`field_f64`) och en högnivå-`Writer`.

### Grafik- och Metal-gränssnitt (`dunder-hundmat`)

- **Textur- och atlasstöd i Metal-dogfood**: Nuvarande `macos.ddr` stöder bara
  otexturerade trianglar (`max 100000 vertices`), vilket tvingar fram en 6-vertex
  quad per täckt typsnittspixel (`13 643` kvadrater / `81 858` av `100 000`
  vertex för en skärm text). Texturerade kvadrater (`texture_2d` /
  alfamaskatlas) är nästa naturliga steg.
- **Headless pixel-readback i `macos.ddr`**: Motsvarighet till Vulkan-modulens
  `open_headless` + `pixels()` förenklar automatisk visuell verifiering på macOS.
