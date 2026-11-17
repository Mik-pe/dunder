# Nästa steg för Dunder

Fältflyttar, `replace`, returnerade läslån och lånade intervall är införda.
Arrayer kan äga sammansatta värden och ta bort ett element med bibehållen
ordning. [Minnesmodellen](memory.md) · [Standardbiblioteket](standard.md)

## Samlingar

Nästa steg är generiska `Map<K, V>` och `Set<T>` med statisk specialisering
och uttryckliga krav på vilka värden som får flyttas och klonas. Dagens
`Map` har strängnycklar och `i64`-värden. Dagens `Set` lagrar strängar.

[Undersökningen av kolumnlagring](soa-research-plan.md) visar hur en
följeslagartyp kan ge poståtkomst och kolumnlån med befintliga arrayer.
Prototypen passerade med den självbyggda kompilatorn som även avvisade
skrivning under ett aktivt kolumnlån.

## Externa resurser och typade fel

Låt en `@resource`-typ ange hur den stängs vid blockslut, retur, `break`,
`continue` och fel. Definiera hur en explicit stängning samverkar med den
automatiska och om stängningen får blockera. En flytt överför ansvaret
till den nya ägaren.

Inför därefter `Result<T, E>` och felpropagering. Ett anrop som misslyckas
med att ta över en resurs behöver kunna lämna tillbaka den till anroparen.

## Språk och standardbibliotek

Arbetet med Skrift och Hundmat har visat följande behov.

- Förbättra heltalsinferens i arrayliteraler och inför flervärdesreturer.
  Se över namn som krockar med inbyggda egenskaper och när strängar
  behöver klonas vid `push`.
- Visa operandvärden när en jämförelse i `assert` misslyckas.
- Utöka stränginterpolering med format för exempelvis hexadecimala tal.
- Samla `clamp`, `min`, `max` och trigonometriska funktioner i en
  matematikmodul.

[Runliteraler](syntax.md#utf-8-strängar-och-runes) och
[relativa importer](syntax.md#moduler-och-importer) stöds. `std.parse`
läser decimaler till `f32` och `f64`.
[`std.json`](standard.md#skriva-json) har flyttalsfält och en skrivare
som sköter strukturen för objekt och arrayer.

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

I `dunder-hundmat/graphics/macos.ddr` återstår texturstöd för en
teckenatlas och läsning av pixlar utan fönster. Med en atlas kan text
ritas utan att varje täckt pixel behöver en egen fyrhörning.
