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
        var h: Header = zeroed()
        h.tag = 7
        h.data = cstr("dunder")
        h.length = u32(strlen(h.data))
        assert(h.length == 6)
    }
}
```

## Typ och layout

`u8`, `u16`, `u32` och `u64` upptar 1, 2, 4 respektive 8 byte i en array eller
struct. `usize` och den opaka råpekaren `ptr` följer målets pekarbredd,
8 byte på de tre native-målen och 4 byte på wasm32. En literal kontrolleras
mot sin förväntade typ. Konverteringar som `u8(value)` kontrollerar att värdet
ryms. Index och längd i Dunders befintliga samlings-API har typen `i32`.

`@c_layout` placerar fält i deklarationsordning med naturlig justering och
avrundar totalstorleken till den största fältjusteringen. Fält kan vara tal,
`ptr`, `extern fn`, fasta arrayer av C-skalärer med positiv längd eller andra
`@c_layout`-structar. Ange C-API:ets exakta typer: exempelvis `u32` för ett
32-bitars flaggfält. `size_of(Header)` och `align_of(Header)` ger `usize`
för valt mål. `zeroed()` i en unsafe-kontext skapar en C-struct där alla
fält har värdet noll. Noll kan vara ett ogiltigt handtag för ett visst API.

## Anrop och adresser

`@extern("bibliotek", "symbol") unsafe fn` deklarerar en direkt import.
Funktionsvärdet har typen `extern fn(Parametrar) -> Resultat`. Anrop kontrollerar
antal och typer och kräver en unsafe-kontext även efter tilldelning till en
variabel. Dunder-funktioner och C-funktionspekare har skilda typer. På native
betecknar `"c"` systemets C-bibliotek och `"dl"` systemets dynamiska laddare.
På Wasm anger de två strängarna importmodul och importnamn.

Parametrar och returvärden kan vara skalära tal eller pekare. Native-anrop kan
också överföra en C-struct bestående av en till fyra identiska flyttal (även
nästlade). Skicka andra structar med pekare. Använd en fast signatur för varje
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
`"linux"` och `"macos"` binder ett nåbart anropsställe till respektive system.
Ett modultillstånd kan läggas bakom en privat funktion som alltid returnerar
samma områdesadress. Anroparen ansvarar för samtidighet.

`std.dl.open`, `std.dl.symbol` och `std.dl.close` tar och returnerar C-API:ets
egna råvärden. Kontrollera laddarens resultat. Behåll bibliotekshandtaget så
länge någon kan anropa dess funktioner.

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

## Filåtkomst genom delegerade handtag

En modul med `capability filesystem` kan välja sökvägar med
`std.fs.grant_read(path)` och `std.fs.grant_write(path)`. Därefter kan den
skicka handtagen till moduler som använder `std.fs.read`, `metadata`, `write`,
`append` eller `write_bytes`. Ett läshandtag och ett skrivhandtag är skilda
nominella typer. En `pub opaque struct` kan skickas och kopieras över
modulgränser. Endast modulen som deklarerar den får i säker kod konstruera
ett värde eller läsa, skriva och låna dess fält. Kontrollen av
`@requires(filesystem)` gäller också när modulens kod tar funktionsvärdet.

```dunder
capability filesystem
import std.fs

fn copy(src: &fs.ReadFile, dst: &fs.WriteFile) -> bool {
    result := fs.read(&src)
    result.ok && fs.write(&dst, result.value).ok
}

fn main() {
    input := fs.grant_read("source.txt")
    output := fs.grant_write("copy.txt")
    assert(copy(&input, &output))
}
```

Handtagen innehåller en sökväg. Operativsystemet slår upp den vid varje
operation, inklusive symboliska länkar och relativa sökvägar. Ett handtag
håller ingen öppen filbeskrivare. Modulen som tar emot ett handtag kan
delegera det vidare.

Modulhuvudet styr direkta unsafe-operationer och tillgången till deklarationer
märkta `@requires(filesystem)`. Importer av äldre `sys` ger fortfarande
processens fil- och argumentåtkomst. Grafikbibliotekens publika funktioner
kan skapa fönster och `print` kan skriva till stdout. En importerad funktion
kan själv använda ett privilegium som dess egen modul valt. Bedöm hela
importkedjan för kod som ska köra obetrodda program.
