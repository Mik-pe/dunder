# Nästa steg för Dunder

Föreslagen ordning efter ägarskapsgranskningen. Etapp 1 är implementerad;
etapperna 2–5 är fortsatt en utvecklingsplan. Nuvarande kontrakt finns i
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

## 2. Lokala lån och lånade vyer

Gör det möjligt att namnge ett läslån till ett fält eller arrayelement och
använda ett array-/textintervall som vy utan ny buffert. Börja med lån som
stannar i samma scope; utöka sedan till returer med ett tydligt samband
mellan indata och returens livslängd. Ägaren får inte flyttas, ersättas eller
muteras på ett motstridigt sätt medan lånet används.

Det tar bort behovet av tillfälliga strängägare för rena läsningar och gör
JSON-, CSV-, typsnitts- och HTTP-API:er möjliga utan materialiserade
mellanbuffertar eller `(data, off, length)`-triplar. Utvidga även lån till att
omfatta temporära uttryck vid anrop (`&f()`, `&[1, 2]`) så att anropare slipper
namnge engångsvariabler eller skriva dubbla `*_owned`-metoder. Verifiera både
avvisade livstidsfel och att vyerna behåller originalets adress utan
allokering. Begränsade lån är redan möjliga under anrop; den här etappen
utvidgar samma regel.

## 3. Bättre ägda samlingar

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

## 4. Automatisk cleanup för externa resurser och typade fel

Låt `@resource`-typer ange cleanup som kompilatorn kör vid scope-slut,
retur, `break`, `continue` och felvägar. Flytt lämnar över ansvaret; lån
skapar inget nytt cleanupansvar. Definiera särskilt vad som får blockera
och hur explicit stängning samverkar med automatisk cleanup. Ett glömt
`close` eller en tidig retur ska inte läcka en socket.

Inför sedan generiska `Result<T, E>` och enkel felpropagering. Samla dagens
blandning av `.ok`/`.error`, enums och statuskoder utan att dölja resursägare
på felvägen. Ett fallibelt konsumtionsanrop måste kunna lämna tillbaka en
resurs som inte togs över.

## 5. Språkergonomi, testdiagnostik och standardbibliotek

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
