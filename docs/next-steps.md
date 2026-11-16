# Nästa steg för Dunder

Föreslagen ordning efter ägarskapsgranskningen. Etapp 1 och 2a är
implementerade; etapp 2b och återstående delar av 3–5 är utvecklingsplanen.
Nuvarande kontrakt finns i
[minnesmodellen](memory.md), [standardbiblioteket](standard.md) och
[status](status.md).

## 1. Partiella flyttar och kontrollerad replace — implementerat

Initiering spåras nu per structfält, även för nästlade fältkedjor. Oberoende
fält kan flyttas separat och återinitieras genom tilldelning. Grenar och
loopkanter följer samma glesa platsträd; återstående runtimeägare städas
med befintlig rekursiv cleanup. `defer` behåller ett konservativt rotlås.

`replace(&mut place, replacement)` returnerar den tidigare ägaren utan att
lämna en oinitierad lucka. Den återanvänder typade plats- och ägaroperationer;
inga nya runtimehjälpare eller MIR-opkoder behövs. En generell `take` utan
uttrycklig ersättare har inte införts eftersom ett giltigt standardvärde
inte finns för alla typer.

Kompilatorns workerresultat överför modellfält separat och byter ut färdiga
kod- och metadataarenor mot tomma värden. Tabell i `dunder-hundmat` använder
separata flyttar för sitt ägda aggregeringsresultat. Verifieringen omfattar
de tre nativa målens runtimes, optimeringar på/av, felkontrakt och cleanup.
[Minneskontrakt och gränser](memory.md#fält-element-och-begränsningar) ·
[Arkitektur](arkitektur.md#initiering-per-lagringsplats) ·
[Riktad verifiering](../tools/test-partial-moves.py).

## 2a. Lokala läslån och lånade vyer — implementerat

`r := &owner.field` och `r := &values[index]` namnger oföränderliga läslån.
Typer kan också annoteras med `&T`. Lånen gäller till deklarationsblockets
slut, utan NLL. Oberoende syskonfält kan fortfarande flyttas och ändras;
arrayelement och intervall låser konservativt hela sin array. Motstridig
mutation, flytt och `replace` avvisas.

`&values[start..end]` ger `&[]T`; `&text[start..end]` ger `&[]u8` med
kontrollerade UTF-8-gränser. Vyerna använder originalets adress utan
heapallokering eller retain. `.clone()` materialiserar en ägd array.
Temporärer som `&f()` och `&[1, 2]` får lånas under anropet men inte bindas
lokalt. Lokala `&mut` och lokala lån av scratchlagring ingår inte.

Lånetabellen använder ägartillståndets kanoniska platsidentiteter och den
befintliga escapegrafen; den skapar ingen andra initieringsmodell.
HTTP-servern i `dunder-hundmat` använder namngivna path- och bodyvyer utan
offsetpar vid API-gränsen. Kompilatorns fragment-cache ska använda samma
mekanism för rena läsningar efter fröuppdateringen. Riktad verifiering
kontrollerar adressidentitet, allokeringsräknare, livstidsfel, intervall och
UTF-8 på de tre nativa målen, utan prestandamätningar.
[Kontrakt](memory.md#lokala-läslån-och-intervallvyer) ·
[Arkitektur](arkitektur.md#proveniens-och-lexikala-läslån) ·
[Verifieringsverktyg](../tools/test-local-borrows.py).

## 2b. Returnerade lån — planerat

Definiera ett tydligt samband mellan indata och returens livslängd innan
lån får lämna funktionen eller lagras i längre levande aggregat. Bygg sedan
JSON-, CSV-, typsnitts- och HTTP-API:er som kan returnera vyer utan nya
buffertar eller `(data, off, length)`-tripplar. Lokala skrivlån och lån av
scratchlagring kräver också uttryckliga kontrakt innan de utvidgas.

## 3. Bättre ägda samlingar — planerat

Utöka arrayelement till structar, payload-enums och andra arrayer,
med samma flytt-, klonings- och cleanupregler som fristående värden.
Det ersätter parallella fältarrayer i bland annat HTTP-servern och
kompilatorn. Lägg till ägd borttagning som flyttar ut ett element utan en
oinitierad lucka; `swap` följt av `pop` fungerar redan när ordningen får
ändras.

Bygg generiska `Map<K, V>` och `Set<T>` ovanpå denna grund. Välj en enkel
modell med statisk specialisering och synliga ägarskapskrav. Nuvarande
`Map` är `string → i64` och `Set` innehåller strängar.

> **Forskningspunkt (`SoA`)**: Utvärdera attributdriven SoA-härledning
> (t.ex. `@derive(SoA)` / `@soa`) som komplement till vanlig `[]Struct`, så att
> logiska poster kan hanteras samlat utan att ge upp cachevänlig kolumnlayout.
> Se [soa-research-plan.md](soa-research-plan.md).

## 4. Automatisk cleanup för externa resurser och typade fel — planerat

Låt `@resource`-typer ange cleanup som kompilatorn kör vid scope-slut,
retur, `break`, `continue` och felvägar. Flytt lämnar över ansvaret; lån
skapar inget nytt cleanupansvar. Definiera särskilt vad som får blockera
och hur explicit stängning samverkar med automatisk cleanup. Ett glömt
`close` eller en tidig retur ska inte läcka en socket.

Inför sedan generiska `Result<T, E>` och enkel felpropagering. Samla dagens
blandning av `.ok`/`.error`, enums och statuskoder utan att dölja resursägare
på felvägen. Ett fallibelt konsumtionsanrop måste kunna lämna tillbaka en
resurs som inte togs över.

## 5. Språkergonomi, testdiagnostik och standardbibliotek — återstående plan

Konkreta förbättringspunkter som bromsade iterationen vid utveckling och
integration av `dunder-skrift` och `dunder-hundmat/dogfood/skrift`:

- **Typsystem, inferens och syntax**:
  - Kontextuell heltalstypning i arrayliteraler (`[1, 2]` med `:=` härleds till
    `[]i64` och tvingar ofta `: []i32 = [...]`).
  - Flerradiga `.method()`-kedjor utan krav på yttre parenteser (`unused value`)
    samt stöd för flersatsiga block i `if ... { ... } else { ... }`-uttryck.
  - Tydligare `rune`-literaler (i dag är `'A'` en `string`, vilket krävde
    `rune(65)`), anonyma flervärdesreturer/tupler (för parsningshjälpare utan
    engångsstructar) och mindre `.clone()`-tvång vid `.push()` av strängar.
  - Kollisioner med reserverade ord som `len` i modul- och
    parameternamn samt enhetligare hantering av relativa importer (`"../..."` vs
    `..`).
- **Diagnostik, `check` och format**:
  - `assert(...)` visar nu källfil, radnummer och uttryck på nativa mål;
    visning av faktiska jämförelsevärden återstår. `dunder check` stöder
    biblioteksmoduler utan `fn main()`.
  - Formatangivelser i `f`-strängar (bl.a. hexutskrift) samt eventuella
    `{ path = "..." }` / `examples/`-tillägg i `dunder.toml` (`[dependencies]`
    och `std.bytes` big-endian-läsare är redan införda).
- **Standardbibliotek och grafik-dogfood**:
  - `std.parse`: flyttalsparsning (`float` / `f64`).
  - `std.math`: gemensam modul (`clamp`, `min`, `max`, trigonometri) så att
    dogfood-program inte behöver kopiera egna `math.ddr`.
  - `std.json`: flyttalsfält (`field_f64`) och en bekvämare `Writer`-abstraktion
    ovanpå lågnivåhjälparna (`write_str`, `field_i64`).
  - I `dunder-hundmat` (`graphics/macos.ddr`): texturstöd / alfamaskatlas (så att
    typsnitt inte behöver sändas som en quad per täckt pixel mot
    vertexgränsen `100000`) samt headless pixel-readback i paritet med Vulkan.

## Leveransregel

Behåll en gemensam regel: läsa lånar, lagra flyttar, duplicera begärs
uttryckligen. Varje etapp ska användas i kompilatorn och ett verkligt
Hundmat-program och verifieras på de tre nativa målen. Utöka syntaxen först
när en sådan konsument visar vad som faktiskt behövs.
