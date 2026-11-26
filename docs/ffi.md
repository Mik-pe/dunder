# Främmande funktioner (FFI), systemlänkning och råminne

Detta dokument är den auktoritativa specifikationen för Dunders FFI-språkyta,
ABI-kontrakt, säkerhets- och kapabilitetsmodell, manifeststyrda länkpolicy i
`dunder.toml` samt de inbyggda binäremittrarna (Mach-O, ELF64, PE32+ och
WebAssembly).

Dunder äger hela verktygskedjan från `.ddr` till körbar binär utan extern
C-kompilator, assembler, länkare, plattforms-SDK, `.tbd`-stubbar eller
`.lib`-importbibliotek. System- och grafik-API:er som Metal, Cocoa/AppKit,
CoreGraphics, QuartzCore, Vulkan, Wayland, X11, DirectX 12, DXGI, Win32 och
COM är dynamiskt länkade vid operativsystemets ABI-gräns och emitteras direkt
av kompilatorns inbyggda bildskrivare — inklusive offline-korskompilering.

```dunder
capability unsafe
import std.ffi as ffi

@link("c")
extern "C" {
    fn strlen(text: *const ffi.c_char) -> usize
}

fn main() {
    unsafe {
        len := strlen(c"dunder".as_ptr())
        assert(len == 6)
    }
}
```

---

## 1. Språkyta och typsystem

### 1.1 Primitiva FFI-typer och `std.ffi`

Följande primitiva skalärtyper får passera en FFI-gräns och lagras i
`@repr(C)`-typer:

| Dunder-typ | Storlek / ABI | Anmärkning |
| --- | --- | --- |
| `i8`, `u8` | 1 byte | Signerat respektive osignerat heltal |
| `i16`, `u16` | 2 byte | Tvåkomplement respektive osignerat |
| `i32`, `u32` | 4 byte | Fast bredd |
| `i64`, `u64` | 8 byte | Fast bredd |
| `isize`, `usize` | Målets pekarbredd | 8 byte på 64-bitars nativa mål, 4 byte på `wasm32`/`wasm32-wasi` |
| `f32`, `f64` | 4 respektive 8 byte | IEEE 754 binary32 / binary64 |
| `bool` | 1 byte | Motsvarar C `_Bool` (`0` = `false`, `1` = `true`); utgående värden är alltid `0` eller `1`, inkommande icke-noll-byte tolkas som `true` |

Modulen `std.ffi` exporterar plattformsanpassade C-typalias och opaka
`void`-typer som följer målplattformens C-ABI:

| Alias i `std.ffi` | `arm64-macos` | `arm64-linux` | `x86_64-linux` | `x86_64-windows` | `wasm32` / `wasi` |
| --- | --- | --- | --- | --- | --- |
| `c_char` | `i8` | `u8` | `i8` | `i8` | `i8` |
| `c_schar` / `c_uchar` | `i8` / `u8` | `i8` / `u8` | `i8` / `u8` | `i8` / `u8` | `i8` / `u8` |
| `c_short` / `c_ushort` | `i16` / `u16` | `i16` / `u16` | `i16` / `u16` | `i16` / `u16` | `i16` / `u16` |
| `c_int` / `c_uint` | `i32` / `u32` | `i32` / `u32` | `i32` / `u32` | `i32` / `u32` | `i32` / `u32` |
| `c_long` / `c_ulong` | `i64` / `u64` | `i64` / `u64` | `i64` / `u64` | `i32` / `u32` | `i32` / `u32` |
| `c_longlong` / `c_ulonglong` | `i64` / `u64` | `i64` / `u64` | `i64` / `u64` | `i64` / `u64` | `i64` / `u64` |
| `c_void` | `extern type` | `extern type` | `extern type` | `extern type` | `extern type` |

Notera skillnaden mellan **LP64** (macOS och Linux, där `c_long` är 64 bitar)
och **LLP64** (Windows, där `c_long` är 32 bitar), samt att `char` är osignerat
på ARM64/Linux. Använd alltid `std.ffi`-aliasen när en C-signatur anger `long`
eller `char`, och explicita bredder (`u32`, `i64` m.fl.) när C-API:et anger
`uint32_t`, `DWORD`, `HRESULT` osv.

### 1.2 Typade råpekare (`*const T` och `*mut T`)

Dunder använder två skilda typade råpekartyper:

- `*const T` — skrivskyddad råpekare till ett värde av typen `T`.
- `*mut T` — skrivbar råpekare till ett värde av typen `T`.

Råpekare är `Copy`, har målets pekarbredd (`size_of(*const T) == size_of(usize)`)
och tillåter null. För generiska `void*`-pekare används `*const ffi.c_void`
respektive `*mut ffi.c_void`.

Skapande och konvertering:

- `&place` konverteras till `*const T` via `addr_of(&place)` (kräver `unsafe`-kontext).
- `&mut place` konverteras till `*mut T` via `addr_of(&mut place)` (kräver `unsafe`-kontext).
- Nollpekare skapas säkert med `ffi.null<T>() -> *const T` och
  `ffi.null_mut<T>() -> *mut T`.
- Adresskonvertering: `p.addr() -> usize` (säker), `ffi.from_addr<T>(a: usize) -> *const T`
  och `ffi.from_addr_mut<T>(a: usize) -> *mut T` (kräver `unsafe`).

Metoder på `*const T` och `*mut T`:

| Metod | Säkerhet | Beteende |
| --- | --- | --- |
| `p.is_null() -> bool` | Säker | Sant exakt när adressen är `0`. |
| `p.addr() -> usize` | Säker | Returnerar pekarens bitmönster. |
| `p.cast<U>() -> *const U` / `*mut U` | Säker | Byter elementtyp med bevarad konst-/mut-kvalificering. |
| `p.cast_const() -> *const T` | Säker | Nedgraderar `*mut T` till `*const T`. |
| `p.cast_mut() -> *mut T` | Säker | Konverterar `*const T` till `*mut T` (skrivning kräver fortsatt `unsafe`). |
| `p.add(n: usize) -> Self` | `unsafe` | Förskjuter pekaren `n * size_of(T)` byte framåt inom samma allokering. |
| `p.sub(n: usize) -> Self` | `unsafe` | Förskjuter pekaren `n * size_of(T)` byte bakåt inom samma allokering. |
| `p.offset(n: isize) -> Self` | `unsafe` | Signerad elementförskjutning med `n * size_of(T)` byte. |
| `p.byte_add(n: usize)` / `.byte_sub(n: usize)` | `unsafe` | Bytevis förskjutning utan skalning med `size_of(T)`. |
| `p.read() -> T` | `unsafe` | Läser ett justerat värde av typen `T` (`T` måste vara `Copy` eller FFI-värdetyp). |
| `p.write(val: T)` (`*mut T`) | `unsafe` | Skriver ett justerat värde av typen `T`. |
| `p.read_unaligned() -> T` / `.write_unaligned(val: T)` | `unsafe` | Läser/skriver ojusterat värde (t.ex. från packade nätverks- eller filbuffertar). |
| `p.as_ref(&owner) -> Option<&T> from owner` | `unsafe` | Returnerar `.None` vid null; binder annars en delad referens `&T` till ägaren `&owner`. |
| `p.as_mut(&mut owner) -> Option<&mut T> from owner` (`*mut T`) | `unsafe` | Returnerar `.None` vid null; binder annars en exklusiv referens `&mut T` till `&mut owner`. |

För lånade bytevyer över sammanhängande minnesintervall finns den ägarbundna
primitiven:

- `unsafe bytes_view(&owner, base, len: usize) -> &[]u8 from owner`

Vid positiv längd måste `base` vara skild från null och hela intervallet
`[base, base + len)` måste vara giltigt och orört under `&owner`-lånets hela
livstid.

`==` och `!=` jämför två pekare av samma typ efter adress. Ett `addr_of(&buf)`
på en dynamisk array `[]T` pekar på Dunders arraybeskrivare; för
elementbufferten skickas `buf.as_ptr()` respektive `buf.as_mut_ptr()` (eller
`addr_of(&buf[0])` när `buf.len > 0`).

### 1.3 Opaka FFI-typer (`extern type`)

För nominellt skilda, nollkostnadshandtag deklareras opaka främmande typer med
`extern type`:

```dunder
pub extern type VkInstance_T
pub extern type VkDevice_T
pub extern type HWND_T
pub extern type MTLDevice
```

Regler för `extern type`:

- En `extern type` har okänd storlek vid kompilering och får aldrig förekomma
  som värde, fält i en struct/union, returtyp per värde eller argument till
  `size_of`, `align_of`, `.add()`, `.sub()`, `.read()` eller `.write()`.
- Den används uteslutande bakom råpekare (`*const T`, `*mut T`), exempelvis
  `type VkDevice = *mut VkDevice_T`.
- Två olika `extern type`-deklarationer är nominellt skilda typer: `*mut VkDevice_T`
  kan aldrig av misstag skickas där `*mut VkInstance_T` förväntas.
- För säkra, ägda handtag paketeras pekaren i en `@resource(dispose) pub opaque struct`.

### 1.4 Layoutattribut: `@repr(C)`, `struct`, `union` och öppna C-enums

#### `@repr(C)` på `struct`

`@repr(C)` placerar fält i deklarationsordning enligt målets C-ABI med
naturlig fältjustering och avrundar totalstorleken uppåt till den största
fältjusteringen:

```dunder
@repr(C)
struct Header {
    tag: u8,
    length: u32,
    data: *const u8,
}
```

Modifierare som kan kombineras med `C`:

- `@repr(C, packed)` — sätter fältjustering till 1 byte (ingen utfyllnad mellan
  fält eller i slutet).
- `@repr(C, packed(N))` — begränsar varje fälts justering till högst `N`
  (`N` är en tvåpotens: `1, 2, 4, 8, 16, ...`).
- `@repr(C, align(N))` — höjer typens minsta justering till minst `N`
  (`N` är en tvåpotens) och avrundar totalstorleken till en multipel av `N`
  (används för exempelvis 16-bytejusterade SIMD-, Metal- och DirectX-typer).
- `packed` och `align` får inte kombineras i samma attribut.
- På en `packed`-typ avvisar kompilatorn att ta en direkt referens (`&s.field`,
  `&mut s.field`) till ett fält vars justering kan understiga fälttypens
  naturliga justering; fältet läses eller skrivs istället per värde (kopia)
  eller via `addr_of(&s)` + `read_unaligned()`/`write_unaligned()`.

Tillåtna fälttyper i `@repr(C)`-structar och -unioner:

- Primitiva FFI-typer (`i8..i64`, `u8..u64`, `isize`, `usize`, `f32`, `f64`, `bool`).
- Typade råpekare (`*const T`, `*mut T`).
- Externa funktionspekare (`extern "<abi>" fn(...) -> ...`) samt nullbara
  `Option<extern "<abi>" fn(...) -> ...>`.
- Öppna C-enums märkta `@repr(<heltal>)`.
- Nästlade `@repr(C)`-structar och `@repr(C)`-unioner.
- Fasta arrayer `[T; N]` av ovanstående typer (elementen ligger tätt med
  `size_of(T)` bytes mellanrum, exakt som `T field[N]` i C).

Hanterade Dunder-typer (`string`, `[]T`, `Map`, `Set`, fångande closures,
slutna Dunder-enums och `@resource`-typer) är förbjudna i `@repr(C)`-typer.
`size_of(T) -> usize`, `align_of(T) -> usize` och `offset_of(T, field) -> usize`
utvärderas vid kompilering för valt mål.

#### Stackallokering och partiell nollinitiering

`@repr(C)`-structar och -unioner är värdetyper som lagras **direkt på stacken**
(utan tvungen heapallokering vid literalkonstruktion, `zeroed()` eller retur
från externa anrop). Stora structar bör av prestandaskäl fortfarande skickas
med pekare till C-API:er som accepterar pekare.

- `ffi.zeroed<T>() -> T` (eller kontexttypad `zeroed()`, kräver `unsafe`) konstruerar ett värde på stacken
  med alla `size_of(T)` byte — inklusive all utfyllnad — satta till `0`.
- Partiell nollinitiering tillåts på `@repr(C)`-structar i `unsafe`-kontext:
  ```dunder
  unsafe {
      info := VkInstanceCreateInfo {
          sType: .InstanceCreateInfo,
          pApplicationInfo: addr_of(&app_info),
          ..zeroed()
      }
  }
  ```
  Alla utelämnade fält samt all dold utfyllnad mellan och efter fält
  nollställs deterministiskt, vilket eliminerar behovet av att manuellt lista
  dussintals nollfält i Vulkan-, Win32- och DirectX-beskrivare.

#### `@repr(C)` på `union`

En `union` lagrar alla sina fält överlappande på byteoffset `0`. Dess
justering är den största justeringen bland fälten och dess storlek är den
största fältstorleken avrundad uppåt till justeringen.

```dunder
@repr(C)
union VkClearColorValue {
    float32: [f32; 4],
    int32: [i32; 4],
    uint32: [u32; 4],
}
```

Regler för `union`:

- Alla `union`-deklarationer måste ha `@repr(C)` (eventuellt med `packed`
  eller `align(N)`).
- Endast FFI-kompatibla värdetyper får vara fält (inga ägda/resursbärande
  typer).
- Konstruktion med ett fält (`VkClearColorValue { float32: [0.0, 0.0, 0.0, 1.0] }`)
  nollställer eventuella resterande byte utöver det valda fältet så att inga
  oinitierade utfyllnadsbyte läcker över FFI.
- Tilldelning till ett fält (`u.uint32 = [1, 2, 3, 4]`) kräver inte `unsafe`,
  men **varje läsning eller lån av ett unionfält kräver `unsafe`**.

#### Öppna C-ABI-enums (`@repr(<heltal>)`)

En fältlös `enum` kan märkas med en explicit heltalstyp för direkt
C-ABI-representation:

`@repr(u8)`, `@repr(i8)`, `@repr(u16)`, `@repr(i16)`, `@repr(u32)`,
`@repr(i32)`, `@repr(u64)`, `@repr(i64)`, `@repr(usize)`, `@repr(isize)`.

```dunder
@repr(i32)
pub enum VkResult {
    Success = 0,
    NotReady = 1,
    Timeout = 2,
    SuboptimalKhr = 1000001003,
    ErrorOutOfHostMemory = -1,
    ErrorOutOfDateKhr = -1000001004,
}
```

Kontrakt för `@repr(<heltal>)`-enums:

- Storlek, justering och anropskonvention är identiska med den angivna
  heltalstypen. Typen är tillåten direkt i `extern`-signaturer och i
  `@repr(C)`-structar och -unioner.
- **Öppen semantik utan runtimefälla**: till skillnad från slutna Dunder-enums
  (som fäller med status 108 för okända värden) kan en `@repr(<heltal>)`-enum
  bära **vilket bitmönster som helst** i sin underliggande heltalstyp. En ny
  returkod eller flagga från en grafikdrivrutin eller framtida OS-version
  orsakar därför aldrig en krasch vid FFI-gränsen.
- Inbyggda metoder på varje `@repr(Int)`-enum `E`:
  - `E::from_raw(raw: Int) -> E` — ofelbar konstruktion från valfritt `Int`-värde.
  - `E::try_from_raw(raw: Int) -> Option<E>` — returnerar `.Some(e)` endast om
    `raw` matchar en namngiven variant, annars `.None`.
  - `e.to_raw() -> Int` — returnerar det underliggande råa heltalsvärdet.
- Eftersom typen är öppen kräver varje `match` på en `@repr(<heltal>)`-enum en
  avslutande jokerarm (`_ => ...`).

### 1.5 C-strängar och UTF-16-strängar (`CStr`/`CString`, `WStr`/`WString`)

För nollkostnads strängöverföring till POSIX-, C-, ObjC- och Win32-API:er
tillhandahåller språket och `std.ffi` fyra kompletterande typer:

- **`c"..."` -> `&ffi.CStr`**: statisk, NUL-avslutad UTF-8-sträng i
  skrivskyddat datautrymme. Inbäddad NUL (`\0`) avvisas vid kompilering.
  - `.as_ptr() -> *const ffi.c_char`
  - `.as_bytes() -> &[]u8` (utan avslutande NUL)
  - `.as_bytes_with_nul() -> &[]u8` (inklusive avslutande NUL)
  - `.len() -> usize` (antal byte före NUL)
- **`w"..."` -> `&ffi.WStr`**: statisk, NUL-avslutad UTF-16LE-sträng
  (kompilerad som konstant `[u16; N]`). Inbäddad NUL avvisas vid kompilering.
  Eliminerar dynamisk UTF-8 -> UTF-16-konvertering för konstanta Win32-namn
  som fönsterklasser och DLL-sökvägar.
  - `.as_ptr() -> *const u16`
  - `.as_units() -> &[]u16` (utan avslutande NUL)
  - `.as_units_with_nul() -> &[]u16` (inklusive avslutande NUL)
  - `.len() -> usize` (antal UTF-16-kodenheter före NUL)
- **`ffi.CString`**: ägd, heapallokerad, NUL-avslutad UTF-8-sträng.
  - `CString::new(text: string) -> Result<CString, NulError>` och
    `CString::from_bytes(data: &[]u8) -> Result<CString, NulError>` (avvisar inbäddad NUL).
  - `.as_ptr()`, `.as_bytes() -> &[]u8`, `.as_bytes_with_nul() -> &[]u8`,
    `.len() -> i32`, `.is_empty() -> bool`, `.to_string() -> Result<string, sys.IoError>`.
- **`ffi.WString`**: ägd, NUL-avslutad UTF-16LE-sträng för dynamiska
  Win32 `*W`-anrop.
  - `WString::new(text: string) -> Result<WString, NulError>` och
    `WString::from_units(data: &[]u16) -> Result<WString, NulError>` (avvisar inbäddad NUL).
  - `.as_ptr()`, `.as_units_with_nul() -> &[]u16`, `.len() -> i32`,
    `.is_empty() -> bool`, `.to_string() -> Option<string>`.
- **Inläsning från främmande strängpekare och OS-felkoder** (alla null-säkra):
  - `unsafe ffi.c_strlen(p)` / `unsafe ffi.c_strnlen(p, limit)`
  - `unsafe ffi.from_cstr(p) -> Option<string>` (validerar UTF-8)
  - `unsafe ffi.from_cstr_bounded(p, max_bytes) -> Option<string>`
  - `unsafe ffi.from_cstr_lossy(p) -> Option<string>`
  - `unsafe ffi.c_wstrlen(p)` / `unsafe ffi.from_wstr(p) -> Option<string>` /
    `unsafe ffi.from_wstr_lossy(p) -> Option<string>` (avkodar UTF-16LE)
  - `unsafe ffi.errno() -> i32`, `unsafe ffi.set_errno(value: i32)`,
    `unsafe ffi.last_os_error() -> i32`

### 1.6 Nullbara funktionspekare (NPO)

En främmande funktionspekartyp `extern "<abi>" fn(Parametrar) -> Resultat` är
garanterat **icke-null**. Dunder-funktionspekare `fn(...)` och
`extern "<abi>" fn(...)` är skilda typer och kan inte blandas implicit.

`Option<extern "<abi>" fn(Parametrar) -> Resultat>` omfattas av
**nollpekaroptimering (NPO)**:

- Den har exakt samma storlek, justering och anropskonvention som en rå
  funktionspekare (`size_of(usize)`).
- `.None` representeras av adressen `0` (null); `.Some(f)` representeras av
  funktionsadressen direkt utan taggbyte.
- Typen får användas direkt som fält i `@repr(C)`-structar och -unioner samt
  som parameter och returtyp i `extern`-funktioner (t.ex. valfria callbacks i
  `VkAllocationCallbacks`, `wl_registry_listener` och `WNDCLASSEXW`).

---

## 2. Externa deklarationer, anropskonventioner, globala symboler och callbacks

### 2.1 Grupperade `extern "<abi>"`-block och `@link(...)`

Främmande funktioner och datasymboler deklareras i grupperade `extern`-block
kopplade till en **logisk biblioteksnyckel** via `@link(...)`:

```dunder
capability unsafe
import std.ffi as ffi

@link("c")
extern "C" {
    fn malloc(size: usize) -> *mut ffi.c_void
    fn free(p: *mut ffi.c_void)

    @link_name("snprintf")
    fn c_snprintf(buf: *mut ffi.c_char, cap: usize, fmt: *const ffi.c_char, ...) -> ffi.c_int
}
```

#### Stödda anropskonventioner (`<abi>`)

| Konvention | Betydelse och mål-ABI |
| --- | --- |
| `"C"` | Målets standard-C-ABI: AAPCS64 på `arm64-macos`/`arm64-linux` (inkl. Apple-specifik stackpackning och variadik-på-stack på Darwin), full System V AMD64-klassificering (`INTEGER`/`SSE`/`MEMORY` för 1–16 byte samt stackkopia/`sret` för >16 byte) på `x86_64-linux`, Win64 på `x86_64-windows`, Wasm-import-ABI på `wasm32`/`wasm32-wasi`. |
| `"system"` | Operativsystemets API-konvention: Win64 på `x86_64-windows`, `"C"` på övriga 64-bitars mål (och `stdcall` om 32-bitars x86-Windows någonsin införs). Rekommenderas för Win32 och plattformsoberoende systemanrop. |
| `"win64"` | Uttrycklig Microsoft x64-anropskonvention: fyra registerargument (`RCX`/`RDX`/`R8`/`R9` eller `XMM0..XMM3`), 32 byte shadow space allokerat av anroparen, 1/2/4/8-bytes structar i GPR och övriga storlekar via anroparkopia/pekare, `XMM6..XMM15` callee-saved. |
| `"sysv64"` | Uttrycklig System V AMD64-anropskonvention: sex GPR (`RDI`/`RSI`/`RDX`/`RCX`/`R8`/`R9`), åtta XMM (`XMM0..XMM7`), full 8-bytesklassificering, 16-byte stackjustering före `call`, `%al` satt till antal XMM-register vid variadiska anrop. |
| `"msvc-com"` | Microsoft x64 C++ COM-medlemsfunktions-ABI (DirectX 12, DXGI, DirectWrite, WIC). För metoder som returnerar en struct per värde — **även 1/2/4/8-bytesstructar** som `D3D12_CPU_DESCRIPTOR_HANDLE` (`8` byte) från `GetCPUDescriptorHandleForHeapStart` — skickas `this` i `RCX` och en dold returpekare (`sret`) i **`RDX`**, övriga parametrar från `R8`/`R9`/stack, och funktionen returnerar `sret`-pekaren i `RAX`. (I vanligt Win64 C-ABI returneras en 8-bytesstruct direkt i `RAX`, vilket kraschar COM-anrop.) |

#### `@link(...)`-attributet

`@link(...)` fästs på ett `extern "<abi>" { ... }`-block:

- `@link("<nyckel>")` — anger logisk biblioteksnyckel (t.ex. `"c"`, `"objc"`,
  `"metal"`, `"vulkan"`, `"wayland-client"`, `"d3d12"`, `"user32"`). På
  `wasm32`/`wasm32-wasi` anger strängen Wasm-importmodulens namn (t.ex.
  `"wasi_snapshot_preview1"`).
- `@link(macos = "<lib>", linux = "<lib>", windows = "<lib>")` — anger
  plattformsspecifika biblioteksnycklar eller kanoniska biblioteksnamn direkt
  i samma block (t.ex. `@link(macos = "/usr/lib/libsqlite3.dylib", linux = "libsqlite3.so.0")`).
- `@link(framework = "<Namn>")` — macOS-ramverkskortform (motsvarar nyckeln
  `framework:<Namn>`, som löses till
  `/System/Library/Frameworks/<Namn>.framework/Versions/<A|B|C>/<Namn>` ur dyld
  shared cache om inget annat anges i `dunder.toml`).
- Valfria namngivna parametrar i `@link(...)`:
  - `weak = true` — mjuk biblioteks- och symbolbindning (`LC_LOAD_WEAK_DYLIB`
    + `BIND_SYMBOL_FLAGS_WEAK_IMPORT` på Mach-O, `STB_WEAK` på ELF64).
    Används för valfria nyare OS-ramverk (t.ex. `MetalFX`) eller frivilliga
    symboler.
  - `delay = true` — fördröjd DLL-laddning på Windows via PE32+ Delay-Load
    Import Directory (DataDir 13) så att avsaknad av exempelvis `d3d12.dll`
    eller `d3dcompiler_47.dll` inte stoppar programstarten.
  - `version = "<ver>"` — sätter ELF-symbolversion (t.ex. `"GLIBC_2.17"`) för
    samtliga symboler i blocket.

#### Medlemsattribut, variadiker och externa datasymboler (`static`)

Inne i ett `extern "<abi>" { ... }`-block kan varje deklaration ha:

- `pub` — exporterar symbolen från modulen.
- `@link_name("<symbol>")` — binder Dunder-namnet till ett annat binärt
  exportnamn (t.ex. flera typade alias för `objc_msgSend` eller
  `curl_easy_setopt`).
- `@link_ordinal(N)` — binder med Windows PE-ordinalnummer i stället för namn
  (`1 <= N <= 65535`).
- `@link_version("<ver>")` — explicit ELF-symbolversion för enskild symbol
  (`DT_VERSYM`/`DT_VERNEED`).
- `@weak` — markerar en enskild symbol som svag även i ett normalt block.
- `@thread_safe` — markerar att funktionen får anropas från isolerade `task`-arbetare.
- `@requires(filesystem | network | process | stdio)` — kopplar kapabilitetskrav
  till externa funktioner precis som vanliga Dunder-funktioner.

**Funktioner i `extern`-block** är alltid osäkra att anropa (anrop kräver
`unsafe`-kontext). Nyckelordet `unsafe` skrivs därför inte framför `fn` inne i
blocket.

**Variadiska funktioner** skrivs med `...` sist i parameterlistan efter minst
en fast parameter:

```dunder
@link("curl")
extern "C" {
    fn curl_easy_setopt(easy: *mut CURL, option: ffi.c_int, ...) -> ffi.c_int
}
```

Vid anrop till en variadisk funktion gäller C:s standardregler för promovering:
argument efter den sista fasta parametern måste vara C-promoverade skalärer
(`i32`, `u32`, `i64`, `u64`, `isize`, `usize`, `*const T`, `*mut T`, `f64`,
`extern fn` eller `Option<extern fn>`). Mindre heltal (`i8`, `u8`, `i16`,
`u16`, `bool`) och `f32` konverteras uttryckligen av anroparen till `i32`/`u32`
respektive `f64` (kompilatorn ger diagnostik om ett opromoverat värde skickas
variadiskt). Variadiska aggregat stöds inte.

**Externa datasymboler (`static` / `static mut`)** importerar globala konstanter
eller variabler exporterade av delade bibliotek (t.ex. Waylands
`wl_*_interface`-tabeller, Cocoas `NSDefaultRunLoopMode`/`NSApp` och POSIX
`environ`):

```dunder
@link("wayland-client")
extern "C" {
    static wl_compositor_interface: wl_interface
    static wl_shm_interface: wl_interface
}

@link("c")
extern "C" {
    static mut environ: *mut *mut ffi.c_char
}
```

En `extern static` emitteras som `STT_OBJECT` på ELF64, datapekare i
`__DATA_CONST`/GOT på Mach-O och dataslot i IAT på PE32+. Både läsning,
skrivning (`static mut`) och adressbildning (`addr_of(&wl_compositor_interface)`)
kräver `unsafe`-kontext.

**Nåbarhetsbaserad beskärning**: `target_is("...")` i `if`/`else if`-grenar
utvärderas vid kompilering före importregistrering. Onåbara `extern`-deklarationer
skapar inga `LC_LOAD_DYLIB`-, `DT_NEEDED`- eller IAT-poster. Ett Linux-program
utan nåbara `extern`-symboler fortsätter därmed att kompileras till en helt
statisk, libc-fri ELF-binär med direkta systemanrop.

### 2.2 Callbacks (`extern "<abi>" fn`) och trådsäker körmiljö

En toppnivåfunktion kan deklareras direkt med en främmande anropskonvention
och skickas som typad funktionspekare till C, Objective-C, Win32, Wayland,
Vulkan eller COM:

```dunder
capability unsafe

extern "C" fn on_registry_global(
    data: *mut ffi.c_void,
    registry: *mut wl_registry,
    name: u32,
    iface: *const ffi.c_char,
    version: u32,
) {
    // ...
}
```

Garantier och semantik för `extern "<abi>" fn`-callbacks:

1. **Fullständigt C-ABI i båda riktningarna**: parametrar och returvärden får
   vara godtyckliga FFI-typer, inklusive `@repr(C)`-structar och -unioner per
   värde, `@repr(<heltal>)`-enums, råpekare och `Option<extern fn>`.
2. **Registerbevarande**: callback-prologen sparar och återställer samtliga
   callee-saved register enligt målets ABI (inklusive `x19..x28` och `d8..d15`
   på ARM64 samt `XMM6..XMM15` på Win64).
3. **OS-TLS för samtidighet, reentrans och främmande trådar**:
   - Dunders trådlokala exekveringstillstånd (registerbank och allokatorkontext)
     lagras i operativsystemets trådlokala lagring (OS-TLS), aldrig i en enda
     delad global slot.
   - Flera trådar kan samtidigt korsa FFI-gränsen och anropa callbacks utan
     datakapplöpning.
   - Reentranta callbacks (exempelvis `DispatchMessageW` -> `WndProc` ->
     Win32-anrop -> ny `WndProc`, eller nästlade AppKit-delegater) fungerar
     genom att varje aktivering sparar och återställer föregående ramtillstånd.
   - **Främmande OS-trådar** (t.ex. Metal-kompletteringshanterare,
     ljudcallbacks eller drivrutinstrådar): om en callback anropas på en tråd
     som inte skapats av Dunder kopplas automatiskt en lättvikts, trådisolerad
     Dunder-kontext till trådens TLS (med automatisk städning när OS-tråden
     avslutas). Den gamla tvångsavslutningen med kod 111 vid anrop från annan
     tråd tas bort.
4. **Fällgräns (Abort-on-trap)**: Dunder-fällor (`panic`, `assert`, overflow,
   ogiltigt index) får aldrig rulla upp stacken genom främmande C/ObjC/COM/OS-kod.
   En fälla som når gränsen för en `extern "<abi>" fn` skriver diagnostik till
   stderr och avslutar processen omedelbart och deterministiskt.
5. **Kapabiliteter**: om en `extern "<abi>" fn` använder `@requires(...)`
   kontrolleras kapabiliteten på det ställe där funktionsvärdet tas, precis
   som för vanliga funktionsvärden.

### 2.3 Resurssäker dynamisk laddning (`std.dl`)

För bibliotek som är valfria vid körning, väljs dynamiskt (t.ex. Wayland vs
X11 på samma Linux-binär) eller laddar funktioner via en drivrutinsloader
(t.ex. Vulkan `vkGetInstanceProcAddr`/`vkGetDeviceProcAddr` eller
`eglGetProcAddress`) tillhandahåller `std.dl` ett resurssäkert gränssnitt:

```dunder
capability unsafe
import std.dl as dl

unsafe fn open_vulkan() -> Result<(dl.Library, dl.ProcResolver), string> {
    lib := dl.Library::open_first(&["libvulkan.so.1", "libvulkan.so"], .Now)?
    resolver := dl.ProcResolver::from_library_context_proc_addr(
        &lib,
        "vkGetInstanceProcAddr",
        ptr(0),
    )?
    .Ok((lib, resolver))
}
```

Kontrakt i `std.dl`:

- `pub enum LoadMode { Default, Lazy, Now }` — ersätter råa POSIX-heltal (`1`, `2`),
  med `mode_flags(mode: LoadMode) -> i32`.
- `@resource(dispose) pub opaque struct Library` (och `unsafe impl Sendable for Library`):
  - `Library::open(path: string) -> Result<Library, string>`
  - `Library::open_with(path: string, mode: LoadMode) -> Result<Library, string>`
  - `Library::open_lazy(path: string) -> Result<Library, string>`
  - `Library::open_first(candidates: &[]string, mode: LoadMode) -> Result<Library, string>` (alias `open_any`)
  - `Library::current() -> Result<Library, string>` (alias `current_process`)
  - `lib.is_open() -> bool`, `lib.raw_handle()`, `lib.close() -> bool`, `lib.leak()`
  - `unsafe lib.symbol(name: string) -> Result<..., string>` / `unsafe lib.try_symbol(name: string)`
  - `unsafe lib.load_required_symbols(names: &[]string)` (alias `load_symbols`) — laddar en hel grupp obligatoriska symboler och rapporterar alla saknade namn i ett samlat fel.
  - `unsafe lib.load_optional_symbols(names: &[]string)` — returnerar `.Some(...)`/`.None` per symbol.
  - `unsafe lib.resolver() -> ProcResolver` — skapar en `ProcResolver` kopplad till biblioteket.
  - `dispose` frigör handtaget (`dlclose` / `FreeLibrary`). Anroparen ansvarar
    för att `Library` hålls vid liv så länge någon upplöst funktionspekare kan
    anropas.
- **Enhetlig uppslagning och kedjor (`ProcResolver` och `ProcChain`)**:
  - `ProcResolver::from_library(&lib)` / `from_raw_library(handle)`
  - `ProcResolver::from_proc_addr(get_proc)` / `from_proc_addr_ptr(raw_fn)` / `from_library_proc_addr(&lib, loader_symbol)` — för 1-argumentsuppslagare (t.ex. `eglGetProcAddress`, `glXGetProcAddress`).
  - `ProcResolver::from_context_proc_addr(context, get_proc)` / `from_context_proc_addr_ptr(context, raw_fn)` / `from_library_context_proc_addr(&lib, loader_symbol, context)` samt `resolver.derive_context(loader_symbol, context)` — för 2-arguments kontextuppslagare (t.ex. `vkGetInstanceProcAddr` -> `vkGetDeviceProcAddr`).
  - `ProcChain` (`new`, `from_resolvers`, `push`, `try_resolve`, `resolve`, `load_required_symbols`, `load_optional_symbols`) — ordnad reservkedja av uppslagare (t.ex. device -> instance -> library).
  - Modulfunktioner: `dl.load_required_symbols`, `dl.load_optional_symbols`, `dl.load_proc_symbols`, `dl.load_optional_proc_symbols`.

---

## 3. Säkerhets- och kapabilitetsmodell

### 3.1 Strikt modulbunden `capability unsafe`

Ett modulhuvud deklarerar vilka privilegier som får användas direkt i filen,
exempelvis `capability filesystem, unsafe`.

- `capability unsafe` kontrolleras **strikt per modul**. Om modul A importerar
  modul B (t.ex. `std.alloc` eller en FFI-bindning) som deklarerar
  `capability unsafe`, får modul A **aldrig** ärva `unsafe`-behörighet.
- Utan `capability unsafe` i modulens eget huvud avvisas `extern`-block,
  `extern "<abi>" fn`-definitioner, `unsafe fn`, `unsafe { ... }`,
  `@thread_safe` samt `impl Sendable` för resurser som innehåller råpekare.
- Säker kod får anropa publika, säkra omslagsfunktioner som exponerats av en
  `unsafe`-modul, under förutsättning att omslaget upprätthåller kontrakten för
  livstid, justering, nullfrihet och aliasing.

### 3.2 Enhetlig `unsafe`- och `@requires`-kontroll

- Anrop till `extern`-funktioner, anrop via `extern fn`-värden, avreferering
  eller pekarkalkyl på `*const T`/`*mut T`, läsning av `union`-fält,
  läsning/skrivning/adress av `extern static` samt `as_ref`/`as_mut`/`bytes_view`
  kräver alltid en `unsafe`-kontext.
- Kapabilitetskrav (`@requires(filesystem | network | process | stdio)`)
  kontrolleras enhetligt både vid direkta anrop, när funktionsvärdet tas från
  en `extern`-deklaration (`f := my_extern`) och när en funktion används som
  `extern "<abi>" fn`-callback.

### 3.3 Tillåtelselista för nativa bibliotek (`native_libs`)

För tredjepartsberoenden i `dunder.toml` räcker `capabilities = ["unsafe"]`
inte ensamt för att länka godtyckliga nativa bibliotek. Varje beroende måste
dessutom ha de logiska biblioteksnycklar det länkar godkända i `native_libs`:

```toml
[dependencies]
vk = { path = "../vk", capabilities = ["unsafe"], native_libs = ["vulkan", "wayland-client"] }
```

Regler:

- Standardvärdet för `native_libs` på ett beroende är `[]` (inga nativa
  bibliotek tillåtna).
- `native_libs = ["*"]` beviljar uttryckligen obegränsad nativ länkning till
  beroendet.
- Rotpaketet och det inbyggda standardbiblioteket (`std`/`sys`) är obegränsade.
- För transitiva beroenden gäller snittet längs beroendekedjan: en nyckel
  måste vara godkänd (explicit eller via `"*"`) i varje led från rotprojektet
  ned till det länkande paketet.
- Om ett beroende innehåller en nåbar `@link("<nyckel>")`-deklaration som inte
  finns i dess beviljade `native_libs` stoppas bygget med felmeddelande som
  pekar ut beroendet och den otillåtna biblioteksnyckeln.

### Objektminne och privata stackceller

- Rå minnesåtkomst måste avse ett levande källobjekt eller en externt
  tillhandahållen region och hålla sig inom objektets giltiga intervall. En
  rundresa via `usize` ger ingen ny åtkomsträtt till andra lokala objekt.
- Kompilatorns spillceller, oexponerade lokaler och FP/LR-länkage är privat
  implementationslagring som främmande kod inte får läsa eller skriva. Ett
  uttryckligt `addr_of(&place)` exponerar endast det angivna objektet under dess
  livstid och låneregel.
- Rekursion och återanrop skapar separata aktiveringar. Vanliga externa anrop
  returnerar en gång eller avslutar processen; stackbyte, `longjmp` och
  `returns-twice` ingår inte i ett vanligt `extern`-kontrakt.

### Filåtkomst genom delegerade handtag

För högre OS-privilegier kompletteras `capability unsafe` av
`capability filesystem`, `network`, `process` och `stdio` samt delegerade
handtag (`fs.ReadFile`, `fs.WriteFile`, `fs.ReadDir`, `process.Executable`).
En modul med `capability filesystem` väljer sökvägar med `grant(path)` och kan
sedan lämna över det opaka handtaget till oprivilegierade moduler som anropar
dess metoder utan att själva äga `capability filesystem`. Handtagen lagrar en
sökväg som operativsystemet slår upp vid varje operation (inklusive
symboliska länkar och relativa sökvägar) och kan öppnas till strömmande
`@resource(dispose)`-handtag (`FileReader`, `FileWriter`, `File`).

---

## 4. Manifestpolicy (`dunder.toml`), biblioteksregister och binäremittrar

### 4.1 Schema i `dunder.toml` och CLI-flaggor

Följande sektioner och nycklar styr nativ länkning och applikationspaketering
i `dunder.toml`:

```toml
name = "studio"
version = "0.1.0"
root = "src/main.ddr"

[dependencies]
gfx = { path = "../gfx", capabilities = ["unsafe"], native_libs = ["metal", "objc", "vulkan", "d3d12", "dxgi"] }

[native]
rpath = ["$ORIGIN/../lib"]

[native.libs.vulkan]
weak = false

[target.arm64-macos.native]
rpath = ["@executable_path/../Frameworks"]

[target.arm64-macos.native.libs.vulkan]
path = "@rpath/libMoltenVK.dylib"

[target.x86_64-linux.native]
libc = "glibc"

[target.x86_64-linux.native.libs.curl]
soname = "libcurl.so.4"

[target.x86_64-windows.native.libs.d3d12]
dll = "d3d12.dll"
delay = true

[app]
subsystem = "gui"
dpi_aware = true
utf8 = true
icon = "assets/studio.ico"
bundle_id = "org.example.studio"
entitlements = "macos/entitlements.plist"
frameworks = ["vendor/MoltenVK.framework"]
```

Sammanfattning av schemafälten:

| Sektion / Nyckel | Typ / Värden | Beskrivning |
| --- | --- | --- |
| `[dependencies.<namn>].native_libs` | `[]string` (standard `[]`) | Tillåtna logiska biblioteksnycklar eller `["*"]`. |
| `[native].rpath` / `[target.<t>.native].rpath` | `[]string` | Sökvägar för `LC_RPATH` (Mach-O) eller `DT_RUNPATH` (ELF64). |
| `[native].libc` / `[target.<t>.native].libc` | `"glibc"` \| `"musl"` | Väljer Linux dynamisk laddare (`PT_INTERP`) och libc-SONAME (standard `"glibc"`). |
| `[native.libs.<key>]` / `[target.<t>.native.libs.<key>]` | Tabell | Åsidosätter nyckeln `<key>` med fälten `path`, `framework`, `soname`, `dll` (strängar) samt `weak`, `delay` (booleska). |
| `[app].subsystem` | `"console"` \| `"gui"` | Windows PE32+ subsystem (`CUI = 3` standard, `GUI = 2`). |
| `[app].dpi_aware` | `bool` | Bäddar in `PerMonitorV2` DPI-medvetenhet i Windows `.rsrc`-manifestet. |
| `[app].utf8` | `bool` | Bäddar in `<activeCodePage>UTF-8</activeCodePage>` i Windows `.rsrc`-manifestet. |
| `[app].icon` | `string` | Relativ sökväg till `.ico`-fil som bäddas in i Windows `.rsrc`. |
| `[app].bundle_id` | `string` | Eget `CFBundleIdentifier` för macOS `.app`-bundle. |
| `[app].entitlements` | `string` | Relativ sökväg till XML-entitlements som förseglas i Mach-O `CodeDirectory`. |
| `[app].frameworks` | `[]string` | Lokala `.framework`/`.dylib`-paket som kopieras till `Contents/Frameworks/` i macOS `.app`. |

**Prioritetsordning för upplösning av en logisk nyckel**:
1. `[target.<exakt-mål>.native.libs.<key>]` (t.ex. `x86_64-linux`, `arm64-macos`)
2. `[target.<familj>.native.libs.<key>]` (`macos`, `linux`, `windows`)
3. `[native.libs.<key>]`
4. Kompilatorns inbyggda biblioteksregister (avsnitt 4.2)

**CLI-flaggor** (åsidosätter respektive manifestvärde för bygget):

- `--subsystem console|gui` — väljer Windows PE32+ subsystem.
- `--rpath <sökväg>` — lägger till en `LC_RPATH`/`DT_RUNPATH`-post (kan upprepas).
- `--libc gnu|musl` — väljer Linux libc-ABI och `PT_INTERP`.

Alla `[native]`-, `[app]`-, `native_libs`- och CLI-länkinställningar inkluderas
i byggkvittot (`build_receipt`) så att ett ändrat länk- eller paketval alltid
ogiltigförklarar ett tidigare cachat bygge.

### 4.2 Inbyggt logiskt biblioteksregister

Kompilatorn innehåller en inbyggd upplösningstabell för kända OS- och
grafikbibliotek. Källkod behöver därmed inte hårdkoda plattformsspecifika
filnamn eller `/System/Library/Frameworks/...`-sökvägar.

| Logisk nyckel | `arm64-macos` | `arm64-linux` / `x86_64-linux` (`glibc`) | `arm64-linux` / `x86_64-linux` (`musl`) | `x86_64-windows` |
| --- | --- | --- | --- | --- |
| `c` | `/usr/lib/libSystem.B.dylib` | `libc.so.6` | `libc.musl-<arch>.so.1` | `msvcrt.dll` |
| `dl` | `/usr/lib/libSystem.B.dylib` | `libdl.so.2` | `libc.musl-<arch>.so.1` | `kernel32.dll` |
| `m` | `/usr/lib/libSystem.B.dylib` | `libm.so.6` | `libc.musl-<arch>.so.1` | `ucrtbase.dll` |
| `pthread` | `/usr/lib/libSystem.B.dylib` | `libpthread.so.0` | `libc.musl-<arch>.so.1` | `kernel32.dll` |
| `objc` | `/usr/lib/libobjc.A.dylib` | — | — | — |
| `metal` | `/System/Library/Frameworks/Metal.framework/Versions/A/Metal` | — | — | — |
| `appkit` | `/System/Library/Frameworks/AppKit.framework/Versions/C/AppKit` | — | — | — |
| `cocoa` | `/System/Library/Frameworks/Cocoa.framework/Versions/A/Cocoa` | — | — | — |
| `foundation` | `/System/Library/Frameworks/Foundation.framework/Versions/C/Foundation` | — | — | — |
| `quartzcore` | `/System/Library/Frameworks/QuartzCore.framework/Versions/A/QuartzCore` | — | — | — |
| `coregraphics` | `/System/Library/Frameworks/CoreGraphics.framework/Versions/A/CoreGraphics` | — | — | — |
| `corevideo` | `/System/Library/Frameworks/CoreVideo.framework/Versions/A/CoreVideo` | — | — | — |
| `vulkan` | `@rpath/libvulkan.1.dylib` | `libvulkan.so.1` | `libvulkan.so.1` | `vulkan-1.dll` |
| `wayland-client` | — | `libwayland-client.so.0` | `libwayland-client.so.0` | — |
| `x11` | — | `libX11.so.6` | `libX11.so.6` | — |
| `xcb` | — | `libxcb.so.1` | `libxcb.so.1` | — |
| `xkbcommon` | — | `libxkbcommon.so.0` | `libxkbcommon.so.0` | — |
| `kernel32` | — | — | — | `kernel32.dll` |
| `user32` | — | — | — | `user32.dll` |
| `gdi32` | — | — | — | `gdi32.dll` |
| `ole32` | — | — | — | `ole32.dll` |
| `ws2_32` | — | — | — | `ws2_32.dll` |
| `d3d12` | — | — | — | `d3d12.dll` |
| `dxgi` | — | — | — | `dxgi.dll` |
| `d3dcompiler` | — | — | — | `d3dcompiler_47.dll` |
| `advapi32` | — | — | — | `advapi32.dll` |
| `shell32` | — | — | — | `shell32.dll` |
| `ucrt` | `/usr/lib/libSystem.B.dylib` | `libc.so.6` | `libc.musl-<arch>.so.1` | `ucrtbase.dll` |

Utöver tabellen ovan stöder macOS ramverksformen `@link(framework = "<Namn>")`
(intern nyckel `framework:<Namn>`), som automatiskt löses till
`/System/Library/Frameworks/<Namn>.framework/Versions/<ver>/<Namn>` (`C` för
`Foundation`/`AppKit`, `B` för `Message`/`System`, annars `A`). På Linux
ersätts `<arch>` med `aarch64` respektive `x86_64`. För MoltenVK på macOS
åsidosätts nyckeln `vulkan` vid behov via `[target.arm64-macos.native.libs.vulkan]`.

### 4.3 Garantier hos de inbyggda binäremittrarna

#### Mach-O (`arm64-macos`)

- **Dyld Shared Cache**: Systembibliotek och ramverk löses direkt ur macOS
  dyld shared cache via sina kanoniska installationsnamn i `LC_LOAD_DYLIB`,
  utan `.tbd`-filer eller installerad Xcode SDK.
- **Bindning och segment**: Importerade funktions- och datapekare binds med
  `LC_DYLD_CHAINED_FIXUPS` i det skrivskyddade segmentet `__DATA_CONST`
  (kompatibelt med moderna dyld-krav och `codesign --verify --strict`).
- **Load commands**: Emitterar `LC_LOAD_DYLIB`, `LC_LOAD_WEAK_DYLIB` (med
  `BIND_SYMBOL_FLAGS_WEAK_IMPORT` för mjuka symboler) och `LC_RPATH`.
- **Dynamisk `text_off`**: Startoffset för `__text` beräknas dynamiskt och
  sidjusteras till `16 384` byte oberoende av antal load commands (ingen övre
  16 KiB-gräns); DWARF-basadressen och `adrp`-relokeringar följer samma
  `text_off`.
- **Signering och `.app`-paket**: Den inbyggda ad hoc-signeraren stöder
  entitlements i `CodeDirectory` (`CSSLOT_ENTITLEMENTS`) när
  `[app].entitlements` anges. `.app`-paketeraren stöder inbäddade ramverk i
  `Contents/Frameworks/` inklusive interna relativa symlänkar som
  `Versions/Current -> A` (och avvisar alltjämt absoluta eller utbrytande
  symlänkar).

#### ELF64 (`arm64-linux`, `x86_64-linux`)

- **Statisk standard för ren Dunder**: Program utan nåbara `extern`-symboler
  emitteras som 100 % statiska, libc-fria ELF-binärer med direkta systemanrop.
- **Dynamiska ELF-binärer**: Emitterar positionsoberoende `ET_DYN` (PIE) med
  `PT_GNU_RELRO` och `DT_BIND_NOW` så att GOT-tabellen skrivskyddas direkt
  efter startbindning.
- **Tabeller och symboler**: Emitterar `DT_NEEDED` (alltid versionerade
  SONAMEs, aldrig `-dev`-länkar), `DT_RUNPATH`, `DT_GNU_HASH` (samt vid behov
  SysV `DT_HASH`), `STT_FUNC` för funktioner, generella `STT_OBJECT`-poster
  för `extern static`-datasymboler samt `STB_WEAK` för mjuka symboler.
- **Symbolversionering**: När symbolversioner anges (eller krävs av
  glibc-symboler) emitteras `DT_VERSYM`, `DT_VERNEED` och `DT_VERNEEDNUM` så
  att oversionerade referenser inte tyst binds till historiska 1990-talsversioner
  av glibc-symboler.
- **Valbar libc-ABI (`glibc` / `musl`)**: Sätter korrekt `PT_INTERP`
  (`/lib/ld-linux-aarch64.so.1` eller `/lib64/ld-linux-x86-64.so.2` för glibc;
  `/lib/ld-musl-aarch64.so.1` eller `/lib/ld-musl-x86_64.so.1` för musl) samt
  motsvarande standard-SONAMEs.

#### PE32+ (`x86_64-windows`)

- **Dynamiska huvuden**: `SizeOfHeaders` beräknas utifrån faktiskt antal
  sektioner (ingen 512-byte-begränsning).
- **Subsystem**: Stöder både `IMAGE_SUBSYSTEM_WINDOWS_CUI` (`3`, standard) och
  `IMAGE_SUBSYSTEM_WINDOWS_GUI` (`2`, via `[app] subsystem = "gui"` eller
  `--subsystem gui`) utan oönskat konsolfönster för grafikprogram.
- **Stackupprullning och stackprobning**: Emitterar `.pdata` och `.xdata`
  (DataDir 3) med giltiga `RUNTIME_FUNCTION`/`UNWIND_INFO`-poster för
  Win64 SEH, drivrutiner, COM och debuggers, samt `__chkstk`-stackprobning för
  stackramar större än 4 KiB.
- **ASLR**: Emitterar `.reloc` (DataDir 5) och sätter
  `IMAGE_DLLCHARACTERISTICS_DYNAMIC_BASE | IMAGE_DLLCHARACTERISTICS_HIGH_ENTROPY_VA`.
- **Resurser (`.rsrc`, DataDir 2)**: Bäddar direkt in Win32 XML-applikationsmanifest
  (`PerMonitorV2` DPI-medvetenhet, `UTF-8` activeCodePage, Windows 10/11
  `supportedOS`-GUID:er) samt valfri applikationsikon utan `rc.exe`.
- **Importtabeller**: Skriver direkt IAT i skrivskyddad `.rdata`, stöder
  Delay-Load Import Directory (DataDir 13) för `delay = true`-bibliotek,
  import via ordinalnummer (`@link_ordinal(N)`) och använder `ucrtbase.dll`
  som standard-UCRT.

#### WebAssembly (`wasm32`, `wasm32-wasi`)

- Nåbara `extern`-block blir typade Wasm-funktionsimporter under angiven
  importmodul (`@link("wasi_snapshot_preview1")`, `@link("env")` osv.).
- `@repr(C)`-structar och -unioner använder 32-bitars pekare och skickas via
  pekare i linjärt minne.
- `extern "C" fn`-callbacks läggs i den exporterade tabellen
  `__indirect_function_table` så att värden kan anropa dem via `call_indirect`.

---

## 5. Exempel för verkliga OS- och grafik-API:er

### 5.1 Metal och Objective-C (macOS)

```dunder
capability unsafe
import std.ffi as ffi

extern type MTLDevice
extern type ObjcClass
extern type ObjcSel
extern type NSString

@repr(C)
struct MTLClearColor {
    red: f64,
    green: f64,
    blue: f64,
    alpha: f64,
}

@link("metal")
extern "C" {
    fn MTLCreateSystemDefaultDevice() -> *mut MTLDevice
}

@link("foundation")
extern "C" {
    static NSDefaultRunLoopMode: *const NSString
}

@link("objc")
extern "C" {
    fn objc_getClass(name: *const ffi.c_char) -> *mut ObjcClass
    fn sel_registerName(name: *const ffi.c_char) -> *mut ObjcSel

    @link_name("objc_msgSend")
    fn msg_send_ptr(obj: *mut ffi.c_void, sel: *mut ObjcSel) -> *mut ffi.c_void

    @link_name("objc_msgSend")
    fn msg_send_set_clear_color(
        pass: *mut ffi.c_void,
        sel: *mut ObjcSel,
        color: MTLClearColor,
    )
}
```

`MTLClearColor` (fyra `f64`, HFA) skickas direkt i `d0..d3` enligt Apple
ARM64-ABI utan manuell pekaromskrivning, och `NSDefaultRunLoopMode` importeras
direkt som datasymbol ur `Foundation.framework` i dyld shared cache.

### 5.2 Vulkan (Linux, Windows och macOS)

```dunder
capability unsafe
import std.dl as dl
import std.ffi as ffi

extern type VkInstance_T

@repr(i32)
enum VkResult {
    Success = 0,
    NotReady = 1,
    ErrorOutOfHostMemory = -1,
}

@repr(C)
union VkClearValue {
    color: [f32; 4],
    depth_stencil: VkClearDepthStencilValue,
}

@repr(C)
struct VkClearDepthStencilValue {
    depth: f32,
    stencil: u32,
}

@repr(C)
struct VkAllocationCallbacks {
    pUserData: *mut ffi.c_void,
    pfnAllocation: Option<extern "system" fn(*mut ffi.c_void, usize, usize, u32) -> *mut ffi.c_void>,
    pfnReallocation: Option<extern "system" fn(*mut ffi.c_void, *mut ffi.c_void, usize, usize, u32) -> *mut ffi.c_void>,
    pfnFree: Option<extern "system" fn(*mut ffi.c_void, *mut ffi.c_void)>,
    pfnInternalAllocation: Option<extern "system" fn(*mut ffi.c_void, usize, u32, u32)>,
    pfnInternalFree: Option<extern "system" fn(*mut ffi.c_void, usize, u32, u32)>,
}

@repr(C)
struct VkInstanceFns {
    vkDestroyInstance: extern "system" fn(*mut VkInstance_T, *const VkAllocationCallbacks),
    vkEnumeratePhysicalDevices: extern "system" fn(*mut VkInstance_T, *mut u32, *mut *mut ffi.c_void) -> VkResult,
}
```

### 5.3 DirectX 12 och COM (Windows)

```dunder
capability unsafe
import std.ffi as ffi

extern type ID3D12DescriptorHeap

@repr(C)
struct GUID {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

@repr(C)
struct D3D12_CPU_DESCRIPTOR_HANDLE {
    ptr_val: usize,
}

@repr(C)
struct ID3D12DescriptorHeapVtbl {
    QueryInterface: extern "system" fn(*mut ID3D12DescriptorHeap, *const GUID, *mut *mut ffi.c_void) -> i32,
    AddRef: extern "system" fn(*mut ID3D12DescriptorHeap) -> u32,
    Release: extern "system" fn(*mut ID3D12DescriptorHeap) -> u32,
    // ... mellanliggande COM-metoder ...
    GetCPUDescriptorHandleForHeapStart: extern "msvc-com" fn(*mut ID3D12DescriptorHeap) -> D3D12_CPU_DESCRIPTOR_HANDLE,
}

@link("d3d12", delay = true)
extern "system" {
    fn D3D12CreateDevice(
        adapter: *mut ffi.c_void,
        min_feature_level: u32,
        riid: *const GUID,
        out_device: *mut *mut ffi.c_void,
    ) -> i32
}
```

`"msvc-com"` garanterar att `GetCPUDescriptorHandleForHeapStart` skickar
`this` i `RCX` och dold `sret`-pekare i `RDX` enligt MSVC:s C++ COM-ABI, medan
`delay = true` fördröjer laddningen av `d3d12.dll`.

### 5.4 Win32-fönster och callback (Windows)

```dunder
capability unsafe
import std.ffi as ffi

extern type HWND_T
extern type HINSTANCE_T

@repr(C)
struct WNDCLASSEXW {
    cbSize: u32,
    style: u32,
    lpfnWndProc: Option<extern "system" fn(*mut HWND_T, u32, usize, isize) -> isize>,
    cbClsExtra: i32,
    cbWndExtra: i32,
    hInstance: *mut HINSTANCE_T,
    hIcon: *mut ffi.c_void,
    hCursor: *mut ffi.c_void,
    hbrBackground: *mut ffi.c_void,
    lpszMenuName: *const u16,
    lpszClassName: *const u16,
    hIconSm: *mut ffi.c_void,
}

@link("user32")
extern "system" {
    fn RegisterClassExW(wc: *const WNDCLASSEXW) -> u16
    fn DefWindowProcW(hwnd: *mut HWND_T, msg: u32, wparam: usize, lparam: isize) -> isize
}

extern "system" fn wnd_proc(hwnd: *mut HWND_T, msg: u32, wparam: usize, lparam: isize) -> isize {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

unsafe fn register_window(inst: *mut HINSTANCE_T) -> u16 {
    wc := WNDCLASSEXW {
        cbSize: u32(size_of(WNDCLASSEXW)),
        lpfnWndProc: .Some(wnd_proc),
        hInstance: inst,
        lpszClassName: w"DunderWindow".as_ptr(),
        ..zeroed()
    }
    RegisterClassExW(addr_of(&wc))
}
```

### 5.5 Wayland-klient (Linux)

```dunder
capability unsafe
import std.ffi as ffi

extern type wl_display
extern type wl_registry
extern type wl_interface

@repr(C)
union wl_argument {
    i: i32,
    u: u32,
    f: i32,
    s: *const ffi.c_char,
    o: *mut ffi.c_void,
    n: u32,
    a: *mut ffi.c_void,
    h: i32,
}

@repr(C)
struct wl_registry_listener {
    global: Option<extern "C" fn(*mut ffi.c_void, *mut wl_registry, u32, *const ffi.c_char, u32)>,
    global_remove: Option<extern "C" fn(*mut ffi.c_void, *mut wl_registry, u32)>,
}

@link("wayland-client")
extern "C" {
    static wl_compositor_interface: wl_interface
    static wl_shm_interface: wl_interface

    fn wl_display_connect(name: *const ffi.c_char) -> *mut wl_display
    fn wl_display_dispatch(display: *mut wl_display) -> ffi.c_int
    fn wl_display_disconnect(display: *mut wl_display)
}
```

---

## 6. Migrering från äldre FFI-syntax

Den äldre FFI-ytan fasas ut under migreringen av `stdlib/`, `packages/`,
`tests/` och `tools/`:

| Äldre konstruktion | Ny specifikation |
| --- | --- |
| `@c_layout struct S { ... }` | `@repr(C) struct S { ... }` (plus `packed`, `align(N)`, `union` och `@repr(<heltal>) enum`) |
| `@extern("lib", "sym") unsafe fn f(...)` | `@link("nyckel") extern "C" { [@link_name("sym")] fn f(...) }` |
| `@c_variadic(N) @extern(...)` | Variadisk parameter `...` inne i `extern "<abi>" { ... }` |
| `ptr`, `addr(&x)`, `load_*`, `store_*` | `*const T`, `*mut T`, `addr_of(&x)`, `.read()`, `.write()`, `.read_unaligned()`, `.write_unaligned()` |
| `ref_at(p)` (oankrad) | `p.as_ref(&owner)` / `p.as_mut(&mut owner)` eller `p.read()` / `p.write(v)` |
| Manuell `dlopen`/`dlsym`-hantering | Resurssäker `std.dl.Library`, `ProcResolver` och `ProcChain` |
| `cstr("...")` | `c"..."` (`&ffi.CStr`) och `w"..."` (`&ffi.WStr`) |
| `callback(fn_namn)` | Toppnivå `extern "<abi>" fn fn_namn(...)` |
| `zeroed()` med heapallokering | Stackallokerad `ffi.zeroed<T>()` och `S { f: v, ..zeroed() }` |
