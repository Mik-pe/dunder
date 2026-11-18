# Nästa steg för Dunder

Fältflyttar, `replace`, returnerade läslån och lånade intervall är införda.
Arrayer kan äga sammansatta värden och ta bort ett element med bibehållen
ordning. [Minnesmodellen](memory.md) · [Standardbiblioteket](standard.md)

## Samlingar

[`Map<K, V>` och `Set<T>`](standard.md#generiska-samlingar) stöder
statiskt specialiserade typer, lånade uppslag och flytt av ägda värden.
Proven täcker hashkollisioner, omhashning, resurser och kloningsdiagnostik.

[Undersökningen av kolumnlagring](soa-research-plan.md) visar hur en
följeslagartyp kan ge poståtkomst och kolumnlån med befintliga arrayer.
Prototypen passerade med den självbyggda kompilatorn som även avvisade
skrivning under ett aktivt kolumnlån.

## Externa resurser och typade fel

[Automatisk stängning](memory.md#automatisk-stängning) kan deklareras med
`@resource(dispose)`. Den följer ägaren genom flytt och körs synkront när
ägaren släpps. TCP-handtagen och `StreamSet` använder den.

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

`std.net` samlar TCP-transporten, inklusive läsning av byte som ligger
kvar i socketens kö. HTTP-servern och klienten byggs som separata paket
med protokollprov. Migrera användare till paketberoenden innan
övergångsimporterna `std.http` och `std.http_client` tas bort.
[HTTP-paketen](http.md)

### Körbara exempel i manifestet

En provkörning från projektets underkatalog `examples` bekräftade att
`dunder run named.ddr` hittar projektets lokala beroenden. `dunder run`
väljer fortfarande `root` och `dunder test` väljer `test_root`.
[Dagens manifestparser](../selfhost/driver/manifest.ddr) avvisar
sektionen `[examples]` med en diagnostik på sektionsraden.

Om projekten behöver namngivna exempel kan manifestet mappa namn till
rotfiler på samma sätt som det i dag anger `root` och `test_root`.

```toml
[examples]
atlas = "examples/atlas.ddr"
```

Ett framtida `dunder run --example atlas` kan välja den rotfilen relativt
manifestet och använda samma beroendegraf och låsfil som projektet.
`build` och `check` kan dela väljaren. Utdata under
`build/<mål>/examples/<namn>` skiljer exempel med samma filnamn
och undviker kollisioner med huvudprogrammet. `root` och `test_root`
fortsätter vara standardvalen.

## Grafik

Hundmats macOS-prov testar Dunders FFI med en Metal-texturatlas och
BGRA8-läsning utan fönster. [macOS-modulen i provet](https://github.com/Mik-pe/dunder-hundmat/blob/7c655764bf5e9a135811aa6bfa50f152f2f98e92/dogfood/graphics/macos.ddr)

Skrift ritar varje packat textprov med en fyrhörning. Verifieringen
körde Skrift i fönster och utan fönster och kontrollerade färgkanaler,
radorientering och alfablendning i [pixelprovet](https://github.com/Mik-pe/dunder-hundmat/blob/7c655764bf5e9a135811aa6bfa50f152f2f98e92/dogfood/metal/pixels.ddr).
