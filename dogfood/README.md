# Dogfooding av Dunder

Det här är Dunder-program som kompileras och körs med Dunders egen backend.
De används också av kompilatorns integrationstester, med kontrollerad stdout
och definierade felstatusar. Importer skrivs utan filändelse.

Det fristående [Tabell-projektet](../projects/tabell/README.md) använder också
Dunder för ett riktigt CSV-CLI med eget manifest, parser, gruppering och
diagnostik. Det driver importer utan citattecken, strängsortering och stderr.

Kör hela testsamlingen från reporoten:

```sh
cargo run --release -- test
# 58 testgrupper i det sammanslagna trädet
```

Manifestets `test_root = "dogfood/tests.ddr"` väljer testträdet. Enskilda
program och deras tester går också att köra direkt:

```sh
cargo run --release -- run dogfood/structs.ddr
cargo run --release -- test dogfood/geometry.ddr
cargo run --release -- test dogfood/parameter_cases.ddr
```

Med kompilatorn kopierad till `build/dunder` används samma kommandon som
`./build/dunder run ...` och `./build/dunder test ...`.

| Program | Vad det provar | Förväntade stdout-rader med `dunder run` |
| --- | --- | --- |
| [collections.ddr](collections.ddr) | Arrayändring, intervall, iteratorer och längd. | `-2`, `-1`, `0`, `1`, `13`, `3` |
| [recursion.ddr](recursion.ddr) | Rekursiv Fibonacci och Euklides algoritm. | `6765`, `21` |
| [sorting.ddr](sorting.ddr) | Insertion sort, index, kopierad arrayparameter, for/while och ordningskontroll. | `45`, `9` |
| [iterators.ddr](iterators.ddr) | Fold med callback, återanvändbar iterator, continue och returnerat intervall. | `36`, `20`, `0` |
| [arena/main.ddr](arena/main.ddr) | Bump-allokering, kapacitetsfel och rekursiv traversering över tre moduler. | `28` |
| [structs.ddr](structs.ddr) | Exporterad struct, uttryck som input, värdekopia och fältändring. | `25`, `32`, `3` |
| [numeric/main.ddr](numeric/main.ddr) | f32-vektor, kompenserad f64-summering och rotberäkning med callback. | `5.0`, `2.0`, `true` |
| [filesystem.ddr](filesystem.ddr) | Medföljande sys, UTF-8-fil, append, metadata, rename och städning. | `18`, `filesystem ok` |
| [enums.ddr](enums.ddr) | Namngivna värden, payloads, uttömmande match och nästlade Ok/Err. | `42`, `enums ok` |
| [standard.ddr](standard.ddr) | Word counter med map/set, dynamiska arrayer, text, byte, SHA-256 och decimalparser. | `4`, `6`, `stdlib ok` |
| [utf8.ddr](utf8.ddr) | UTF-8, byte/kodpunkter, ordräkning, strängretur och text i structar. | `bytes=25`, `runes=20`, `words=5`, `🌍 Hej världen` |
| [tokenizer.ddr](tokenizer.ddr) | UTF-8-ord, ASCII-skiljetecken och citerade strängar med escapes. | `fn\|hälsa\|(\|namn\|)\|{\|print\|(\|"Hej 🌍"\|)\|}` |
| [metal/triangle.ddr](metal/triangle.ddr) | Nativefönster, en gradienttriangel och en Dunder-skriven händelseloop. | `triangle frames=N` med `--frames N`; kräver grafisk session. |
| [metal/main.ddr](metal/main.ddr) | Metal-scen med moduler, enumtillstånd, animation, dragning och tangentkommandon. | `atlas frames=N` med `--frames N`; kräver grafisk session. |

[parameter_cases.ddr](parameter_cases.ddr) testar `(20) -> 4`, uttryck på
båda sidor, flera argument och genererade fall i en loop.
[geometry.ddr](geometry.ddr) testar structargument direkt och genom en lokal
bindning. De två filerna är bibliotek utan `main` och körs med `dunder test`.
[tests.ddr](tests.ddr) samlar importträdet och lägger till två modulsamspelstest.

```dunder
@test {
    (Point { x: 3, y: 4 }) -> 25
    (Point { x: 6 / 2, y: 2 + 2 }) -> 20 + 5
    point := Point { x: 3, y: 4 }
    length_squared(point) == 25
}
pub fn length_squared(p: Point) -> i32 {
    p.x * p.x + p.y * p.y
}
```

Varje kopplat block eller `@test fn` är en testgrupp som körs i en egen
nativeprocess. Bool-uttryck blir assertions. Första felet avbryter gruppen,
och runnern fortsätter med nästa. Tester från gemensamma importer körs en
gång. Testbyggen behöver ingen `main` och kör inte programmets `main`.
Standardtidsgränsen är fem sekunder per grupp; `--timeout-ms` ändrar den.

Arrayer, structar och arrayiteratorer har värdesemantik och kopieras vid
bindning och anrop. Iteratorer tar en kopia av arrayen; de lånar den inte.
Arrayelement stöder `i32`, `i64`, `f32`, `f64`, `bool`, `string`, `rune`
och enums utan payload. Structfält kan också innehålla nästlade aggregat,
dynamiska arrayer och funktionsvärden.
Structar, fasta arrayer och data-enums kan returneras med värdesemantik.
Användardefinierade iteratorer återstår.

[Standardbiblioteket](../docs/standard.md) är Dunder-kod, inbäddad i
kompilatorn och importerad som `std.map`, `std.set` och så vidare.
`standard.ddr` kopplar ihop samlingarna i en UTF-8-ordräknare. Dess två
testgrupper och bibliotekens nio grupper ingår i huvudträdet.
Även den [Dunder-skrivna kompilatorns](../selfhost/README.md) lexer och
typkontroll har egna taggade grupper här. Den bygger skalära nativeprogram
men kan ännu inte kompilera hela sin egen källkod.

[filesystem.ddr](filesystem.ddr) använder `import sys`, som är inbäddat i
kompilatorn. Programmet skapar en egen katalog med process-ID, provar
filflödet och städar de kända objekten. Det har två testgrupper, och sys
lägger till två grupper för felmeddelanden och typade felkoder.
Kör `dunder test dogfood/filesystem.ddr` för alla fyra.
`sys.read` ger `Ok(string)` / `Err(IoError)`; övriga resultat har `.ok` och
`.error`. Vanliga OS-fel hanteras i programmets kontrollflöde.
Läs [sys-kontraktet](../docs/sys.md) för textgränser och skrivbeteende.

[enums.ddr](enums.ddr) har tre egna testgrupper för payload-utvärdering,
nästlade resultat och namngivna värden i arrayer och structfält. Med sys
blir det fem grupper: `dunder test dogfood/enums.ddr`.
Taggar och direkta grenar används utan vtables. Se [enumreglerna](../docs/enums.md).

[numeric/math.ddr](numeric/math.ddr) testar vektorlängd, omskalad hypotenusa,
toleranser och kompenserad summering. [numeric/ode.ddr](numeric/ode.ddr)
integrerar en differentialekvation med Runge–Kutta och en typad callback.
Tillsammans med rotberäkning och precisionsprov i main finns sju numeriska
testgrupper. Kör `dunder test dogfood/numeric/main.ddr`.
Även [flyttalsutskriften](../runtime/float.ddr) är skriven i Dunder och
kompileras in i program som skriver flyttal. Se [reglerna](../docs/floating.md).

[metal/scene.ddr](metal/scene.ddr) och [metal/math.ddr](metal/math.ddr) driver
nativegrafikstödet. Fyra grupper i huvudträdet testar geometrihjälparnas
vertexlayout, scenens ändliga triangelista och återanvända lagring,
paus/palett/dragning samt sinus/cosinus i Dunder. De öppnar inget fönster.
`dunder test dogfood/metal/main.ddr` kör sju grupper inklusive delade sys-/
parse-tester. De två körbara exemplen och deras GUI-kontrakt beskrivs i
[macOS-/Metal-guiden](../docs/macos-metal.md).

[vulkan/main.ddr](vulkan/main.ddr) kör motsvarande Atlas-scen på Linux med
Vulkan. Geometri, animation och ASCII-kontroller är Dunder-kod. Headless-läge
och PPM-capture använder samma pipeline som fönsterläget. Fem nya grupper
provar geometri, scen, matematik och programargument utan grafikdrivrutin.
Pixel- och livscykeltester körs separat med lavapipe; se
[Linux/Vulkan-guiden](../docs/linux-vulkan.md).

[text/scan.ddr](text/scan.ddr) implementerar `find`, `replace`, ordräkning,
UTF-8-gränser och avkodning i Dunder. Både textprogrammet och tokenizern
importerar samma modul; dess fyra testgrupper körs en gång i hela trädet.
Blanktecken är en uttryckligt listad delmängd. Tokenizern bevarar stavning
och citattecken, men är inte hela Dunder-lexern eller en fullständig
klassificering av Unicode-identifierare. Outslutna citat ger en assertion.

Strängmodellen lånar uppdelningen mellan byte och runes från
[V](https://docs.vlang.io/v-types.html#strings). `.len` räknar byte,
`.runes().len` räknar Unicode-kodpunkter och `s[a..b]` kräver UTF-8-gränser.
`.bytes()` och `.runes()` är vyer över oföränderlig text, inte allokerade
arrayer. En sammansatt emoji kan innehålla flera runes. Ingen normalisering
eller grapheme-klusterhantering görs.

Dynamiska strängar använder en separat 16 MiB processarena. Kopior kan
dela oföränderliga data; sammanslagning, snitt och vissa `.str()`-anrop
allokerar nya värden. Gamla värden återvinns först vid processlut, så många
upprepade operationer kan fylla arenan och ge status 105. Inga anspråk på
optimerad textbearbetning eller färdig minneshantering görs här.

Arenan använder en fast `[i32; 96]` på stacken. `pool.reserve` kontrollerar
kapacitet och flyttar markören; den gamla markören är ett indexhandtag.
Varje trädnod upptar tre platser: värde, vänster index, höger index.
`-1` betyder tom länk. Det är fungerande bump-allokering inom den bufferten,
men inget allmänt allokerings-API eller färdig minnesmodell för språket. Bufferten
kopieras i dagens rekursiva anrop; detta är ett korrekthetsprov, inte ett
prestandaanspråk.
