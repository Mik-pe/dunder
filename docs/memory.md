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
| `r := &place` | Namnger ett oföränderligt läslån. Läsningar i en loop kan upprepas. En läsning i `defer` håller lånet vid liv till blockslut. |
| `&mut value` | Exklusivt skrivlån av muterbar lagring, lokalt eller vid anrop. |
| `move`-parameter och `consume(move value)` | Funktionen tar över ägarskapet. |
| `value.clone()` | Skapar en ytterligare ägare uttryckligen. |
| Vanlig `for value in values` | Läslån av samlingen under loopen. |

Regeln för flytt gäller `string`, dynamiska arrayer (`[]T`), iteratorer,
`&mut T`, fångande closures (`Fn`, `FnMut`, `FnOnce`) samt fasta arrayer `[T; N]`, structar och enums som innehåller
flyttbundna värden (inklusive `Map`, `Set` och `@resource`). Tal, bool, runes, pekare, rena
funktionspekare (`fn(...)`), delade vyer (`&T`, `&str`, `&[]T`), fältlösa enums och `Copy`-värden (nästlade structar,
tupler, data-enums och fasta arrayer `[T; N]` vars element är `Copy`) kopieras i stället vid tilldelning och retur.
`&str` och `&[]T` är förstklassiga 2-ords `Copy`-värden `(ptr, len)` (16 byte på 64-bitars nativa mål, 8 byte på `wasm32`) som skickas och returneras direkt i register utan stackbeskrivare.
`Fn(...)` implementerar `Clone` via referensräkning i miljön **endast när alla fångade fält är `Clone`**; en `Fn` som fångar en icke-klonbar `@resource` kan skapas och anropas, men `.clone()` avvisas statiskt. `FnMut` och `FnOnce` är aldrig `Clone`. När en `Copy`-struct,
tupel, payload-enum eller positiv fast array `[T; N]` av POD-element ryms inom 16 åttabyte-ord (upp till 128 byte för fasta arrayer) lagras den som ett platt
inline POD-värde på stacken (utan heapallokering eller referensräkning), och lån
(`&T`/`&mut T`) passerar buffertpekaren direkt (`T*`) på samma sätt som för
`@repr(C)`. Samma platta POD-värden lagras dessutom avboxade direkt intill
varandra i bufferten för `[]T`, `[T; N]`, `Map` och `Set` utan separata
elementallokeringar. Typens ägarskap bestämmer kopierings- och flyttreglerna,
oberoende av om representationen ligger på stack eller heap (större `Copy`-värden kopieras automatiskt vid tilldelning och `.clone()`).

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
`FrozenArena` är sådana resurser. Allokator- och bufferttyperna i `std.alloc`
(`OwnedBuf<A>`, `Bump<A>`, `Arena<A>`, `FrozenArena<A>`, `Capped<A>`, `Block`
och `Layout`) är förseglade som `pub opaque struct` så att säker kod inte kan
förfalska interna pekare, längder eller kapaciteter (läsåtkomst sker via säkra
metoder som `.len()`, `.cap()`, `.size()`, `.align()`, `.used()` och `.remaining()`).
Råa OS-filbeskrivaranrop i `std.sys` (`sys.net_read`, `sys.net_write`,
`sys.net_close`, `sys.net_poll` och `sys.fd_*`) är `unsafe fn`; säker kod
använder i stället ägda `@resource`-handtag i `std.net` och `std.fs`.
Handtag som `TcpListener`, `TcpStream`, `UdpSocket`, `File` och `sqlite.Database`
implementerar `unsafe impl Sendable` med strikt `move`-överföring mellan trådar
(ingen kloning; sändarens ägarslot nollställs), medan trådsäkra synkroniseringshandtag
som `Sender<T>`, `Receiver<T>`, `Published<T>`, `Semaphore`, `CancelToken`,
`AtomicI64`, `AtomicBool` och `Waker` även deklarerar `unsafe impl Sync`.
`task scope`-arbetsytor kan även äga `@resource(dispose)`-fält som stängs på
ägarens tråd när scopet avslutas. Fil- och körningskapabiliteter som bara
innehåller en sökväg kan däremot uttryckligen klonas.
Lokala resurser i en worker följer scopets beviljade kapabiliteter och effektregler.
Transport över en isolerad taskgräns kräver `Sendable`-värden; frysta ögonblicksbilder
i `std.sync.Published<T>` och `task scope (shared ...)` kräver `T: Sendable + Sync`
(se [Isolerade uppgifter](tasks.md)). Resurstyper använder Dunders vanliga structlayout
och lagras som ägda värden.

### Automatisk stängning

Med `@resource(dispose)` anropar kompilatorn den privata instansmetoden
`fn dispose(&self)` eller `fn dispose(&mut self)` när det sista ägda värdet
släpps. Metoden måste vara säker och returnera `()`. Den körs synkront före
frigöring av structens fält och kan därför blockera om stängningsoperationen
blockerar. Samma regel gäller vid blockslut, `return` (även av ett felvärde via `?`),
`break`, `continue`, ersättning av ett värde och när ägaren finns i ett aggregat.
Muterbar `dispose(&mut self)` gör att resurser som `FileReader`, `Bump`,
`OwnedBuf`, `Arena` och `FrozenArena` även kan dela på en explicit `.close()`
eller `.release()` och lämna värdet tomt utan dubbel frigöring.

**Destrukturering, `dropck` och panikkontrakt (1.0):**
- **Destrukturering och partiell flytt:** både partiell fältflytt (`r.fält`) och
  full struct-destrukturering (`Res { ... } := move r`) av en `@resource(dispose)`-struct
  är **förbjudna i säker kod överallt** eftersom `dispose()` annars skulle hoppas över.
  Destrukturering tillåts enbart inuti `unsafe { ... }` i den deklarerande modulen
  (till exempel `OwnedBuf::into_allocator` och `Arena::freeze` i `std.alloc`, där
  modulen själv garanterar städkontraktet).
- **Drop-check (`dropck`):** om en `@resource(dispose)`-typ (direkt eller transitivt)
  lagrar lån håller kompilatorns `Drop`-punkt dessa ursprung levande fram till
  städningen så att `dispose(&self)` aldrig kan läsa en redan ändrad eller frigjord
  ägare. Vanliga structar, enums och vyer utan `dispose` är `may_dangle` — deras
  implicita städning håller inte ursprungen vid liv efter sista användningen.
- **Panik- och fällkontrakt (abort-only):** `panic()`, `assert()` samt runtimefällor
  (heltalsspill `101`, index utanför gräns `103`, strängallokering `105`, ogiltig UTF-8 `106`,
  flyttalsfälla `107`, okänd sluten enum `108`, kapacitets-/minnesgräns `109`,
  ogiltig callback `111`, task-/deskriptorfälla `112`) avbryter processen omedelbart
  utan stackupprullning — varken `defer` eller `dispose` körs vid en panik.
  Återhämtningsbara fel uttrycks med `Result<T, E>` och `?`, som alltid rullar upp
  i strikt LIFO-ordning (`task scope`-inväntan före yttre `defer` och `dispose`).

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
nedan. Funktioner kan även lämna tillbaka läs- och skrivlån från en parameter.

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
Arrayer av ohanterade `@repr(C)`-structar och av POD-värden (kopierbara
structar, tupler, data-enums och fasta arrayer `[T; N]` upp till 16 åttabyte-ord / 128 byte) lagrar elementen
avboxade och direkt intill varandra i samma buffert utan separata
heapallokeringar per element vid `push`, `pop`, `remove`, `swap`, `replace`,
`clone`, skivning (`&values[a..b]`), `for`-loopar eller lagring i `Map` och `Set`.
Fasta POD-arrayer `[T; N]` lagras dessutom helt på stacken även som fristående värden och literaler,
och kopiering eller parameteröverföring av t.ex. `[f64; 2]` eller `[f32; 4]` gör noll heapallokeringar.

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

`r := &owner.field` och `r := &values[index]` skapar lokala referenser med härledd eller explicit typ (`r: &i32 = &values[0]`), och `mut r: &T` kan bindas om lokalt (`r = &annan`) varvid det tidigare ursprunget omedelbart släpps vid full överskrivning.
Lånekontrollen arbetar som icke-lexikal livstidsanalys (**CFG-NLL / Polonius-lik** ursprungspropagering över kontrollflödesgrafen utan tokenskanning):
- Ett lån upphör exakt efter sin sista nåbara användning i kontrollflödesgrafen. Ömsesidigt uteslutande `if`/`else`-grenar (exempelvis `if c { v.push(3) } else { print(r) }`) och sammanfallande fält- eller parameternamn (`Foo { r: 1 }`) ger inga falska konflikter.
- Läsningar i loopar bevarar lånet längs bakåtkanter, och läsningar i `defer` håller både ägare och lån vid liv längs **varje** utgångsväg (`return`, `?`, `break`, `continue` och normalt blockslut).
- **Invarians bakom `&mut`:** ursprung som ingår i `T` bakom `&mut T` eller `&mut FnMut` är bidirektionellt invarianta (`O_a <-> O_b`). Det förhindrar att ett kortlivat lokalt lån skrivs in i en mer långlivad ägare genom en muterbar referens. I Dunder 1.0 tillåts lokal ombindning (`mut r: &T` samt `&mut`-ombindning av lokala poster inom samma funktion), medan överskrivning av lagrade lånefält genom en `&mut`-funktionsparameter (t.ex. `fn set(dst: &mut Holder, val: &i32)`) avvisas.
- **Tvåfaslån (`TwoPhaseMut`):** används strikt för implicita `&mut self`-metodmottagare (t.ex. `v.push(v.len)`). Under argumentutvärderingen tillåts delade läsningar av mottagaren, medan skrivningar, flyttar, `&mut` och ett andra tvåfaslån (`v.push(v.pop())`) avvisas; vid anropspunkten aktiveras lånet till fullt `Mut` och kontrolleras på nytt mot aktiva lån (så `v.push_ref(&v[0])` avvisas).
- **Återlån och utvärderingsordning:** sammansatta uttryck — anropsargument, binära operatorer (inklusive `&str ==`/`!=`), snittgränser, structliteraler och `replace` — utvärderas strikt från vänster till höger med levande lån i temporärer. Återlån genom ett `&mut`-alias (`&*r`, `&mut *r`, `&r[0]`) kopplas till den underliggande ägaren så att motstridiga ändringar via aliaset fångas enhetligt.

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
storleksändring eller `&mut str` är tillåten. En uttrycklig `.clone()` (eller `.str()`/`.to_string()`)
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

### Lån i structar och enums

Delade och exklusiva lån kan ingå i structfält och enumpayloads, även via nästlade typer (inklusive `Option<&mut T>`, `Result<&str, E>` och användardefinierade markörer).
En funktion som returnerar ett sådant värde anger källan med `from` när det finns flera möjliga källparametrar.
Lokala värden — även från `if`- och `match`-uttryck (`r := if c { &a } else { &b }`) — kan låna från flera ägare; varje ägare förblir lånad till sista nåbara användningen. Vid retur måste samtliga ursprung härstamma från den deklarerade `from`-källan. Fältlösa enumvarianter som `.None` och `.Err` delar statisk lagring utan allokering eller ägare. Matchning och destrukturering bevarar sambandet med den ursprungliga lagringen.

Ett värde med lån kan bindas lokalt och skickas och returneras mellan funktioner.
Arrayelement, ägda closuremiljöer som lämnar scopet och taskgränser kräver fristående värden.
Eftersom `&str` och `&[]T` är 2-ords `Copy`-värden `(ptr, len)` utan lokal stackramsbeskrivare kan även `Option<&str>`, `Result<&str, E>`, tupler och structar som lagrar delvyer returneras direkt när underliggande ägare anges med `from`.

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
`swap(&mut a, &mut b)` byter två initierade ägare av samma typ, även bakom muterbara parametrar. Platserna måste vara åtskilda och båda indexuttrycken utvärderas före bytet.

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

`match &mut pages[i]` lånar variantens nyttolast exklusivt. I en arm som
`.Ready(n) => { n.value += 1 }` ändras posten i den ursprungliga arrayen.
Ett delat mönster kan lämna en strängvy från ett fält när mottagaren äger
fältet och returkontraktet anger `from self`.

Ett uppslag med returtypen `Option<&mut i32> from self` kan ge
`.Some(&mut self.values[i])` eller `.None`. Värdet i `.Some` håller
ägaren exklusivt lånad fram till sin sista användning. Matcha resultatet
direkt eller använd `match move` för ett namngivet resultat; i `.Some(m) => { m.clear() }`
återlånas `*m` för metodanrop och `&mut m` utan att flytta `m`.
I en `match`-arm med guard hålls matchningsämnet under ett delat `Shared`-lån
genom hela guarden, så guarden får varken mutera eller flytta ut bindningar (`if take(move v)` avvisas);
först efter att guarden godkänts gäller vanliga flyttregler i armkroppen.

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
- `.Ok(mut x)` skapar en muterbar bindning och kräver `match move ...` eller ett
  färskt ägt uttryck (annars föreslås `match move` eller lokal kopia/`clone` för
  klonbara typer). Icke-klonbara `@resource`-typer föreslår aldrig `.clone()`
  i någon ägarskaps- eller lånediagnostik.
Ägare som ett aktivt `defer` behöver måste vara initierade och olåsta för flytt
längs varje väg som når `defer`-blocket — inklusive tidig `return`, `?`, `break` och `continue`.

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
referensens sista nåbara användning i kontrollflödesgrafen, med samma regler för loopar och
`defer` som för direkta läslån. Det skyddar hela
platsen som lämnades som källargument. Den som skickar `&record.bytes` kan
alltså fortfarande ändra ett oberoende fält i `record`. Ett arrayintervall
delar elementlagringen med originalet och returneras som ett 2-ords `Copy`-värde `(ptr, len)` i register utan lokal stackbeskrivare. I
`unsafe` observerar `addr_of(&view[i])` adressen till originalelementet och
`addr_of(&view)` adressen till vyns egen lokala cell. En uttrycklig `.clone()` ger en ägd kopia.

En metod som uppdaterar sin samling kan lämna tillbaka en delad vy med
`fn refresh(&mut self) -> &[]i32 from self`. Anroparen får läsa vyn
och får ändra samlingen igen efter vyns sista användning.

En funktion kan också returnera ett exklusivt lån från en muterbar parameter,
exempelvis `fn first(a: &mut []i32) -> &mut i32 from a { &mut a[0] }`.
Anroparen kan binda resultatet lokalt och skriva genom det. Överlappande
åtkomst måste vänta tills lånet upphör. Returkontraktet gäller vanliga Dunder-anrop. C-anrop,
taskresultat och referenser lagrade i structar eller arrayer kräver egna
livstidsregler och ingår inte. För låg-nivåbehållare som lagrar råa pekare
(till exempel `OwnedBuf::as_bytes`, `Arena::bytes` och `FrozenArena::bytes` i
`std.alloc`) skapar `unsafe bytes_view(&owner, base, length)` en lånad vy
`&[]u8` bunden till `&owner`: kompilatorn håller källan lånad lika länge som
vyn används; se [FFI och råminne](ffi.md).

## Pool och kostnader

Ett nativt program begär sin första heapregion när det behöver allokera.
För små block begär runtimen 64 KiB. En större första allokering får en region
som rymmer hela storleksklassen och dess metadata. En profilerad körning
skapar regionen redan vid start för att lagra räknarna.

När den aktiva regionen blir full begär runtimen en ny. Den föredragna
storleken fördubblas upp till 64 MiB. Ett större enskilt block styr självt
storleken. Om operativsystemet nekar extra tillväxt försöker runtimen igen
med utrymmet för just blocket och dess metadata, avrundat uppåt till 4 KiB.
Resident minne beror på vilka sidor programmet använder. Wasm utökar sitt
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
structar med skalära fält och `@repr(C)`-structar med nästlade värdefält
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

`alloc.with_bytes(block, read)` lånar ut hela blocket till en funktion
med parametern `&alloc.ByteView` och returnerar funktionens resultat.
Anropet är `unsafe`. Anroparen ansvarar för att blockets minne är
initialiserat, läsbart och stabilt under hela anropet. `view.len()` och
indexet i `view.get(index)` har typen `usize`. Vid slutet eller längre
bort returnerar `get` värdet `.None`. Vyn och lån från den får bara
användas under anropet.

`view.window(first, end)` lånar ut intervallet som `&[]u8` utan att
kopiera data. Gränserna måste uppfylla `first <= end && end <= view.len()`.
Antalet byte i fönstret måste rymmas i `i32`. Ett tomt fönster vid slutet
är giltigt och lokala index börjar på noll även när `first` ligger över
2 GiB.

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

En synkron callback kan fånga en post med lånade fält genom `fn[&post]`.
Både posten och fältens ägare förblir lånade medan callbacken kan anropas.
En closure som returnerar ett lån härlett ur sin fångade miljö (t.ex. `fn[&v]() -> &i32 { &v[0] }`)
binder returursprunget till miljöns fångade ursprung (`PARAM_ENV`), så att `v` förblir lånat så länge
returvärdet används även efter att closuren själv gått ur bruk.
En callback som ska överleva anropet behöver äga sina fångade data.

Traiten `Iterator` är **icke-lånande** i 1.0: en iterator som bär ett lagrat lån (exempelvis
`std.text.Graphemes`) knyter resultatet från `next(&mut self)` till ett delat lånefält på typen
med `from self.text` — fristående från `&mut self`-lånet. När mottagaren är en lokal eller tillfällig
iterator (som `for g in text.graphemes(s.as_str())`) hålls den lånade källan `s` låst genom hela
loopen medan markörens egna fält kan flyttas fram, och två resultat från `Graphemes.next` kan
läsas samtidigt. En iterator vars `next(&mut self)` försöker returnera ett lån bundet direkt till
`&mut self` (`from self`, t.ex. `&mut self.buf[i]`) avvisas från `Iterator` eftersom upprepade
`next()`-anrop annars skulle skapa överlappande `&mut`-alias.

Funktioner med `Option<&str> from value`, `Result<&str, E> from value` eller användarstructar
med `&str`/`&[]T`-fält returnerar 2-ords `Copy`-vyer `(ptr, len)` direkt utan stackramshinder;
underliggande data förblir lånade från `value`.
