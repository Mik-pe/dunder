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


## Kommande förbättringar

### Diagnostik och kompilator

- **Enummönster med nyttolast:** När en variant som bär ett värde (t.ex. `.Err`
  i `Result<T, string>`) matchas utan parenteser bör felet peka på själva
  mönstret och föreslå `.Err(e)` i stället för `expected '('` inne i armens
  kropp.
- **Metodanrop utan parenteser:** När en okänd fältåtkomst matchar en befintlig
  metod (t.ex. `parsed.is_ok`) bör felmeddelandet föreslå metodanropet
  (`parsed.is_ok()`).
- **Oföränderlig bindning vid tilldelning:** Fel som `assignment requires a
  mutable stored value` bör inkludera en notering med radnummer för den
  ursprungliga `:=`-deklarationen.
- **Tydligare typfel och `&mut`-vidarebefordran:** Skriv ut vilken typ som är
  förväntad respektive given i `type mismatch`-fel i stället för `A / B`. När
  en `&mut`-parameter autoderefereras till sitt värde vid vidarebefordran bör
  diagnostiken föreslå `&mut namn`.
- **Effektanalys i task-arbetare:** När en arbetare avvisas för ambient I/O
  eller systemanrop bör diagnostiken peka ut det faktiska anropet (och vid
  behov anropskedjan) i stället för enbart arbetarfunktionen.
- **Slicing av dynamiska arrayer:** Förbättra felmeddelandet för `arr[a..b]`
  (som i dag ger `slice requires a string`) så att det hänvisar till lånat
  intervall `&arr[a..b]`, kloning eller `array.append` — eller utred stöd för
  ägd skivning.
- **`mut` i `match`-mönster:** Stöd föränderliga mönsterbindningar som
  `.Ok(mut x)` i `match`, eller ge ett tydligt felmeddelande som föreslår
  `.Ok(x) => { mut s := x; ... }` i stället för `expected '('` eller
  `match pattern requires an enum`.
- **Ignorerad `()`-retur (`_ := ...`):** När `_ := expr` avvisas med
  `local requires a value` för ett `()`-uttryck (t.ex. `sys.sleep_ms(...)`),
  lägg till en hänvisning om att anropa uttrycket direkt utan `_ :=`.

### Ägande och lån

- **Mer precisa lån och färskhetsregler vid `push`:** Se över konservativa
  lån där ett tidigare läslån (t.ex. `.len`, `==` eller ett funktionsanrop med
  lånat argument) blockerar en senare flytt i samma block, samt att
  loopbindningar i `for x in move batch` inte känns igen som färska ägda värden
  vid `push`. Dokumentera de exakta reglerna tydligt i [memory.md](memory.md).
- **Flytt av `@resource`-nyttolast ur `match`:** Gör det möjligt att flytta
  nyttolaster (särskilt `@resource`-värden som `Child` i `Result<Child, E>`) ur
  ägda `match`-värden på samma sätt som fältflyttar ur lokala structar, och
  föreslå aldrig `.clone()` i felmeddelandet för icke-klonbara resurser.
  Dokumentera gränsen mellan fältflytt och mönsterbindning i
  [memory.md](memory.md) och [enums.md](enums.md).

### Standardbibliotek och formatering

- **Stränghjälpare i `std.text`:** Komplettera `std.text` med vanliga
  CLI-funktioner som `contains`, `replace`, ASCII-`lower`/`upper` och
  utfyllnad (`pad_left`/`pad_right`).
- **Strängbredd i f-strängar:** Utöka formatangivelserna i stränginterpolering
  med bredd- och justeringsstöd för strängar (motsvarande `:<N` / `:>N`),
  utöver befintlig heltalsformatering.
- **`std.json.Doc.raw_span`:** Dokumentera hur `raw_span` beter sig per nodtyp
  (strängnoder saknar citattecken och har kvar escape-sekvenser) och överväg en
  `raw_json(node)`-metod som returnerar giltig JSON-källtext för alla nodtyper.
- **`std.process.ChildSet::wait_any`:** Förtydliga (eller se över namnet på)
  `wait_any`, som returnerar antalet färdiga filbeskrivare från `poll` snarare
  än en färdig slot.

### CLI och dokumentation

- **`dunder install`:** Lägg till ett `dunder install`-kommando för att bygga
  och installera det aktuella Dunderpaketets binär till samma
  installationskatalog som `dunder` (eller en konfigurerbar målkatalog i
  `PATH`).
- **Underkatalog för Git-beroenden (`subdir`):** Stöd ett valfritt
  `subdir`-fält för Git-källor i `dunder.toml` så att paket i ett monorepo
  (t.ex. `packages/http`) kan bero på direkt utan lokal kopiering.
- **Arbetskatalog vid `dunder run`:** Säkerställ att den byggda binären körs i
  anroparens aktuella arbetskatalog (eller dokumentera avvikelsen tydligt).
- **Testutdata i `dunder test`:** Skilj lokala tester från standardbibliotekets
  egna `@test`-funktioner i sammanräkningen (eller kör bara paketets tester
  som standard).
- **Dokumentationsförtydliganden:** Visa parenteser för metoder på
  `Result<T, E>` (`.is_ok()`, `.is_err()`, `.unwrap_or(...)`) och förtydliga
  `.Err` med `()`-nyttolast i [standard.md](standard.md); dokumentera att
  `spawn` tar exakt ett säkert värdeargument (samt mönstret att samla argument
  i en `Sendable`-struct) i [tasks.md](tasks.md); dokumentera jokertecken `_`
  i `match`-mönster (t.ex. `.Err(_)`) i [syntax.md](syntax.md); och förtydliga
  skillnaden mellan `&arr[a..b]` och `text[a..b]`.
