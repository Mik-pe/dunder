# Minne och ägarskap

Dunder frigör strängar, arraybuffertar och sammansatta värden under körning.
Kompilatorn lägger in städning vid tilldelning, blockslut, retur, `break`
och `continue`. Den genererade koden och minnespoolen följer med varje program
på ARM64/macOS, ARM64/Linux, x86-64/Linux, x86-64/Windows samt Wasm32
(`wasm32` och `wasm32-wasi`).


I ett program byggt med `-g` anger standardfel källfil, rad, kolumn och
operation vid heltalsspill (status 101) och index utanför en array eller vy
(status 103). Positionen avser operatorn eller den inledande hakparentesen,
även när felet uppstår i en importerad modul.

## Ägare, lån och kloning

En bindning är oföränderlig utan `mut`. Muterbarhet anger vilka ändringar
som är tillåtna; ägarskap anger vem som ska frigöra värdet.

| Användning | Regel |
| --- | --- |
| `b := a`, tilldelning, retur och konstruktion | Flyttar resursägande värden till mottagaren. |
| Vanlig funktionsparameter | Lånar strängar och aggregat för läsning under anropet. |
| `r := &place` | Namnger ett oföränderligt läslån fram till dess sista lexikala läsning i blocket (eller till blockslut vid `defer`/inre loopar). |
| `&mut value` vid anrop | Exklusivt skrivlån; kräver muterbar lagring. |
| `move`-parameter och `consume(move value)` | Funktionen tar över ägarskapet. |
| `value.clone()` | Skapar en ytterligare ägare uttryckligen. |
| Vanlig `for value in values` | Läslån av samlingen under loopen. |

Regeln för flytt gäller `string`, dynamiska och fasta arrayer, iteratorer och
structar/enums som innehåller sådana värden, inklusive map och set. Tal,
bool, runes, pekare, funktionsvärden och records utan resursfält eller `@resource`
kan kopieras som enkla värden. Typens ägarskap bestämmer regeln, oberoende
av om kompilatorn lagrar dess representation på stack eller heap.

### Unika externa resurser

`@resource` på en struct anger att värdet äger en unik extern resurs,
även när representationen bara består av heltal eller pekare. Värdet och
aggregat som innehåller det flyttas; `.clone()` och replikering av en
sådan resurs avvisas. Vanliga parametrar lånar fortfarande för läsning.
Markeringen är skild från `opaque`, som styr åtkomst till representationen.

```dunder
@resource
struct Handle { id: i32 }

fn forward(move handle: Handle) -> Handle { handle }

fn main() {
    original := Handle { id: 42 }
    next := forward(move original)
    print(next.id)
}
```

Standardbibliotekets `TcpListener`, `TcpStream`, `StreamSet`, `UdpSocket`, `File`,
`sqlite.Database`, kanaler (`Sender`/`Receiver`), `Child`,
`ChildSet`, `ChildStdin`, `ChildStdout`, `ChildStderr`, `FileReader`,
`FileWriter` samt allokeringsbehållarna `Bump`, `OwnedBuf`, `Arena` och
`FrozenArena` är sådana resurser. Handtag som `TcpListener`, `TcpStream`,
`UdpSocket`, `File`, `sqlite.Database`, `Sender<T>` och `Receiver<T>` implementerar
dessutom `Sendable` med strikt `move`-överföring mellan trådar (ingen kloning;
sändarens ägarslot nollställs), och `task scope`-arbetsytor kan även äga
`@resource(dispose)`-fält som stängs på ägarens tråd när scopet avslutas. Fil- och körningskapabiliteter som bara
innehåller en sökväg kan däremot uttryckligen klonas.
Lokala resurser i en worker följer scopets beviljade kapabiliteter och effektregler. Transport över en
isolerad taskgräns kräver `Sendable`-värden; frysta ögonblicksbilder kan även delas låsfritt via `std.sync.Published<T>` och `ReadGuard<T>`. Resurstyper använder Dunders
vanliga structlayout och lagras som ägda värden.

### Automatisk stängning

Med `@resource(dispose)` anropar kompilatorn den privata instansmetoden
`fn dispose(&self)` eller `fn dispose(&mut self)` när det sista ägda värdet
släpps. Metoden måste vara säker och returnera `()`. Den körs synkront före
frigöring av structens fält och kan därför blockera om stängningsoperationen
blockerar. Samma regel gäller vid blockslut, `return` (även av ett felvärde),
`break`, `continue`, ersättning av ett värde och när ägaren finns i ett aggregat.
Muterbar `dispose(&mut self)` gör att resurser som `FileReader`, `Bump`,
`OwnedBuf`, `Arena` och `FrozenArena` även kan dela på en explicit `.close()`
eller `.release()` och lämna värdet tomt utan dubbel frigöring.
En runtimefälla som avslutar hela processen går direkt till processavslut.

Flytt av hela resursen överför stängningsansvaret. Använd `replace` för
att ta över ett ägt fält och lämna ett giltigt värde åt stängningsmetoden.
En separat explicit `close(&mut self)` kan rapportera operationens
resultat. Låt den markera resursen stängd när ansvaret försvinner och låt
`dispose` kontrollera markeringen. `TcpListener`, `TcpStream`,
`StreamSet` och `UdpSocket` använder en negativ filbeskrivare som markering.
`@resource` utan metodnamn ger unikt ägarskap med explicit stängning.

```dunder
fn total(values: []i32) -> i32 {
    mut sum := 0
    for value in values { sum += value }
    sum
}

fn retain(move values: []i32) -> []i32 { values }

fn main() {
    mut values: []i32 = [1, 2, 3]
    values.reserve(32)
    print(total(values))
    print(total(values))
    independent := values.clone()
    saved := retain(move values)
    print(total(saved))
    print(total(independent))
}
```

Användning efter flytt ger kompileringsfel. En `mut`-lokal kan få ett nytt
värde efter flytt. Efter grenar måste värdet finnas på varje väg som når
användningen; en loop måste återställa värden som behövs i nästa varv.
Ett lånat resursvärde får inte flyttas till en ägd bindning, retur eller container.
Välj en `move`-parameter när funktionen ska behålla indata, eller klona när
det ska finnas två oberoende ägare. Läslån kan namnges lokalt enligt reglerna
nedan. Funktioner kan även lämna tillbaka ett läslån från en parameter.

Flytt av en dynamisk array behåller buffert och kapacitet. `.clone()` ger en
oberoende buffert och klonar ägda barn rekursivt; klonens reservkapacitet
behöver inte matcha originalets. Strängar är oföränderliga: deras uttryckliga
kloner delar UTF-8-data genom referensräkning. Kloning av en sträng behöver
därför ingen teckenkopia. Literaler ligger i programbilden.

Tilldelning utvärderar det nya värdet innan det gamla släpps. Varje ägd
resurs frigörs en gång när dess ägare lämnar sitt scope. `pop` tar över det
borttagna elementet. `clear` släpper elementen och behåller kapaciteten.

Element kan vara structar, enums med payload eller andra fasta och
dynamiska arrayer. Dessa värden använder samma klonings- och cleanupregler
i en samling som utanför den. Ändliga rekursiva värden, exempelvis
`Node { children: []Node }`, kan klonas. Om ett nåbart barn är en
`@resource` avvisas kloning och upprepade arrayliteraler statiskt.
`for element in &values` lånar ägda element; `for element in move values`
flyttar dem. Referenser och scratchvyer får inte lagras som arrayelement.
Vanliga arrayer av C-layout-structar stöds, medan fasta inline-arrayer
inne i en C-layout-struct fortfarande kräver skalära element.

Giltiga dynamiska arrayer har `0 <= len <= cap <= i32::MAX`, oberoende av
minnespoolens storlek. Runtime kontrollerar önskat antal före växande
allokering och begränsar dubblerad kapacitet till denna gräns. När en
giltig befintlig kapacitet räcker bevisar den också att antalet ryms.
Negativa eller för stora antal når samma kapacitetsfälla, kod 109.

Direkt iteration med `for byte in text.bytes()` eller
`for scalar in text.runes()` kan strömma från en behållen strängsnapshot
utan att skapa konverteringens mellanbuffert. Källan utvärderas en gång;
att ersätta strängen under loopen ändrar inte värdena som återstår.
En lagrad konvertering är fortfarande en självständig array. Dolda
mellanbuffertars allokering och resursåtgång är implementationsdetaljer;
uttryckliga kapacitetskontroller och fel för index eller UTF-8 behålls.

Temporära resursägare överförs direkt till mottagaren. Skriv `move value`
när ett redan lagrat värde lämnas till en ägande funktionsparameter.
En ny ägare som skapas av ett anrop eller en literal kan lämnas direkt.
Optimeringsflaggor ändrar inte reglerna för lån, flytt eller frigöring.

Samma regel gäller icke-adresserbara uttryckstemporärers privata
implementationslagring. Kompilatorn får hålla deras värden i register;
resursfel för stacklagring som då aldrig skapas är inte garanterade.
Detta tillåter inte att ta bort adresserbara källobjekt, uttryckliga
allokeringar eller anropsargument. Deras giltiga adresser,
utvärderingsordning och uttryckliga kontroller
behålls. Rå maskinkods stackåtkomster får ingen sådan garanti av sitt
instruktionsmönster. Okänd stacktrafik, SP-observationer och anrop kräver
egna bevis innan deras fysiska stackförlopp får ändras.

I `text + change(&mut text)` behåller vänsterledet strängen som lästes före
funktionsanropet. Samma utvärderingsordning gäller för argument, index och
snittgränser. En retur bevarar sitt resultat medan funktionen städar sina
lokaler. Ett fatalt runtimefel avslutar processen eller avbryter Wasm-anropet.

### Lokala läslån och intervallvyer

`r := &owner.field` och `r := &values[index]` skapar oföränderliga lokala
referenser. Typen kan härledas eller anges, exempelvis `r: &i32 = &values[0]`.
Ett lokalt läslån inom samma block och loopdjup frisätts automatiskt efter sin
sista lexikala användning innan en senare skrivning, `replace` eller flytt av
ägaren. Om referensen fortfarande kan läsas senare i blocket, fångas av ett
`defer`-block eller deklareras utanför en loop där ägaren ändras (inre
grenslingor/loopar eller senare läsningar) ligger lånet kvar till blockslut;
ett inre `{ ... }`-block eller sista läsningen frigör det tidigare.

```dunder
fn main() {
    mut values: []i32 = [7, 20, 22, 9]
    {
        first := &values[1]
        middle: &[]i32 = &values[1..3]
        assert(first == 20 && middle.len == 2 && middle[1] == 22)
        independent := middle.clone()
        assert(independent[0] == 20)
    }
    values[1] = 21
    text := "xå🌍y"
    utf8: &str = &text[1..7]
    assert(utf8.len == 6 && utf8 == "å🌍" && utf8[0] == 195)
}
```

`&values[start..end]` har typen `&[]T` för arrayens elementtyp. Det fungerar
för fasta och dynamiska arrayer och för delintervall av en befintlig vy.
`&text[start..end]` och `text.as_str()` ger en scope-bunden lånad UTF-8-strängvy
av typen `&str` utan livstidsannotationer och utan ny strängallokering (med
`.as_bytes()` för att se motsvarande `&[]u8`).
Båda gränserna utvärderas en gång i källordning. Intervallet måste uppfylla
`0 <= start <= end <= len`, annars blir runtime-status 103. För text krävs
dessutom UTF-8-gränser, även för ett tomt intervall; ett brott ger 106 efter
att intervallgränserna kontrollerats.

Vyn pekar på originalets element, skapar ingen heapbuffert och behåller
ingen ägare genom referensräkning. `len` och `cap()` för `&[]T` ger vyns
längd, medan `&str.len` ger antalet UTF-8-byte. Varken skrivning genom vyn,
storleksändring eller `&mut str` är tillåten, och `&str` kan inte lagras i
structar, enums eller arrayer. En uttrycklig `.clone()` (eller `.str()`/`.to_string()`)
på en `&str` skapar en ägd `string`, medan `.clone()` på `&[]T` ger `[]T`. Vyns livslängd skyddas i stället av lånet.

Ett aktivt lån av ett structfält tillåter ändring och flytt av oberoende
syskonfält. Flytt, `replace`, skrivning eller ett skrivlån som överlappar
den lånade platsen avvisas, inklusive ändring av hela roten. Ett lån av
ett arrayelement eller delintervall låser konservativt hela arrayen.
Återlån följer den ursprungliga ägaren, även genom flera lokala alias.

Temporära uttryck kan lånas under ett anrop, exempelvis `read(&f())` eller
`sum(&[1, 2])`. Ägaren hålls då levande genom argumentutvärderingen och
anropet. `r := &f()` och `r := &[1, 2]` avvisas: en lokal referens förlänger
inte en temporärs livslängd. Samma regel gäller för resultatet av ett anrop
som returnerar ett lån från temporären.

Nya lokala lån får inte lämnas till en funktion som kan exponera deras
lagring som råpekare, inte heller genom indirekta anrop. Explicit `unsafe`
kan observera adressen direkt, exempelvis med `addr(&view[0])` i ett
identitetsprov; råpekaren följer då kontraktet för `unsafe`.

### Fält, element och begränsningar

Ett ägt managed-fält kan flyttas ut ur en lokal eller ett färskt returvärde
utan datakopia. Oberoende fält, även nästlade, kan flyttas separat. Efter en
fältflytt får syskonfält fortfarande läsas och ändras; läsning, lån eller
flytt av hela föräldern kräver att alla dess fält är initierade. Tilldelning
till ett muterbart fält återinitierar det fältets subträd; tilldelning till
hela bindningen återinitierar allt. Grenar och loopkanter följer samma
fältplatser. Kvarvarande fält frigörs exakt en gång vid cleanup.
Flytt ur ett lån avvisas.
Arrayelement kan flyttas ut ur en dynamisk array med `pop` eller
`remove(index)`. `remove` överför ägarskapet till returvärdet och flyttar
efterföljande element åt vänster inom samma buffert. Kvarvarande element
behåller sin ordning och sina ägare. `.clone()` skapar en uttrycklig kopia
av ett element eller ett inline-fält i C-layout.
`values.swap(i, j)` byter element på plats utan kloning eller referensräkning.

`replace(&mut place, replacement)` installerar ett nytt värde och returnerar
den tidigare ägaren. Destinationen måste vara initierad och muterbar och kan
vara en lokal, ett fält, ett arrayelement eller lagring bakom en `&mut`-parameter.
Ett redan lagrat resursägande ersättningsvärde kräver `move value`; ett färskt
uttryck eller en uttrycklig klon kan lämnas direkt. Destinationen och dess
index utvärderas en gång före ersättningsuttrycket. Det senare får inte
flytta eller mutera destinationens rot. Båda operanderna är färdigutvärderade
innan själva bytet, och inget anrop ser en oinitierad lucka.

För managed-värden överförs befintliga buffertar, adresser och kapaciteter.
Inline C-lagring och fasta POD-arraylån behöver i stället en självständig
bytekopia av returvärdet. Scratchlagring stöds inte av `replace`.
Det finns ingen generell `take` med implicit standardvärde: en ersättare
måste vara ett uttryckligen valt giltigt värde, även för `@resource`.

Vanlig `for` och `match` lånar. Ett aktivt lån under `for x in record.field`
eller `match record.enum_field` låser enbart det lånade fältets sökväg, så
oberoende syskonfält kan fortfarande ändras eller växa med `.push(...)`.
`for value in move values` respektive `match move value` överför ägaren direkt.
Loopbindningen i `for value in move values` (samt vid iteration över en ägd
temporär array, `for value in make_values()`) och en färsk fältprojektion
från ett temporärt uttryck (`make_pair().left`) känns igen som färska ägda
värden vid `push`, `replace` och ägande parametrar utan att extra `move`
behövs; lagrade lokaler och fält på namngivna ägare kräver fortfarande
`move value` (eller `.clone()` för klonbara värden).
`for value in values.iter()` lånar också; en lagrad ägande iterator skapas
från `(move values).iter()` eller `values.clone().iter()`.

Även vanlig `match värde` (eller `match s.fält`) över en namngiven lokal eller
ett ägt structfält tillåter att en arm flyttar ut en ägd eller `@resource`-nyttolast
genom dess mönsterbindning (exempelvis `failure = e` i `.Err(e)`, `taken := c`
eller `out.push(move c)` i `.Ok(c)`), på samma sätt som en partiell fältflytt:
- Själva flytten förbrukar motsvarande ägarskapssökväg från den matchade roten
  längs den armen, medan armar som bara läser eller divergerar (`return`,
  `break`, `continue`) lämnar ämnet orört.
- Gräns mot fältflyttar och lånade källor: en flytt ur `match` kräver ett ägt,
  oindexerat ämne (inte en parameter/referens `fn f(r: Result<...>)`,
  `match &r`, `match arr[i]` eller fält inuti en `@resource(dispose)`-struct
  med automatisk stängning).
- En `match`-guard får aldrig flytta ut bindningar (`if take(move v)` avvisas);
  efter att guarden godkänts gäller vanliga flyttregler.
- `.Ok(mut x)` skapar en muterbar bindning och kräver `match move ...` eller ett
  färskt ägt uttryck (annars föreslås `match move` eller lokal kopia/`clone` för
  klonbara typer). Icke-klonbara `@resource`-typer föreslår aldrig `.clone()`
  i någon ägarskaps- eller lånediagnostik.
Ägare som ett aktivt `defer` behöver får inte flyttas eller autofrisättas
före den uppskjutna körningen.

Arrayrepetition som `[text; 4]` är en uttrycklig replikering: initialiseraren
utvärderas och konsumeras en gång, även om antalet är noll. Skriv
`[text.clone(); 4]` när den befintliga strängägaren ska finnas kvar.

## Returnerade läslån

```dunder
fn window(data: &[]u8, start: i32, end: i32) -> &[]u8 {
    &data[start..end]
}
```

`window` kan lämna tillbaka en del av anroparens buffert. En funktion med
flera delade parametrar anger källan med `from namn` efter returtypen.
Samtliga returvägar måste kunna härledas till samma parameter. Ett fält,
ett arrayelement, ett lokalt alias till parametern eller ett vidarebefordrat
anrop kan alla uppfylla det kravet. Lokala ägare, råpekare och
`move`-parametrar ger ingen sådan livstid.

Om anroparen binder resultatet till en lokal gäller lånet fram till
referensens sista lexikala läsning i blocket (eller till blockets slut vid
`defer` eller när ägaren ändras i ett djupare loopscope). Det skyddar hela
platsen som lämnades som källargument. Den som skickar `&record.bytes` kan
alltså fortfarande ändra ett oberoende fält i `record`. Ett arrayintervall
delar elementlagringen med originalet.
Anroparen håller den returnerade vyns beskrivare i sin egen stackram,
så att flera anrop kan hålla olika intervall levande samtidigt. I
`unsafe` observerar `addr(&view[i])` adressen till originalelementet.
`addr(&view)` observerar vyns egen referenscell, även när funktionen
vidarebefordrar en hel array. En uttrycklig `.clone()` ger en ägd kopia.

Returkontraktet gäller delade referenser i vanliga Dunder-anrop. C-anrop,
taskresultat och referenser lagrade i structar eller arrayer kräver egna
livstidsregler och ingår inte. För låg-nivåbehållare som lagrar råa pekare
(till exempel `OwnedBuf::as_bytes`, `Arena::bytes` och `FrozenArena::bytes` i
`std.alloc`) skapar `unsafe bytes_view(&owner, base, length)` en lånad vy
`&[]u8` bunden till `&owner`: kompilatorn håller källan lånad lika länge som
vyn används; se [FFI och råminne](ffi.md).

## Pool och kostnader

Nativ kod mappar en initial heapregion (samt dynamiska tilläggsblock/slabs via
`mmap` vid fortsatt tillväxt) vid första allokeringen. Resident minne beror på vilka sidor programmet använder. Wasm utökar sitt
linjära minne vid behov. På alla mål delar strängar och arrayer samma pool.
Frigjorda block läggs på en lista för sin storleksklass. Klasserna börjar vid
16 byte och växer i tvåpotenser. En ny begäran kan återanvända ett block i rätt
klass. Blandade storlekar kan lämna lediga block i andra klasser i en enskild slab innan ett nytt block mappas.

Innanför ett dynamiskt stränghandtag ligger en 64-bitars bytelängd följd av
UTF-8-data. Referensräknaren ligger åtta byte före handtaget. Sexton byte före
ett dynamiskt handtag ligger en pekare till storleksklassens frilista. Statiska strängar
har räknare noll. Strängar kan dela data utan cykler eftersom deras innehåll
saknar referenser till andra objekt. Referensräkningen är icke-atomär.

Dynamisk sammanslagning och snitt tillåter högst 1 GiB per strängblock
inklusive längdordet på de nativa målen och 64 MiB på wasm. En misslyckad
strängallokering i dessa operatorer ger status 105. En ogiltig UTF-8-gräns eller Unicode-kodpunkt ger 106.
En ogiltig arraykapacitet eller otillräckligt utrymme för en array eller region ger 109.
Wasm avbryter den aktuella körningen med en trap. För uttrycklig
blockallokering utan dolda fällor erbjuder [`std.alloc`](standard.md#generell-allokering)
`RawAllocator` (`Global`, `System`, `Capped<A>`, `Tracking<A>`, `Bump<A>`),
`OwnedBuf<A>`, `Arena<A>`/`FrozenArena<A>` samt generationssäkra `Pool<T>`/`Id<T>`.

## Verifiering

```sh
dunder_bin="$(./tools/bootstrap.sh)"
"$dunder_bin" run tools/test_runtime_contracts.ddr -- "$dunder_bin"
"$dunder_bin" run tools/test_array_moves.ddr -- "$dunder_bin"
"$dunder_bin" run tools/test_resource_moves.ddr -- "$dunder_bin"
"$dunder_bin" run tools/test_resource_ownership.ddr -- "$dunder_bin"
"$dunder_bin" run tools/test_stdlib_ownership.ddr -- "$dunder_bin"
"$dunder_bin" run tools/test_local_borrows.ddr -- "$dunder_bin"
"$dunder_bin" run tools/test_returned_borrows.ddr -- "$dunder_bin"
"$dunder_bin" run tools/test_io.ddr -- "$dunder_bin"
"$dunder_bin" run tools/test_alloc.ddr -- "$dunder_bin"
```

Lånesviten korsbygger de tre nativa målen och kör värdmålets program.
`--runner TARGET=COMMAND` kan upprepas för lokala målmiljöer, exempelvis
`--runner 'arm64-linux=/sökväg/till/lokal-runner arm64'`. Kommandot får
målbinärens absoluta sökväg som sista argument. Sviten kontrollerar adress,
allokerings- och retainräknare, livstidsfel, intervall och UTF-8; den gör inga
prestandamätningar.

Kontrakts- och ägarskapssviterna är skrivna i Dunder
(`tools/test_runtime_contracts.ddr`, m.fl.) med Node för Wasm-proven. Minnesproven håller
bland annat arraykopior, nästlade enumvärden och strängreturer levande medan
andra värden ersätts eller frigörs. På värdmålet kontrolleras resultat och
maximal RSS för de minnesbegränsade fallen. [Dunder-programmen](../tests/contracts/). För Wasm kör Node separata prov för
bland annat minnesoperationer och arrayer. [Wasm-målet](targets.md#webassembly)

För heltal, flyttalsbitar, UTF-8 och samlingar finns
[körbara referensfall](../tests/fixtures/runtime_contracts/). Felproven
kontrollerar status och exakt utskrift före felet. Med `--case string_returns`
körs ett enskilt prov. Vid körning med en emulator kan `--runner` ange
programmet som startar målbinären. RSS-gränsen gäller när målbinären körs direkt.

## Lexikala scratch-regioner

En tidsbegränsad arena skapas med `region namn(antal_byte)`.
Kapaciteten är ett `i32` som utvärderas en gång vid inträdet. Utan parenteser
är kapaciteten 64 KiB. Regionen lånar en buffert från minnespoolen. Varje
`alloc` flyttar en markör i bufferten, justerar adressen för elementtypen och
nollställer de nya elementen. Underblock och loopar kan allokera från samma
region. Den lämnar tillbaka sin buffert vid blockslut, retur och loopavbrott.

```dunder
struct Node { left: i32, right: i32, weight: i64 }

fn frame()->i64 {
    region scratch(4096) {
        mut nodes: []Node = scratch.alloc(32)
        nodes[3].weight = 12
        return nodes[3].weight
    }
}
```

En vy har fast längd. Den tillåter indexering och `len` och dess element
kan lånas under ett funktionsanrop. Element kan vara tal, `bool`, `rune`
eller structar med motsvarande värdefält. En kopia av en nod är ett vanligt
värde. En hel vy kan delas med en funktion som tar ett delat arraylån för
en representation som matchar parameterlagringen. Vanliga structelement
lagras inline i regionen och som pekarplatser i en ägd array, så hela
scratchvyer av sådana structar kan inte lånas som `&[]Struct`.
Kompilatorn avvisar tilldelning, retur, råadresser och
storleksändringar av själva vyn. En för liten region, negativt elementantal
eller index utanför vyn ger status 103. Negativ eller omöjlig begäran om
regionkapacitet ger arrayallokerarens status 109.


Självkompilatorn stöder markörallokering för skalärer, vanliga flata
structar med skalära fält och `@c_layout`-structar med nästlade värdefält
eller fasta arrayer av skalärer. För ett lån
till ett skalärt element eller en hel vy kräver selfhost att elementens
representation matchar funktionsparameterns lagring. Ett lån av ett enskilt
fält eller en C-layout-struct använder regionens lagring. Ett muterbart lån
av en hel vanlig struct använder en ägd tillfällig kopia och skriver tillbaka
fältvärdena efter anropet. Självkompilatorn följer lån genom funktionsanrop,
återlån och funktionspekare och avvisar lån som når `addr`. En kopia av ett
enskilt element blir ett vanligt självständigt värde, medan `.clone()` på en
hel scratch-vy avvisas (`scratch values cannot be cloned out of their region`).
En inre region kan skugga en yttre. Namnet på regionen och dess direkta
deklarationer tillhör samma scope. Se `tests/contracts/region_borrows.ddr`
och de ogiltiga exemplen under `tests/fixtures/region_errors/`.


### Packade bytebuffertar

`[]u8` använder en byte per element. `std.bytes.append_utf8` och
`std.bytes.decode` arbetar med packade buffertar. Tabells CSV-fält, SHA-256
och selfhost-kompilatorns kodbild och signatur använder samma representation.
`sys.write_u8` och `sys.decode_u8` tar `&[]u8`. `&[]i32`-buffertar
konverteras mellan representationerna med `std.bytes.from_i32` och
`std.bytes.to_i32`.

### Maskinbreda buffertar

På nativa 64-bitarsmål tar `std.alloc.layout_array<T>(count)` ett antal av
typen `usize`. `System` kan reservera en större region än minnespoolens
blockgräns. Ett prov med 536 870 913 `u32` reserverar 2 147 483 652 byte,
skriver det första och sista elementet och läser tillbaka det sista.
Systemallokatorn reserverar virtuellt adressutrymme. Provet berör bara
sidorna i vardera änden.

En inbyggd `&[]u8` använder `i32` för längd och index. Att skapa den
över hela regionen ovan ger status 103. En ägare som använder `usize`
kan i stället låna ut fönster med `bytes_view`. Kontrollera först
`first <= end && end <= size`, därefter att `end - first` ryms i
`i32`. Ett tomt intervall vid slutet är giltigt. Fönstrets index
startar på noll även när `first` ligger över 2 GiB.

`usize` passar för elementantal och C:s `size_t`. En baklängesloop med
osignerade index kan börja vid den exklusiva slutgränsen, testa
`end > first` och minska före läsning. En signerad pekarbred differens
kan också representera läget −1 före första elementet. På de nativa
64-bitarsmålen kan den uttryckas med `i64`. En sådan C-bindning behöver
`i32` på wasm32 för att motsvara `ptrdiff_t`.

På wasm32 kontrollerar `Layout` både multiplikation och utfyllnad mot
2 147 483 647 byte. Exempelvis ryms 536 870 911 stycken `u32` inom
layoutgränsen. Nästa element ger `AllocError.InvalidLayout`.
Omvandlingen från `usize` till `i32` kontrollerar att värdet ryms.
En godkänd layout garanterar inte att värdens minne räcker.
En HTTP-kropps bytegräns och `Capped<A>` begränsar fortfarande vad
anroparen får förbruka oavsett adressbredd.
