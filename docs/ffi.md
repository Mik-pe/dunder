# Främmande funktioner, råminne och handtag

Följande exempel använder den typade kompilatorn. Ett modulhuvud anger vilka
privilegier som får användas direkt i filen. Kommentarer och tomrader får
stå före huvudet. Flera privilegier skrivs på samma rad, till exempel
`capability filesystem, unsafe`.

```dunder
capability unsafe

@c_layout
struct Header { tag: u8, length: u32, data: ptr }

@extern("c", "strlen")
unsafe fn strlen(text: ptr) -> usize

fn main() {
    unsafe {
        mut h: Header = zeroed()
        h.tag = 7
        h.data = cstr("dunder")
        h.length = u32(strlen(h.data))
        assert(h.length == 6)
    }
}
```

## Typ och layout

`i16` lagrar ett signerat tvåkomplementsvärde i två byte. Arrayer med `i16`
använder två byte per element. Läsning via `load_i16` och konvertering till
`i32` eller `i64` bevarar tecknet.

`u8`, `u16`, `u32` och `u64` upptar 1, 2, 4 respektive 8 byte i en array eller
struct. `usize` och den opaka råpekaren `ptr` följer målets pekarbredd,
8 byte på de fyra nativa målen (ARM64/macOS, ARM64/Linux, x86-64/Linux och
x86-64/Windows) och 4 byte på wasm32 (båda wasm-målen). En literal kontrolleras
mot sin förväntade typ. Konverteringar som `u8(value)` kontrollerar att värdet
ryms. Index och längd i Dunders befintliga samlings-API har typen `i32`.

`@c_layout` placerar fält i deklarationsordning med naturlig justering och
avrundar totalstorleken till den största fältjusteringen. Fält kan vara tal,
`ptr`, `extern fn`, inbäddade `@c_layout`-structar och fasta arrayer av
dessa ohanterade typer. Fältet `headers: [Header; 2]` har samma layout som
`Header headers[2]` i C. Även i fristående arrayer ligger sådana element med
`size_of(Header)` bytes mellanrum. Skicka `addr(&headers[0])` och antalet
element till C när arrayen har minst ett element.
`string` har Dunders egen ägarmodell och får inte användas som C-fält.
Ange C-API:ets exakta typer, exempelvis `u32` för ett
32-bitars flaggfält. `size_of(Header)` och `align_of(Header)` ger `usize`
för valt mål. `zeroed()` i en unsafe-kontext skapar en C-struct där alla
fält har värdet noll. En fast array kan initialiseras med exempelvis
`mut headers: [Header; 2] = [zeroed(); 2]`. Varje element har egen lagring. Noll kan vara ett ogiltigt handtag för ett visst API.

En C-struct får uppta högst 1 048 575 byte inklusive utfyllnad. Till
exempel ryms ett fält av typen `[u8; 262144]` för ett bildkakel på
256 × 256 RGBA-pixlar. `zeroed()` lägger själva C-värdet på heapen.
Skicka stora structar med pekare och jämför layouten med C:s `sizeof`,
`_Alignof` och `offsetof`. Gränsen för utgående stackargument i ett
enskilt C-anrop är 4 080 byte.

## Anrop och adresser

`@extern("bibliotek", "symbol") unsafe fn` deklarerar en direkt import.
Funktionsvärdet har typen `extern fn(Parametrar) -> Resultat`. Anrop kontrollerar
antal och typer och kräver en unsafe-kontext även efter tilldelning till en
variabel. Dunder-funktioner och C-funktionspekare har skilda typer. På native
betecknar `"c"` systemets C-bibliotek (`libSystem.B.dylib` på macOS,
`libc.so.6` på Linux, `msvcrt.dll` på Windows där även `"msvcrt"` och
`"kernel32"`/`"kernel32.dll"` stöds) och `"dl"` systemets dynamiska laddare
(`libdl.so.2`). På Wasm anger de två strängarna importmodul och importnamn.

En variadisk C-funktion deklareras med `@c_variadic(antal_fasta_parametrar)`.
Varje anrop har fortfarande en fullständigt statisk signatur: deklarera separata
typade alias när samma C-symbol används med olika typer, till exempel libcurl:

```dunder
capability unsafe

@c_variadic(2)
@extern("/usr/lib/libcurl.dylib", "curl_easy_setopt")
unsafe fn setopt_ptr(easy: ptr, option: i32, value: ptr) -> i32
```

Antalet fasta parametrar måste ligga mellan noll och deklarationens antal
parametrar. De återstående parametrarna måste redan ha C:s promoverade typer:
`i32`, `u32`, `i64`, `u64`, `usize`, `ptr`, `f64` eller en C-funktionspekare.
Konvertera `f32` till `f64` och små heltal till `i32` före anropet. Variadiska
aggregat stöds inte. Attributet kräver `@extern unsafe fn` och ett nativt mål.
Gränsen mellan fasta och variadiska parametrar ingår i funktionens typ och
bevaras vid härledd tilldelning (`call := setopt_ptr`); den kan inte tilldelas
en vanlig `extern fn`-typ. Apple ARM64 använder åttabytes stackplatser för
variadiska argument; Linux- och Windows-målen (SysV respektive Win64, med
fyra registerargument och 32 byte skuggrymd) använder respektive plattforms
C-konvention.

Parametrar och returvärden kan vara skalära tal eller pekare. Native-anrop kan
också överföra en C-struct bestående av en till fyra identiska flyttal (även
nästlade). Win64 kan skicka och returnera övriga `@c_layout`-structar
som värden. Structar på 1, 2, 4 eller 8 byte går via heltalsplatser.
Större structar och övriga storlekar överförs via en kopia hos anroparen.
På övriga mål skickas andra structar med pekare. Använd en fast signatur för varje
anrop. `callback(namn)` ger en C-funktionspekare till en namngiven
Dunder-funktion med skalära argument och skalärt eller tomt resultat. En
native-callback lånar det pågående Dunder-anropets exekveringstillstånd.
C-biblioteket får anropa den synkront på samma OS-tråd medan anropet från
Dunder till C fortfarande pågår. Om ett C-bibliotek anropar funktionen från
en annan tråd eller utan ett aktivt anrop avslutas processen med kod 111.

Den osäkra primitiven `static_bytes("native", 16)` ger adressen till ett
16-byte-justerat och initialt nollställt område i programmets skrivbara
datasegment. Varje anropsställe äger ett eget område som lever tills processen
avslutas. Längden är en heltalsliteral mellan en byte och 16 MiB.
`"linux"`, `"macos"` och `"windows"` binder ett nåbart anropsställe till
respektive system. Ett modultillstånd kan läggas bakom en privat funktion som
alltid returnerar samma områdesadress. Anroparen ansvarar för samtidighet.

För plattformsspecifik kod som väljs statiskt finns compile-time-uttrycket
`target_is("...")` i `if`/`else if`-villkor (med `!`, `&&`, `||` och
parenteser), där giltiga namn är `"macos"`, `"arm64-macos"`,
`"aarch64-apple-darwin"`, `"linux"`, `"arm64-linux"`,
`"aarch64-unknown-linux-gnu"`, `"x86_64-linux"`,
`"x86_64-unknown-linux-gnu"`, `"windows"`, `"x86_64-windows"`,
`"x86_64-pc-windows-msvc"`, `"arm64"`, `"x86_64"`, `"native"`, `"wasm32"`,
`"wasm32-unknown-unknown"`, `"wasi"`, `"wasm32-wasi"` och `"wasm32-wasip1"`
(`"x86_64"` täcker även Windows och `"wasm32"` täcker även wasi). Onåbara
grenar hoppas över före typkontroll
och importregistrering, så `@extern`-anrop i en `target_is("macos")`-gren
skapar inga dynamiska beroenden vid Linux-kompilering.

`std.dl.open`, `std.dl.symbol` och `std.dl.close` tar och returnerar C-API:ets
egna råvärden. Kontrollera laddarens resultat. Behåll bibliotekshandtaget så
länge någon kan anropa dess funktioner.

På Windows tar `open` en nollavslutad UTF-8-sökväg och laddar biblioteket
med `LoadLibraryW`. Flaggvärdena 1 och 2 stöds. En nullpekare som sökväg
hämtar en ägd referens till processens egen modul. Matcha varje lyckat
`open` med ett `close`. Symbolnamnet är exportens exakta namn.

```dunder
capability unsafe
import std.dl

unsafe fn lookup_strlen(library: ptr) -> usize {
    address := dl.symbol(library, cstr("strlen"))
    if address == ptr(0) { return 0 }
    length: extern fn(ptr) -> usize = ffi_fn(address)
    length(cstr("dunder"))
}
```

En C-strängliteral `cstr("text")` har statisk livstid, UTF-8-innehåll och en
avslutande nollbyte. Inbäddad NUL avvisas. För dynamisk text kan anroparen
bygga en `[]u8` med avslutande nolla och skicka `addr(&bytes[0])`. Behåll
bufferten levande och på samma adress under hela lånet. `addr(&place)` och
`addr(&mut place)` tar adressen till ett lagrat värde. Ett arrayelement har
en kontrollerad indexberäkning. `addr(&buf)` på en dynamisk array ger adressen
till Dunders arraybeskrivare.

`load_u16(p)` läser två byte. `store_u16(p, v)` skriver två byte som en sats.
Motsvarande operationer finns för de fem osignerade heltalstyperna, `i32`,
`i64`, `f32`, `f64` och `ptr`. `usize(p)` ger adressens bitar och
`ptr(address)` tar en `usize`. `ref_at(p)` överlämnar ett rått värde som en
`&T`- eller `&mut T`-parameter med förväntad typ. Den som skriver unsafe-koden
ansvarar för livstid, åtkomst, storlek, justering och aliasregler. Ett främmande
funktionsanrop förutsätter dessutom att bibliotekets faktiska ABI stämmer med
deklarationen.

### Objektminne och privata stackceller

Rå åtkomst måste avse ett levande, åtkomligt källobjekt eller en uttryckligen
tillhandahållen extern minnesregion och rymmas inom dess giltiga intervall.
Pekarens bitar är dess representation. En rundresa genom `usize` bevarar
åtkomsten till samma region; ett numeriskt värde ger ingen ny rätt att
läsa eller skriva andra objekt. Utgångna lån, gamla pekare och aritmetik
utanför regionen ger ingen åtkomst till andra lokala värden.

Kompilatorns spillceller, oexponerade lokaler och FP/LR-länkage är privat
implementationslagring. Ett vanligt främmande anrop eller en Wasm-värd får
varken läsa eller skriva dessa genom stackvandring eller tillverkade
adresser. Ett uttryckligt `addr(&place)` exponerar det angivna objektet
enligt dess typ, storlek, livstid och lånevillkor. Det exponerar inte andra
lokaler, dold utfyllnad eller resten av framen. Kompilatorn kan därför
behålla oexponerade värden i register även när andra objekt ändras.

Rekursion och återanrop skapar separata aktiveringar. En pekare till ett
äldre exponerat objekt behåller sitt giltiga lån och ger ingen åtkomst
till en ny aktiverings privata celler. Registerbevarande och denna
minnesregel är separata ABI-egenskaper.

Vanliga anrop returnerar normalt en gång eller avslutar utan återkomst.
Stackbyte, icke-lokala hopp, returns-twice och stackinspektion av privata
lokaler kräver ett separat uttryckligt kontrakt; de följer inte av en
vanlig `@extern`-deklaration. Dessa regler gäller också när en främmande
funktion anropar Dunder genom en callback.

Ett synkront C-anrop som bara läser under anropet kan få ett lån via
ett säkert omslag. Här är C-funktionens kontrakt att den läser exakt
`length` byte och accepterar null när längden är noll.

```dunder
@extern("reader", "sum_bytes") unsafe fn sum_bytes(data: ptr, length: usize) -> usize

fn sum(bytes: &[]u8) -> usize {
    unsafe {
        if bytes.len == 0 { return sum_bytes(ptr(0), usize(0)) }
        return sum_bytes(addr(&bytes[0]), usize(bytes.len))
    }
}
```

Om C-biblioteket sparar data till ett senare anrop behöver bindningen
följa bibliotekets ägarkontrakt. Ett API som kopierar under anropet kan
använda samma lån. Kopian lever tills den släpps med bibliotekets
frigöringsfunktion. Ett API som behåller själva pekaren kräver att
bindningen håller originalets lagring vid liv och på samma adress tills
biblioteket kvitterar avslut. En `ptr` från `addr` förlänger inte lånet.
Ett sådant handtag kan äga en separat stabil allokering och avregistrera
den från C innan det frigör minnet.

För säkra omslag ovanpå råpekare (till exempel i `std.alloc`) skapar
`unsafe bytes_view(&owner, base, length)` en lånad vy `&[]u8` med angiven
längd av typen `usize` över `[base, base + length)` och binder vyns livstid
till den delade referensen `&owner` (den kan också kombineras med `-> &[]u8
from self`). Vid positiv längd måste `base` vara skild från null. Dessutom kan
traits och `impl Trait for Type` deklarera `unsafe fn`-metoder, som i
`std.alloc.RawAllocator`, med krav på att implementationen matchar
säkerhetsnivån i traiten.

## Filåtkomst genom delegerade handtag

En modul med `capability filesystem` kan välja sökvägar med
`fs.ReadFile::grant(path)` och `fs.WriteFile::grant(path)`. Därefter kan den
skicka handtagen till moduler som använder metoderna `.read`, `.metadata`, `.write`,
`.append` eller `.write_bytes`. Ett läshandtag och ett skrivhandtag är skilda
nominella typer. En `pub opaque struct` kan skickas över modulgränser och
följer samma ägarskapsregel som andra structar: handtag som innehåller
strängar flyttas, och en extra ägare kräver `.clone()`.
Endast modulen som deklarerar den får i säker kod konstruera
ett värde eller läsa, skriva och låna dess fält. Kontrollen av
`@requires(filesystem)` gäller också när modulens kod tar funktionsvärdet.

```dunder
capability filesystem
import std.fs
import sys

fn copy(src: &fs.ReadFile, dst: &fs.WriteFile) -> Result<(), sys.IoError> {
    text := src.read()?
    dst.write(text)
}

fn main() {
    input := fs.ReadFile::grant("source.txt")
    output := fs.WriteFile::grant("copy.txt")
    assert(copy(&input, &output).is_ok())
}
```

Handtagen innehåller en sökväg. Operativsystemet slår upp den vid varje
operation, inklusive symboliska länkar och relativa sökvägar. Ett handtag som
`fs.ReadFile` eller `fs.WriteFile` håller ingen öppen filbeskrivare; via
`ReadFile::open()`, `WriteFile::create()`, `WriteFile::open_append()` eller
`fs.File::open(path, OpenOptions)` kan det däremot öppnas till strömmande
`@resource(dispose)`-handtag (`FileReader`, `FileWriter`, `File`). Modulen som
tar emot ett handtag kan delegera det vidare.

Modulhuvudet styr direkta unsafe-operationer samt deklarationer märkta
`@requires(filesystem)`, `@requires(network)`, `@requires(process)` eller
`@requires(stdio)`. Flera kapabiliteter anges kommaseparerat i samma modulhuvud,
t.ex. `capability process, network, filesystem, stdio`.
`fs.ReadDir::grant(path)` och `dir.resolve_read(rel_path)` avgränsar
relativ läsning till en vald rotkatalog utan `..`-flykt, `std.net.listen` /
`std.net.connect` kräver `capability network`, `std.io.stdin` / `stdout` /
`stderr` / `read_stdin` kräver `capability stdio`, och `std.process.run` /
`exec` / `spawn` / `grant` kräver `capability process` (där `Executable`
delegerar rätten att köra ett specifikt program till oprivilegierade moduler).
Importer av äldre `sys` ger fortfarande processens OS-åtkomst. Bedöm hela
importkedjan för kod som ska köra obetrodda program.
