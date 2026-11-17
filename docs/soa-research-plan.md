# Forskning: SoA-härledning (`@derive(SoA)` / `@soa`) för structar i arrayer

Undersökningen gäller hur Dunder kan kombinera postbaserade samlingar
med kolumnlagring (Structure of Arrays), motiverat av
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

## Prototyp med variationstabellen som förebild

Skrifts `FvarTable.axis_at` bygger redan en `VariationAxis` från flera
kolumner. I `CmapTable` läser binärsökningen först en enda kolumn
(`f4_end_codes`) och hämtar de övriga fälten efter att segmentet hittats.
De två fallen ger skäl att exponera både poster och kolumnlån.

Följande test använder tre kolumner bakom ett postbaserat gränssnitt.
Spara exemplet som `axes.ddr` och kör `dunder run axes.ddr`.

```dunder
struct Axis { tag: i32, minimum: f32, maximum: f32 }
struct AxisTable { tags: []i32, minimum: []f32, maximum: []f32 }

impl AxisTable {
    fn new() -> Self { Self { tags: [], minimum: [], maximum: [] } }

    fn push(&mut self, axis: Axis) {
        capacity := self.tags.len + 1
        self.tags.reserve(capacity)
        self.minimum.reserve(capacity)
        self.maximum.reserve(capacity)
        self.tags.push(axis.tag)
        self.minimum.push(axis.minimum)
        self.maximum.push(axis.maximum)
    }

    fn get(&self, index: i32) -> Axis {
        Axis { tag: self.tags[index], minimum: self.minimum[index], maximum: self.maximum[index] }
    }

    fn min_values(&self) -> &[]f32 { &self.minimum[0..self.minimum.len] }

    fn remove(&mut self, index: i32) -> Axis {
        tag := self.tags.remove(index)
        lo := self.minimum.remove(index)
        hi := self.maximum.remove(index)
        Axis { tag: tag, minimum: lo, maximum: hi }
    }
}

fn main() {
    mut axes := AxisTable::new()
    axes.push(Axis { tag: 11, minimum: 100.0, maximum: 900.0 })
    axes.push(Axis { tag: 22, minimum: 50.0, maximum: 200.0 })
    axes.push(Axis { tag: 33, minimum: 0.0, maximum: 90.0 })
    {
        minima := axes.min_values()
        assert(minima.len == 3 && minima[1] == 50.0)
        assert(axes.get(1).tag == 22 && axes.get(1).maximum == 200.0)
    }
    removed := axes.remove(1)
    assert(removed.tag == 22 && removed.minimum == 50.0 && removed.maximum == 200.0)
    assert(axes.min_values().len == 2)
    assert(axes.get(1).tag == 33 && axes.get(1).minimum == 0.0 && axes.get(1).maximum == 90.0)
}
```

## Slutsats

Välj en uttrycklig följeslagartyp för den som vill använda kolumnlayout.
Den kan genereras i frontenden som vanlig Dunder med en typad array per
fält. `push`, `remove`, `set` och `swap` måste hålla kolumnernas längder
och index i takt. En operation som behöver mer utrymme reserverar samtliga
kolumner före den första längdändringen. Ett återlämnat kolumnlån låser
tabellen för skrivning under lånets livslängd.

`get(i)` materialiserar en post. Ett vanligt `&T` kräver en verklig
sammanhängande postplats och kan därför inte lånas direkt ur tabellen.
Låt första versionen hantera skalärfält. Ägda fält kan utökas med
statiska kloningskrav för `get` och ägaröverföring för `remove`.
CSR-packning av nästlade listor behöver ett eget kontrakt för index,
uppdateringar och ägarskap och kan utvärderas separat.

Behåll en separat tät postlayout för C- och GPU-gränssnitt.
Dagens `[]Struct` lagrar handtag till poster, även med `@c_layout` på
elementet. En framtida tät postbuffert hör ihop med
[POD-planen](pod-values-plan.md). Prototypen provar gränssnitt och
lånesemantik med befintliga arrayer. Kostnaden för helpostläsning,
kolumnsökning och verkliga arbetslaster får mätas innan en generell
härledning införs.
