# Minne och ägarskap

Rust-bootstrapens kodgenerering frigör nu dynamiska arraybuffertar och
strängar under körning. Det gäller dess tre mål: ARM64/macOS, ARM64/Linux
och x86-64/Linux. Modellen behöver ingen extern runtime eller spårande GC.
Den Dunder-skrivna backendens egen runtime har ännu bump-allokering;
självbyggets fixpunkt är inte ett bevis på minnesmodellparitet.

## Värden och ansvar

| Värde | Kopiering | Frigöring |
| --- | --- | --- |
| Dynamisk array | Ny oberoende buffert; strängelement behåller sina strängar. | Elementen släpps, sedan bufferten. |
| Struct, fast array, payload-enum | Fält/element kopieras rekursivt; bara aktiv variant räknas. | Ägda fält/element släpps rekursivt. |
| `string`, `.bytes()`, `.runes()` | Delar oföränderliga data och ökar referensräknaren. | Sista ägaren återlämnar blocket till poolen. |
| Strängliteral | Handtag till programbilden. | Lever hela processen; räknare noll betyder statisk lagring. |
| `&T`, `&mut T` | Lånar anroparens värde enligt den begränsade borrow-kontrollen. | Låntagaren frigör inte anroparens lagring. |

Tilldelning färdigställer det nya värdet innan det gamla släpps. Därför
fungerar självtilldelning och kopior från egna fält. Ett arrayargument som
skickas som värde får en oberoende kopia; använd en borrow för att undvika
kopieringen. `pop` överför strängelementets ägande, medan `clear` släpper
alla element men behåller buffertens kapacitet.

Kompilatorn registrerar ägda stackplatser, nollställer dem före användning
och lägger in städning vid scope-slut, `break`, `continue` och retur.
Återanvända temporärplatser släpper sitt föregående värde. En retur säkrar
resultatets ägande innan funktionens lokala värden städas. Städade platser
nollställs, så gemensam returstädning kan hantera även tidigare avslutade
block. Ett fatalt runtimefel avslutar processen; det kör inte användarkod
eller någon separat destruktoravveckling.

Strängläsningar får också ägda temporärer. I `text + change(&mut text)`
måste vänsterledet överleva ändringen i högerledet. Samma regel gäller
argument, index, snittgränser och iteratorer. Temporärer kan leva till
sitt lexikala scope-slut; detta är ännu ingen analys av sista användningen.

## Varför referensräkning för strängar?

Oföränderliga strängar är löv: de innehåller inga andra ägare och kan inte
bilda cykler. Icke-atomär referensräkning ger säker, billig delning utan
full datakopiering och utan att behålla allt till processlut. Arraybuffertar
har i stället unika ägare. Det ersätter det tidigare, ännu inte genomförda
designmålet att undvika referensräkning helt.

Stränghandtaget pekar fortsatt på `[u64 byte_length, UTF-8 bytes]`.
Referensräknaren ligger åtta byte före handtaget. Nya dynamiska värden
startar med en ägare; null accepteras i intern städning och statiska
literaler har räknare noll. Ett frigjort block markeras för att fånga
dubbel frigöring innan återanvändning. Markeringen ersätter inte korrekt
ägarkontroll. Språket har ännu inget API för delat minne mellan trådar;
räknarna får inte användas som om de vore atomära.

## Pooler och kostnader

Arraypoolen är 512 MiB och strängpoolen 16 MiB, vardera med en extra
4 KiB sida för bokföring. De mappas först vid behov. Det är virtuellt
adressutrymme; resident minne beror på vilka sidor som används.
Frigjorda block återanvänds i storleksklasser med kapaciteter som är
tvåpotenser, från 16 byte. Arrayblock har 16 byte metadata; strängblock
har 32 byte inklusive referensräknare och längd. Poolernas mappningar
återlämnas till operativsystemet vid processlut.

En ledig lista per storleksklass ger konstant arbete vid återanvändning.
Block delas eller sammanfogas ännu inte. En blandning av storlekar kan
därför tömma en pool även om andra klasser har lediga block. Levande data,
avrundning, metadata och fragmentering räknas alla mot tillgängligt utrymme.
Det finns ännu inget obegränsat heap- eller användarallokerings-API.

Arraykopior kostar tid och minne proportionellt mot innehållet. Kopia-före-
frigöring kan tillfälligt behöva plats för både gammalt och nytt värde.
Strängkopior kostar en retain/release, medan sammanslagning och snitt
allokerar och kopierar byte. Tillväxt återlämnar gamla arraybuffertar efter
kopieringen. Runtimefel 109 betyder misslyckad arrayallokering; 105 gäller
strängallokering. Återhämtningsbara sys-operationer kan i stället ge fel 12.

## Verifiering

`tests/memory.rs` testar återvinning genom att allokera långt mer totalt än
poolernas gränser medan bara lite data behöver leva samtidigt. Testerna
kontrollerar resultat och begränsar även samplad Linux-RSS för att fånga en
implementation som bara höjer allokeringsgränsen.

Sviten täcker kopior, tilldelning, retur, callbacks, rekursion, nästlade
structar/enums, strängarrayer, `pop`, borrows, loopavbrott, separata block,
sidverkningar mellan operander och upprepade sys-resultat/felvägar.
ARM64 körs direkt eller med QEMU när tillgängligt. Vulkanprovet kräver
`DUNDER_VULKAN_TEST=1` och återvinner tusentals riktiga readback-snapshots.

```sh
cargo test --test memory -- --test-threads=1
# Med loader och lavapipe konfigurerade enligt linux-vulkan.md:
DUNDER_VULKAN_TEST=1 cargo test --test memory -- --test-threads=1
```

Lokala före/efter-prover på Linux/x86-64 visar varför ändringen behövdes:
100 000 kopior av 1024 `i64` fyllde tidigare 512 MiB-poolen efter omkring
65 000 varv. Med återvinning går hela provet klart med cirka 88 KiB samplad
RSS. 10 000 Vulkan-snapshots på 64×48 pixlar går klart kring 43 MiB inklusive
lavapipe; tidigare tog poolen slut kring 2700 snapshots. RSS-värdena är
sampling från `/proc`, inte exakta kernelrapporterade toppvärden eller
portabla prestandalöften.

En miljon upprepade strängbyggen (`"frame " + i.str()`) respektive snitt
på 60 byte går klart med cirka 64 KiB samplad RSS vardera. Före ändringen
gav samma prover status 105 efter omkring 410 000 respektive 230 000 varv
och hade nått cirka 16 MiB RSS.

Nästa arkitektursteg är att porta samma ägarkontrakt till selfhost-backenden,
förbättra blandade storleksklasser och minska onödiga retain/release samt
aggregatkopior. Generella livstider, explicita flyttar och användarvalda
allokatorer behöver egna språkregler innan de kan exponeras säkert.

## Lexikala scratch-regioner

Rust-bootstrapen kan skapa en tidsbegränsad arena med `region namn(antal_byte)`.
Kapaciteten är ett `i32` som utvärderas en gång vid inträdet. Utan parenteser
är kapaciteten 64 KiB. Regionen lånar en buffert från arraypoolen. Varje
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


Selfhost-kompilatorns native-mål stöder samma syntax och markörallokering
för skalärer och vanliga, flata structar med skalära värdefält. För ett lån
till ett skalärt element eller en hel vy kräver selfhost att elementens
representation matchar funktionsparameterns lagring. Enskilda structfält
kan lånas under ett anrop. `tests/fixtures/regions.ddr` används av båda
kompilatorernas regressionstester.


### Packade bytebuffertar

`[]u8` använder en byte per element. `std.bytes.append_utf8` och
`std.bytes.decode` arbetar med packade buffertar. Tabells CSV-fält, SHA-256
och selfhost-kompilatorns kodbild och signatur använder samma representation.
`sys.write_u8` och `sys.decode_u8` tar `&[]u8`. Befintlig kod med
`&[]i32` kan använda `sys.write_bytes` och `sys.decode_utf8`.

På Apple M5 med macOS 27.2 fyllde ett prov 8 388 608 element och skrev ut
sista värdet 255. Tabellen visar medianen av fem separata processer per
cell med `/usr/bin/time -l` den 10 november 2026.

| Kompilator som byggde provet | Maximal RSS med `[]u8` | Maximal RSS med `[]i32` |
| --- | ---: | ---: |
| Rust-bootstrap för arm64-macos | 9 912 320 byte | 35 078 144 byte |
| Selfhost, steg 3 för arm64-macos | 9 912 320 byte | 35 078 144 byte |

Bygg provet igen med `i32` på båda ställen för fyrabytesvarianten.

```dunder
fn main() {
    var data: []u8 = []
    data.reserve(8388608)
    for i in 0..8388608 { data.push(u8(i % 256)) }
    print(data[8388607])
}
```
