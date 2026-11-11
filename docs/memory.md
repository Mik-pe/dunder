# Minne och ägarskap

Dunder frigör strängar, arraybuffertar och sammansatta värden under körning.
Kompilatorn lägger in städning vid tilldelning, blockslut, retur, `break`
och `continue`. Den genererade koden och minnespoolen följer med varje program
på ARM64/macOS, ARM64/Linux, x86-64/Linux och Wasm32.

## Värden och ansvar

| Värde | Kopiering | Frigöring |
| --- | --- | --- |
| Dynamisk array | Ny oberoende buffert. Strängelement kan dela oföränderliga data. | Elementen släpps, sedan bufferten och dess deskriptor. |
| Struct, fast array och payload-enum | Fält och element kopieras enligt sin typ. Enumens aktiva variant bestämmer payloaden. | Ägda fält och element släpps rekursivt. |
| `string` | En dynamisk sträng får en referens till. Literaler ligger i programbilden. | Sista ägaren återlämnar ett dynamiskt block till poolen. |
| `.bytes()` och `.runes()` | Skapar ett ögonblicksvärde med bytes respektive Unicode-kodpunkter. | Den genererade koden städar värdet när dess scope lämnas. |
| `&T` och `&mut T` | Lånar anroparens värde. | Ägaren ansvarar för lagringen. |

Tilldelning säkrar det nya värdet innan det gamla släpps. En array som skickas
som värde får en egen kopia. `pop` överför elementets ägande. `clear` släpper
elementen och behåller arrayens kapacitet.

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

När ett helt uttryck äger ett temporärt värde kan kompilatorn överföra dess
ägarskap till mottagaren och städa den tidigare ägarplatsen utan att kopiera
bufferten. Detta gäller inte en befintlig lokal eller ett lånat fält eller
element. Värdesemantik och observerbar arraykapacitet behålls. Extra
allokeringar för sådana temporära kopior är inte garanterade; resursbrist
beror på de allokeringar som faktiskt utförs. Uttryckliga kapacitetsfel,
storleksgränser, indexfel och aritmetikfel behåller sina kontroller.

I `text + change(&mut text)` behåller vänsterledet strängen som lästes före
funktionsanropet. Samma utvärderingsordning gäller för argument, index och
snittgränser. En retur bevarar sitt resultat medan funktionen städar sina
lokaler. Ett fatalt runtimefel avslutar processen eller avbryter Wasm-anropet.

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
python3 tools/runtime-contracts.py "$dunder_bin" --target wasm32
```

Testverktyget använder Python 3:s standardbibliotek och Node för Wasm. Det
bygger varje program med den angivna Dunder-kompilatorn. Tio minnesprov håller
bland annat arraykopior, nästlade enumvärden, lånade parametrar, snitt och
strängreturer levande medan andra värden ersätts eller frigörs. Exempelvis
kopierar ett prov en array med 1 024 `i64` i 200 000 varv. Ett annat tar emot
en miljon dynamiska strängreturer. Alla tio kontrollerar resultatet och
kräver under 96 MiB maximal RSS på det nativa värdmålet. På Wasm kräver de
under 96 MiB linjärt minne och samma minnesstorlek efter en andra körning
i samma instans. [Dunder-programmen](../tests/contracts/).

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
        var nodes: []Node = scratch.alloc(32)
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
element eller en array som kopierats genom en delad parameter har egen lagring.
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
    var data: []u8 = []
    data.reserve(8388608)
    for i in 0..8388608 { data.push(u8(i % 256)) }
    print(data[8388607])
}
```
