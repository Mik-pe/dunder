# Nästa steg för Dunder

Fältflyttar, `replace`, returnerade läslån och lånade intervall är införda.
Arrayer kan äga sammansatta värden och ta bort ett element med bibehållen
ordning. [Minnesmodellen](memory.md) · [Standardbiblioteket](standard.md)

## Samlingar

Nästa steg är generiska `Map<K, V>` och `Set<T>` med statisk specialisering
och uttryckliga krav på vilka värden som får flyttas och klonas. Dagens
`Map` har strängnycklar och `i64`-värden. Dagens `Set` lagrar strängar.

För stora tabeller är det också värt att undersöka om kompilatorn kan
lagra fälten i separata arrayer och ändå låta programmet arbeta med poster.
[Förslag om kolumnlagring](soa-research-plan.md)

## Externa resurser och typade fel

Låt en `@resource`-typ ange hur den stängs vid blockslut, retur, `break`,
`continue` och fel. Definiera hur en explicit stängning samverkar med den
automatiska och om stängningen får blockera. En flytt överför ansvaret
till den nya ägaren.

Inför därefter `Result<T, E>` och felpropagering. Ett anrop som misslyckas
med att ta över en resurs behöver kunna lämna tillbaka den till anroparen.

## Språk och standardbibliotek

Arbetet med Skrift och Hundmat har visat följande behov.

- Förbättra heltalsinferens i arrayliteraler och stöd för runliteraler,
  flervärdesreturer och relativa importer. Se över namn som krockar med
  inbyggda egenskaper och när strängar behöver klonas vid `push`.
- Visa operandvärden när en jämförelse i `assert` misslyckas.
- Utöka stränginterpolering med format för exempelvis hexadecimala tal och
  undersök hur ett projekt kan deklarera körbara exempel i manifestet.
- Lägg till flyttalsparsning i `std.parse`, flyttalsfält i `std.json` och
  ett enklare gränssnitt för att skriva JSON. Samla `clamp`, `min`, `max`
  och trigonometriska funktioner i en matematikmodul.

I `dunder-hundmat/graphics/macos.ddr` återstår texturstöd för en
teckenatlas och läsning av pixlar utan fönster. Med en atlas kan text
ritas utan att varje täckt pixel behöver en egen fyrhörning.
