# Minne och ägarskap

Dunder frigör strängar, arraybuffertar och sammansatta värden under körning.
Kompilatorn lägger in städning vid tilldelning, blockslut, retur, `break`
och `continue`. Den genererade koden och minnespoolen följer med varje program
på ARM64/macOS, ARM64/Linux, x86-64/Linux och Wasm32.

## Ägare, lån och kloning

En bindning är oföränderlig utan `mut`. Muterbarhet anger vilka ändringar
som är tillåtna; ägarskap anger vem som ska frigöra värdet.

| Användning | Regel |
| --- | --- |
| `b := a`, tilldelning, retur och konstruktion | Flyttar resursägande värden till mottagaren. |
| Vanlig funktionsparameter | Lånar strängar och aggregat för läsning under anropet. |
| `&mut value` | Exklusivt skrivlån; kräver muterbar lagring. |
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

Standardbibliotekets `TcpListener`, `TcpStream`, `StreamSet`, `Child` och
`ChildSet` är sådana resurser. Fil- och körningskapabiliteter som bara
innehåller en sökväg kan däremot uttryckligen klonas.
Markeringen lägger ännu inte till en användardefinierad destruktor:
socketstängning och processhantering sker med bibliotekets explicita
`close`-/`wait`-/set-operationer. Ingen implicit OS-hantering har tillkommit.
Resurser får tills vidare inte transporteras över isolerade taskgränser,
vars snapshotprotokoll kräver klonbara värden. `@resource` kombineras inte
med `@c_layout`.

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
    print(total(values))              // samma läslån igen
    independent := values.clone()    // egen arraybuffert
    saved := retain(move values)     // ägarbyte, ingen elementkopia
    print(total(saved))
    print(total(independent))
}
```

Användning efter flytt ger kompileringsfel. En `mut`-lokal kan få ett nytt
värde efter flytt. Efter grenar måste värdet finnas på varje väg som når
användningen; en loop måste återställa värden som behövs i nästa varv.
Ett lånat värde får inte flyttas till en ägd bindning, retur eller container.
Välj en `move`-parameter när funktionen ska behålla indata, eller klona när
det ska finnas två oberoende ägare. Lån kan ännu inte returneras eller
lagras som lokala referenser eller fält.

Flytt av en dynamisk array behåller buffert och kapacitet. `.clone()` ger en
oberoende buffert och klonar ägda barn rekursivt; klonens reservkapacitet
behöver inte matcha originalets. Strängar är oföränderliga: deras uttryckliga
kloner delar UTF-8-data genom referensräkning. Kloning av en sträng behöver
därför ingen teckenkopia. Literaler ligger i programbilden.

Tilldelning utvärderar det nya värdet innan det gamla släpps. Varje ägd
resurs frigörs en gång när dess ägare lämnar sitt scope. `pop` tar över det
borttagna elementet. `clear` släpper elementen och behåller kapaciteten.

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

### Fält, element och begränsningar

Ett ägt managed-fält kan flyttas ut ur en lokal eller ett färskt returvärde
utan datakopia. Den här versionen behandlar då hela ursprungsobjektet som
förbrukat; dess övriga fält frigörs fortfarande vid scope-slut. En hel
muterbar bindning kan återinitieras. Flytt ur ett lån avvisas.
Arrayelement lämnas över med `pop` eller kopieras uttryckligen med `.clone()`;
indexering lämnar ingen oinitierad lucka i arrayen. Inline-fält i C-layout
kan klonas när ett självständigt arrayvärde behövs.
`values.swap(i, j)` byter element på plats utan kloning eller referensräkning.

Vanlig `for` och `match` lånar. `for value in move values` respektive
`match move value` överför ägaren. `for value in values.iter()` lånar också;
en lagrad ägande iterator skapas från `(move values).iter()` eller
`values.clone().iter()`. Ägda payloadbindningar överförs först efter att
matcharmens villkor har lyckats. Ägare som ett aktivt `defer` behöver får
inte flyttas före den uppskjutna körningen.

Arrayrepetition som `[text; 4]` är en uttrycklig replikering: initialiseraren
utvärderas och konsumeras en gång, även om antalet är noll. Skriv
`[text.clone(); 4]` när den befintliga strängägaren ska finnas kvar.

## Pool och kostnader

Nativ kod mappar 512 MiB virtuellt adressutrymme vid första allokeringen.
Resident minne beror på vilka sidor programmet använder. Wasm utökar sitt
linjära minne vid behov. På alla mål delar strängar och arrayer samma pool.
Frigjorda block läggs på en lista för sin storleksklass. Klasserna börjar vid
16 byte och växer i tvåpotenser. En ny begäran kan återanvända ett block i rätt
klass. Blandade storlekar kan lämna lediga block i andra klasser när poolen blir full.

Innanför ett dynamiskt stränghandtag ligger en 64-bitars bytelängd följd av
UTF-8-data. Referensräknaren ligger åtta byte före handtaget. Statiska strängar
har räknare noll. Strängar kan dela data utan cykler eftersom deras innehåll
saknar referenser till andra objekt. Referensräkningen är icke-atomär.

Dynamisk sammanslagning och snitt tillåter högst 16 MiB per strängblock
inklusive längdordet. En misslyckad strängallokering i dessa operatorer ger
status 105. En ogiltig UTF-8-gräns eller Unicode-kodpunkt ger 106.
En ogiltig arraykapacitet eller otillräckligt utrymme för en array eller region ger 109.
Wasm avbryter den aktuella körningen med en trap.

## Verifiering

```sh
dunder_bin="$(./tools/bootstrap.sh)"
python3 tools/runtime-contracts.py "$dunder_bin"
python3 tools/test-array-moves.py "$dunder_bin"
python3 tools/test-resource-moves.py "$dunder_bin"
python3 tools/test-resource-ownership.py "$dunder_bin"
python3 tools/test-stdlib-ownership.py "$dunder_bin"
```

Testverktyget använder Python 3:s standardbibliotek. Det
bygger varje program med den angivna Dunder-kompilatorn. Tio minnesprov håller
bland annat arraykopior, nästlade enumvärden, lånade parametrar, snitt och
strängreturer levande medan andra värden ersätts eller frigörs. Exempelvis
kopierar ett prov en array med 1 024 `i64` i 200 000 varv. Ett annat tar emot
en miljon dynamiska strängreturer. Alla tio kontrollerar resultatet och
kräver under 96 MiB maximal RSS på det nativa värdmålet.
[Dunder-programmen](../tests/contracts/). Wasm-körningar ingår tills vidare
inte i verifieringskedjan; se [aktuell omfattning](status.md#webassembly-senarelagt-under-språkutveckling).

Testverktyget jämför även heltal, flyttalsbitar, decimalformatering, UTF-8,
mappar, mängder och sortering med oberoende Python-beräkningar. Felproven
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
en tillåten arraytyp. Kompilatorn avvisar tilldelning, retur, råadresser och
storleksändringar av själva vyn. En för liten region, negativt elementantal
eller index utanför vyn ger status 103. Negativ eller omöjlig begäran om
regionkapacitet ger arrayallokerarens status 109.


Självkompilatorn stöder markörallokering för skalärer, vanliga flata
structar med skalära fält och `@c_layout`-structar med värdefält som kan
vara nästlade eller fasta arrayer. För ett lån
till ett skalärt element eller en hel vy kräver selfhost att elementens
representation matchar funktionsparameterns lagring. Ett lån av ett enskilt
fält eller en C-layout-struct använder regionens lagring. Ett muterbart lån
av en hel vanlig struct använder en ägd tillfällig kopia och skriver tillbaka
fältvärdena efter anropet. Självkompilatorn följer lån genom funktionsanrop,
återlån och funktionspekare och avvisar lån som når `addr`. En kopia av ett
element eller en uttrycklig arrayklon har egen lagring.
En inre region kan skugga en yttre. Namnet på regionen och dess direkta
deklarationer tillhör samma scope. Se `tests/contracts/region_borrows.ddr`
och de ogiltiga exemplen under `tests/fixtures/region_errors/`.


### Packade bytebuffertar

`[]u8` använder en byte per element. `std.bytes.append_utf8` och
`std.bytes.decode` arbetar med packade buffertar. Tabells CSV-fält, SHA-256
och selfhost-kompilatorns kodbild och signatur använder samma representation.
`sys.write_u8` och `sys.decode_u8` tar `&[]u8`. Befintlig kod med
`&[]i32` kan använda `sys.write_bytes` och `sys.decode_utf8`.

Ett prov med 8 388 608 element rapporterade 9 912 320 byte maximal RSS med
`[]u8` och 35 078 144 byte med `[]i32` på Apple M5 och macOS 27.2.
Värdena är medianer av fem processer per variant byggda med den självhostade
Stage 3-kompilatorn den 10 november 2026.

Bygg provet igen med `i32` på båda ställen för fyrabytesvarianten.

```dunder
fn main() {
    mut data: []u8 = []
    data.reserve(8388608)
    for i in 0..8388608 { data.push(u8(i % 256)) }
    print(data[8388607])
}
```
