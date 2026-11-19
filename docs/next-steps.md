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

## Grafik

Hundmats macOS-prov testar Dunders FFI med en Metal-texturatlas och
BGRA8-läsning utan fönster. [macOS-modulen i provet](https://github.com/Mik-pe/dunder-hundmat/blob/7c655764bf5e9a135811aa6bfa50f152f2f98e92/dogfood/graphics/macos.ddr)

Skrift ritar varje packat textprov med en fyrhörning. Verifieringen
körde Skrift i fönster och utan fönster och kontrollerade färgkanaler,
radorientering och alfablendning i [pixelprovet](https://github.com/Mik-pe/dunder-hundmat/blob/7c655764bf5e9a135811aa6bfa50f152f2f98e92/dogfood/metal/pixels.ddr).
