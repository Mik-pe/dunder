# Forskning: SoA-härledning (`@derive(SoA)` / `@soa`) för structar i arrayer

Detta är ett forsknings- och designunderlag, inte implementerad funktionalitet.
Frågan gäller hur Dunder bäst kombinerar ergonomi för samlingar av sammansatta
poster med hårdvaruvänlig Structure-of-Arrays-lagring (SoA), motiverat av
[planen för samlingar](next-steps.md#samlingar) och erfarenheter från
`dunder-skrift` och `dunder-hundmat/dogfood/skrift`.

Vanliga `[]Struct`, payload-enums och nästlade arrayer stöds nu av
[frontend_types.ddr](../selfhost/frontend/frontend_types.ddr). Sammansatta
element ligger i åttabytesplatser som pekar på sina värden; kloning och
cleanup följer samma typade hjälpare som fristående värden. SoA är därför
ett möjligt layoutval att utvärdera, inte ett krav för att lagra poster.

## Var påtvingad SoA redan ger fördelar

- **Täta sök- och strömningsloopar**: Binärsökning över `cmap` Format 4 rör bara
  `f4_end_codes[mid]` utan att dra in kalla segmentfält i cachelinjen, och
  `Outline` (`xs`/`ys`/`verbs`) samt `BitmapAtlas` packar konturer och bitmappar
  i sammanhängande buffertar med minimala allokeringar.
- **Förutsägbar livslängd**: Inga dolda destruktorkaskader per element vid
  arraytillväxt.

## Var tidigare begränsningar har lett till extra kod

- **Manuell kolumnsynk**: Tabeller med 4–7 fält (`FvarTable`, `NameTable`,
  `CffTable`, `cli.Options` beroenden) måste hålla flera parallella `[]`-arrayer i
  takt vid `push`, `clear` och felretur. Sortering kräver indirekta
  permutationsarrayer (`TablePack::sorted_indices`) i stället för direkt
  elementbyte.
- **Tvånivås CSR-flattening**: Så fort en logisk post själv äger en array
  (`PrivateFont` med `subrs`, `RenderedText` med `pixels: []u8`) tvingas
  författaren bygga start/slut-tabeller och gemensamma datavektorer för hand.
- **API-ergonomi**: Publika gränssnitt får exponera indexmetoder
  (`axis_at(i)`) i stället för en typad samling.

## Designspår att undersöka

1. **Syntetiserad SoA-container via attribut (`@derive(SoA)` / `@soa struct`)**
   - Kompilatorns frontend genererar en följeslagartyp (t.ex. `AxisSoa` eller
     `soa[] Axis`) med en typad kolumn per fält samt `new`, `reserve`, `push(T)`,
     `pop`, `get(i) -> T`, `set(i, T)`, `swap(i, j)`, `clear` och `len`.
   - **Fördel**: Sänks rakt ned till befintliga `[]skalär` / `[]string`-arrayer
     utan ändringar i array-ABI eller backend, och tillåter fortfarande direkta
     kolumnlån (`&axes.min_value`) i heta kärnor.
   - **Öppna frågor**:
     - Hur ska nästlade ägda arrayfält (`pixels: []u8`) hanteras — förbjudas i
       första steget eller plattas automatiskt till CSR-kolumner (`data`,
       `starts`, `ends`)?
     - **Referenssemantik**: Eftersom ingen sammanhängande `T`-struct finns i
       minnet kan `&table[i]` inte ge en vanlig `&T`-referens; en explicit
       SoA-samlingstyp gör det layoutkontraktet tydligt.

2. **Förhållande till vanlig AoS `[]Struct` och POD**
   - Vanlig sammanhängande `[]Struct` (AoS) behövs fortfarande där extern
     C/GPU-FFI kräver en tät postbuffert (exempelvis `@c_layout struct Vertex`
     direkt till Metal/Vulkan utan manuell packning till `[]f32`).
   - En möjlig kombination är `@derive(SoA)` för interna tabeller och
     domändata, parallellt med vanlig värdestegning för `[]Struct` / POD
     ([pod-values-plan.md](pod-values-plan.md)).
