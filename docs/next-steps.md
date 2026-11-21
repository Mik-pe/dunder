# Nästa steg för Dunder

Fältflyttar, `replace`, returnerade läslån och lånade intervall är införda.
Arrayer kan äga sammansatta värden och ta bort ett element med bibehållen
ordning. [Minnesmodellen](memory.md) · [Standardbiblioteket](standard.md)

## Samlingar

Det statiska traitsystemet (`Eq`, `Ord`, `Hash`, `Display`, `Default`,
`Sendable`, `Storable`, `Clone`, `Copy`) stöder villkorliga
implementationer, standardmetoder i traits och generiska `Self`-signaturer.
Preludium och standardbibliotek (`std.cmp`, generiska algoritmer i `std.array`,
`std.hash.Hasher`, `std.wire`, `Result<T, E>`) använder dessa kontrakt.
[Syntax för traits och generics](syntax.md#generiska-deklarationer) · [Traits-design](generics-design.md)

[`Map<K, V>` och `Set<K>`](standard.md#generiska-samlingar) stöder
statiskt specialiserade typer med `K: Hash + Storable` och `V: Storable`,
lånade uppslag och flytt av ägda värden. Proven täcker hashkollisioner,
omhashning, resurser, villkorlig `Sendable`/`Eq` och kloningsdiagnostik.

[Undersökningen av kolumnlagring](soa-research-plan.md) visar hur en
följeslagartyp kan ge poståtkomst och kolumnlån med befintliga arrayer.
Prototypen passerade med den självbyggda kompilatorn som även avvisade
skrivning under ett aktivt kolumnlån.

## Externa resurser och typade fel

[Automatisk stängning](memory.md#automatisk-stängning) kan deklareras med
`@resource(dispose)`. Den följer ägaren genom flytt och körs synkront när
ägaren släpps. TCP-handtagen, `StreamSet` och `UdpSocket` använder den.

[`Result<T, E>` och felpropagering med `?`](enums.md#typade-fel-med-result)
använder statiskt typade varianter. Ett prov verifierar att `.Err` kan
lämna tillbaka en resurs och att stängning och `defer` följer returvägen.
`ChildSet.try_insert` använder `Result<i32, Child>` för verkliga barnprocesser.

## Språk och standardbibliotek

[Runliteraler](syntax.md#utf-8-strängar-och-runes) och
[relativa importer](syntax.md#moduler-och-importer) stöds. `std.parse`
läser decimaler till `f32` och `f64`.
[`std.json`](standard.md#skriva-json) har flyttalsfält och en skrivare
som sköter strukturen för objekt och arrayer.
På nativa mål visar [en misslyckad jämförelse i `assert`](syntax.md#assertioner)
båda operandvärdena, även för booleska testuttryck i `@test`.
[Stränginterpolering](syntax.md#stränginterpolering) formaterar heltal
decimalt och hexadecimalt med valfri minsta bredd och nollutfyllnad.
[Arrayliteraler](syntax.md#arrayer-intervall-och-iteratorer) härleder typen
för heltalsliteraler från övriga element eller från deras värdeintervall.
`len` kan användas som lokalt namn, fält och metod.
[Strängägande vid `push`](standard.md#växande-arrayer) är verifierat med
flytt, delad text via `.clone()` och färska värden.
[Flervärdesreturer](syntax.md#flervärdesreturer) använder typade tupler som
kan plockas isär, också när en komponent äger en resurs.
[`std.math`](standard.md#matematik) samlar `clamp`, `min`, `max`, `sin`,
`cos` och `tan`. Metal- och Vulkan-exemplen använder samma modul.

### Nätverk

`std.net` samlar TCP- och UDP-transporten för både IPv4 och IPv6:
`IpAddr`, `SocketAddr`, `TcpListener` (med `listen_addr` och `local_addr`),
`TcpStream` (med `connect_addr`, `connect_timeout`, `connect_host`,
`local_addr`, `peer_addr`, `peek_append`, `shutdown_read`, `shutdown_both` samt
läs-/skrivtimeouts), `StreamSet` (`Default`, `peek_append`, `shutdown_read`,
`shutdown_both`, `local_addr`, `peer_addr`, timeouts) och `UdpSocket` (`@resource(dispose)`,
`bind`, `connect`, `send_to`/`send_to_range`, `send`, `recv_from_append`,
`peek_from_append`, `recv_append`), samt `resolve` för värduppslagning.
Övergångsimporterna `std.http` och `std.http_client` har tagits bort;
HTTP-servern och klienten levereras som separata Dunderpaket.
[HTTP-paketen](http.md) · [Standardbiblioteket](standard.md)

### Körbara exempel i manifestet

Namngivna exempel i `[examples]` stöds nu via `--example <namn>` för `build`,
`run` och `check`. Rotfilen väljs relativt `dunder.toml` med projektets
gemensamma beroendegraf och `dunder.lock`, och utdata skrivs till
`build/<mål>/examples/<namn>` för att undvika kollisioner mellan exempel och
huvudprogram. `root` och `test_root` fortsätter att vara standardvalen.
[Körbara exempel i `dunder.toml`](dependencies.md#körbara-exempel)

### Strömmar och standardinmatning

`std.io` inför ström-traitsen `Read`, `Write`, `BufRead: Read` och `Seek`,
kapabiliteten `capability stdio` med `@resource`-handtagen `Stdin`, `Stdout` och
`Stderr` samt hjälpare som `read_stdin`, `read_stdin_bytes` och
`read_stdin_limit`. Omslag och källor inkluderar `BufReader<R>`, `BufWriter<W>`,
`Limited<R>`, `Cursor`, `Empty`, `Sink` och generiska funktioner som `copy`,
`read_to_end`, `read_to_string` och `write_all`. `std.fs` utökas med
`OpenOptions`, `FileReader`, `FileWriter` och `File`, `std.net.TcpStream`
implementerar `Read` och `Write`, och `std.process.Child` exponerar
`take_stdin`, `take_stdout` och `take_stderr` som strömmar.
[Strömmar och standardinmatning](standard.md#strömmar-och-standardinmatning)

### Generell allokering

`std.alloc` tillhandahåller `Layout`, `layout_of<T>`, `layout_array<T>`,
`Block`, `AllocError` och traiten `RawAllocator` (med `unsafe fn`-metoder i
traits och `impl`). Inbyggda allokatorer är `Global`, `System`, `Capped<A>`,
`Tracking<A>` (`Stats`) och `Bump<A>` ovanpå nya primitiv i `std.sys`
(`memory_zero`, `pool_alloc_try`, `pool_free`), kompletterade med
resurshanterade behållare som `OwnedBuf<A>`, `Arena<A>`/`FrozenArena<A>` (`Span`,
`read_into_arena`) och generationssäkra `Pool<T>`/`Id<T>`. Samtidigt utökas
`@resource(dispose)` till att stödja `fn dispose(&mut self)` och `bytes_view`
ger lånebunden `&[]u8`-åtkomst över råa pekarbuffertar.
[Generell allokering](standard.md#generell-allokering)


### Diagnostik och kompilator (0.6.0)

Kompilatorn ger nu direkta felmeddelanden och förbättringsförslag för vanliga
mönster: nyttolastbärande enumvarianter utan parenteser (t.ex. `.Err` i
`Result<T, E>`), fältåtkomst som avsåg metodanrop (t.ex. `parsed.is_ok()`),
mutering av oföränderliga `:=`-bindningar (med notering om deklarationsrad),
`&mut`-vidarebefordran och namngivna typer i typfel, exakta anropsplatser och
anropskedjor vid effektfel i `spawn`-arbetare, vägledning för `&arr[a..b]` vid
array-skivning, `mut` i `match`-mönsterbindningar (t.ex. `.Ok(mut x)`),
ignorera-tips för `()`-uttryck vid `_ := expr` samt modulnivåkonstanter
(`const`, inklusive konstanta arraytabeller). Generiska funktioner och
associerade funktioner på generiska typer (`array.reverse(&mut v)`,
`Box::new(42)`, `mut m: Map<string, i32> = Map::new()`) härleder typargument
från anropsargument eller förväntad typ, och publika typer/konstanter kan
direktimporteras med `import std.Map`, `import std.json.Doc` eller
`import .mod.Type`. Tvärsatsig Hindley–Milner-härledning för oannoterade
nollargumentsanrop som `mut m := Map::new()` hoppades över medvetet; där
krävs i stället explicita typargument (`Map<K, V>::new()`).
[Syntax](syntax.md) · [Task-modellen](tasks.md)

### Ägande och lån (0.6.0)

Läslån från `.len`, jämförelser eller lånade anrop avslutas automatiskt efter
sista läsningen innan senare flytt i samma block, loopvariabler i
`for x in move batch` känns igen som färska ägda värden vid `push`, och ägda
nyttolaster kan flyttas ur `match` (både `@resource`-handtag och ägda värden)
utan falska `.clone()`-förslag för resurser.
[Minnesmodellen](memory.md) · [Enums](enums.md)

### Standardbibliotek och formatering (0.6.0)

`std.text` har kompletterats med `contains`, `replace`, `lower`, `upper`,
`pad_left` och `pad_right`; f-strängar stöder bredd- och justeringsangivelser
för strängar (`:<N` / `:>N`); `std.bytes`, `std.crypto`, `std.text` och `std.sys`
har samordnats kring `[]u8` med konverteringshjälpare (`from_text`, `to_i32_vec`,
`sha256_bytes`); `std.math` erbjuder fällfria `checked_add_*`, `checked_sub_*`,
`checked_mul_*`, `checked_div_*` och `checked_rem_*` som fria typade funktioner
samt heltal har `.wrapping_add`/`.wrapping_sub`/`.wrapping_mul`; `std.json.Doc` dokumenterar `raw_span` och tillhandahåller
`raw_json`; och `std.process.ChildSet` dokumenterar `wait_any`.
[Standardbiblioteket](standard.md)

### CLI och dokumentation (0.6.0)

`dunder install` bygger och installerar binärer till `dunder`-katalogen (eller
`--dir` / `--prefix`), `dunder.toml` stöder `subdir` för Git-beroenden i
monorepon, `dunder run` bevarar anroparens arbetskatalog, `dunder build` och
`dunder run` namnger binärer i `build/` efter paketnamnet i `dunder.toml` när
paketets rotfil byggs, `dunder test` räknar enbart användarens egna `@test` och
dokumentationen har uppdaterats med kapabilitetsöversikten för `sys`/`std.fs`,
parenteser och `.Err(())` på `Result`, `Sendable`-argument i `spawn`, `_` i
`match` samt skillnaden mellan `&arr[a..b]` och `text[a..b]`.
[Beroenden och manifest](dependencies.md) · [Systembiblioteket](sys.md)

### Korrekthet, kapabilitetsskydd och diagnostik (0.6.x)

- **Tydligare kapabilitets- och importdiagnostik:** Kompilatorn skiljer nu på
  okända, duplicerade och saknade modulkapabiliteter, pekar på rätt token vid
  dubbletter och `@requires`, listar giltiga kapabiliteter med förslag för vanliga
  felskrivningar (`io` → `stdio`, `fs` → `filesystem`, `net` → `network`,
  `proc` → `process`, `ffi` → `unsafe`), förklarar hur man lägger till saknad
  `capability` högst upp i modulen och ger specifika fel för okända `std.*`-moduler
  samt direkta funktionsimporter som `import std.io.stdin`.
- **`rune`-aritmetik:** `+`/`-`/`*`/`/`/`%` på `rune` avvisas statiskt
  (jämförelser och explicita konverteringar behålls) så att ogiltiga surrogater
  eller negativa kodpunkter inte kan skapas i säker kod.
- **Closure-lyftning i `closures.ddr`:** Funktioner som returnerar konsumerande
  funktionstyper (`fn mk() -> fn(move []i32)`), closures med flertypade generiska
  returtyper (`Result<T, E>`), och `Self`/`self`-omskrivning inuti
  `impl Trait for Type` samt generiska `impl`-block fungerar korrekt.
- **Täppta kapabilitetsluckor i `sys` och `std.io`:** `sys.net_*` kräver nu
  `@requires(network)` och råa filbeskrivarprimitiv (`sys.fd_read`/`fd_write`/
  `fd_close`/`fd_poll`/`fd_set_nonblocking` samt `std.io.fd_read_chunk`/
  `fd_write_chunk`) är markerade `unsafe`. Motsägande dokumentation i `syntax.md`,
  `memory.md`, `floating.md`, `arkitektur.md` och `sys.md` är uppdaterad.
- **Snabbare testsvit och nivåindelad `verify.ddr`:** `test_mir_scalar` batchar
  nu alla skalärfall i tre samlade binärer via `--batch` i stället för en
  process per instruktion, onödiga kartesiska dubbletter i frontendfel-/matrisloopar
  har gallrats med bibehållen täckning, och `tools/verify-selfhost.sh` samt
  `tools/verify.ddr` kör nivåindelat (`--fast` / standard / `--exhaustive` /
  `--release` / `--all`, plus LPT-ordning) utan att läcka `build/mir-correctness.*`
  eller andra temporära QA-kataloger i `build/`.

### 0.8.0 — Språkergonomi, verktygskedja, prestanda, samtidighet och tjänstemognad

Alla planerade förbättringar från `0.7.0`-, `0.8.0`- och `0.9.0`-färdplanerna är genomförda och släppta samlat som **Dunder `0.8.0`**:

- **Språk- och typergonomi:** Preludiets `Option<T>`, `Result<T, E>`-kombinatorer (`map`, `map_err`, `and_then`, `ok`, `expect`), samordnade `Result`/`Option`-returtyper i `stdlib`, enhetliga `[]u8`-byte-API:er med `string.as_bytes()`, `text[i]: u8`, `text.bytes(): []u8` och scope-bundna lånade UTF-8-strängvyer (`&str` samt `std.text.view`), `[]T.truncate`, generiska metoder i `impl`, `@derive(Eq, Hash, Display, Default)`, fältsnål konstruktion (`Point { x, y }`), bredare `match` (heltal, strängar, tupler, `|`-alternativ och `..`), disjunkta `&mut self`-fältlån, lokala `&mut`-bindningar, `Map::get_mut`/`entry`, muterbara `move`-parametrar, divergensanalys i `unsafe`, råa/flerradiga strängliteraler samt `@embed_file`/`@embed_bytes`.
- **Fristående frontend, verktygskedja och paketsystem:** Uppbruten AST/HIR-frontend med `dunder check` utan kodgenerering, flera fel per körning, varningar, `--agent`-JSON, särskilda felkoder, källkodsutdrag med `^`-spann, `dunder fmt`, `dunder init`/`clean`, `dunder lsp`, `-g`/`--debug` DWARF v4-debuginfo, closure-ergonomi (`Fn`/`FnMut`/`FnOnce`), `Iterator` + `for (i, x)`-enumerate + `0..=n` + `Map`/`Set`-iteration, isolerade paketnamnrymder i `dunder.toml` med per-paket-kapabiliteter, `$DUNDER_HOME`-cache och paketversion i `--app`-bundles.
- **Kodbakändar och körtid:** POD-/stack-värdesemantik, helfunktionsinlining, MIR-SSA som standard, direkta backends till x86-64 (`x86_mir`) och Wasm (`wasm_mir`), dynamiska heap-slabs utan fast 512 MiB-gräns, höjda filinläsningsgränser samt stöd för `wasm32-wasi` och `x86_64-windows` (PE32+).
- **Arenisolerade arbetare utan `async`-färgning:** Kapabilitetsbegränsade I/O-scopes (`task scope ... with (...) arena ...`), `@thread_safe` för granskad stdlib-FFI, flyttbara `Sendable`-`@resource`-handtag och `@resource(dispose)` i `workspace`, återanvänd Linux-arbetarpool, återvunna `()`-jobb, `await_any`, `std.sync` (`Channel<T>`/`Sender`/`Receiver`/`select2`, `Published<T>`/`ReadGuard<T>`, `AtomicI64`/`AtomicBool`/`Semaphore`, `CancelToken`) samt `std.net.PollSet` med `Waker`.
- **Tjänste-, databas-, krypto- och paketmognad:** `std.signal`, moderniserad `std.sqlite` (`Result<T, SqliteError>`, `@resource(dispose)` `Database` och `Transaction`, `SqlValue`, direkta `Row`-läsningar, `exec_batch`), `std.fs` (`set_nofollow`, `chmod`, full `Metadata`, `lstat`/`fstat`, `sync_dir`, atomisk skrivning) + `std.path`, `std.time`, `std.crypto` (`Result`, dynamisk biblioteksladdning, `zeroize`, `SecretBytes`), `std.json` (`Option`/`Result`, `Decode`, `@derive(Encode, Decode)`), samt fristående paket i `packages/` (`http`, `http_client`, `cli` och `log`).

## Kommande förbättringar (efter `0.8.0`)

- **Bredare vektorisering och backend-optimering:** Ytterligare SIMD-/autovektorisering och målspecifik schemaläggning för både `arm64` och `x86_mir`.
- **Fortsatt Windows- och LSP-finputsning:** Ännu bredare stdlib-/paketparitet på `x86_64-windows` samt rikare kodåtgärder och inkrementella analyser i `dunder lsp`.
- **Fristående ekosystempaket:** Ytterligare hjälppaket ovanpå `stdlib` och `packages/` vid behov utan att tynga kärnbiblioteket.
