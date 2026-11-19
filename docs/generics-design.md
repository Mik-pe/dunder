# Generics i Dunders typsystem

En generisk deklaration beskriver en familj av funktioner eller typer. En
instans bestäms av **vilken deklaration** som används och **vilka typer**
som fyller dess parameterplatser. Samma regel gäller för egna deklarationer
och standardbibliotekets `Map`, `Set` och `Result`. Typargumenten är
explicita. [Syntax och exempel](syntax.md#generiska-deklarationer)

## Vad gör två instanser lika?

`Box<(i32, bool,)>` och `Box<(i32, bool)>` är samma typ om båda namnen
syftar på samma `Box`. Två moduler kan samtidigt deklarera varsin
`Box<T>` med likadana fält. Dessa deklarationer ger olika typer, även om
båda instansieras med `i32`.

Modulladdaren binder importalias, lokala namn och typparametrar i deras
lexikala omgivning. Därför är `Map < limit` en jämförelse när `Map` är
en lokal variabel. I en typannotation slås `Map<i32, string>` upp som typ.
Källtext som skickas direkt till kompilatorn använder samma bindning.
[Namnbindning](../selfhost/driver/modules.ddr)

Den ordinarie typparsern representerar exempelvis `Box<[]T>` som en
tillämpning av den bundna deklarationen på en array av en typparameter.
När argumentet för `T` blir känt får arrayen sitt vanliga typ-ID.
Kompilatorns instanstabell använder deklarationens ID och den ordnade
följden av argumentens typ-ID:n. För funktionstyper ingår bland annat
anropskonvention, parameterlägen och returlånets källparameter i identiteten.
[Typparser](../selfhost/frontend/type_syntax.ddr) · [Typupplösning](../selfhost/frontend/type_reader.ddr) · [Instanstabell](../selfhost/frontend/type_keys.ddr)

## När kontrolleras koden?

| Tidpunkt | Vad kompilatorn gör |
| --- | --- |
| Vid deklarationen | Binder namn, typparametrar och generiska tillämpningar i definitionsmodulen. Den ordinarie typkontrollen granskar slutna kroppar och självständiga delar av öppna kroppar. Ett okänt fritt funktionsnamn eller `true + 1` kan därmed fälla en oanvänd mall. |
| När en typ används | Reserverar typens identitet, kontrollerar dess deklarerade krav och fastställer fält, varianttyper och metodsignaturer. Därefter gäller typens vanliga regler för layout, lån och städning. |
| När en funktion eller metod används | Kontrollerar hela den konkreta kroppen med dess faktiska typer. En hänvisning till ett funktionsvärde räknas också som användning. Fel i en konkret kropp pekar på definitionen och visar vilka användningar som begärde instansen. |

Ett typkrav tillhör alla instanser av typen. Krav kan uttryckas med
namngivna traits (`where T: Trait1 + Trait2` eller `<T: Trait1 + Trait2>`),
inbyggda strukturella traits (`Sendable`, `Storable`, `Clone`, `Copy`) eller metodkrav (`T.size: fn(T) -> i32`).
Exempelvis kräver `Map<K, V>` `K: Hash + Storable` och `V: Storable`, och deklarerar villkorligt
`impl Sendable for Map<K, V> where K: Sendable, V: Sendable {}`.
Villkorliga traitimplementationer (`impl Trait for Box<T> where T: Trait`)
inklusive implementationer för sammansatta typer (`impl Eq for []T where T: Eq`,
`impl Ord for (A, B) where A: Ord, B: Ord`) och standardbibliotekets `impl Encode for []T`
löses till fixpunkt under deklarationskontrollen och aktiveras bara när
villkoren uppfylls, så exempelvis `Map<string, MyResource>` fortsätter fungera
i enkeltrådad kod utan att bli `Sendable`.
Traitmetoder med standardkropp syntetiseras per aktiv `impl` när metoden inte
ersätts, och generiska typer i trait-signaturer kan innehålla `Self`
(exempelvis `Result<Self, string>`). Sammansatta traitinstanser materialiseras
behovsstyrt från typerna i generiska argument, jämförelseoperatorer och metodanrop.
För en metod som läser `self.value.len` avgör fälttypen vid metodens
användning om operationen är giltig. Därför kan en annan metod på
`Box<i32>` användas även om `i32` saknar `len`.
Ett uttryckligt krav på en funktion eller ett `impl`-block kontrolleras
också när funktionen eller en metod i blocket väljs.
[Samlingarnas kontrakt](standard.md#generiska-samlingar) · [Traits](../tools/test_traits.ddr) · [Körbart exempel](../tests/fixtures/generics/main.ddr)

Standardbibliotekets generiska deklarationer och förladdade traits (`Ordering`,
`Eq`, `Ord`, `Hash`, `Display`, `Default`) samt deras implementationer för
primitiver, `Result<T, E>` och sammansatta typer bäddas in från Dunderkällan.
Operatorn `?` använder identiteten för det exporterade `Result` för att
hitta dess framgångs- och feltyp. En användardeklarerad enum kan ha
varianterna `Ok` och `Err` med vanlig enumsemantik.
[Prelude](../runtime/prelude/result.ddr) · [Traits-preludium](../runtime/prelude/traits.ddr) · [Inbäddning](../selfhost/embed.ddr)

## Från instans till körbar kod

Samordnaren reserverar varje ny instans innan den följer dess beroenden.
Rekursion med samma nyckel återanvänder instansen. Ett anrop med nya
typargument lägger till en instans i arbetskön. Metodkroppar tas in när
typkontrollen väljer dem. Den bundna källan och dess positioner återanvänds
mellan upptäcktsvarven. Därefter får arbetarna en fryst deklarationsbild.
Instanserna får vanlig maskinkod och vanliga städregler på native och Wasm.
[Specialisering](../selfhost/frontend/generics.ddr) · [Kompilering](../selfhost/frontend/compile.ddr)

Kompilatorn skiljer mellan rekursion med samma argument och en återkommande
deklaration som får nya argument för varje steg. En kedja av 70 olika
deklarationer och en cykel som roterar 67 typargument godtas. Vid upprepad tillväxt som `grow<T>` till
`grow<[]T>` avbryts specialiseringen med diagnostik. Kompilatorn har
även en övre gräns på 65 536 instanser per program.

Generics-sviten bygger för alla fyra mål och kör programmet på värden och
i Wasm. För vart och ett av de tre native-målen jämför den bilder från
seriella och parallella byggen samt kalla och varma inkrementella byggen
byte för byte. Den provar också ändrade mallkroppar med samma cache.
[Genericsproven](../tools/test_generics.ddr) · [Funktionscachen](incremental.md)
