# Dogfooding av Dunder

Det här är Dunder-program som kompileras och körs med Dunders egen backend.
De används också av kompilatorns integrationstester, med kontrollerad stdout
och definierade felstatusar. Importer skrivs utan filändelse.

Kör hela testsamlingen från reporoten:

```sh
cargo run --release -- test
# 18 passed, 0 failed
```

Manifestets `test_root = "dogfood/tests.dn"` väljer testträdet. Enskilda
program och deras tester går också att köra direkt:

```sh
cargo run --release -- run dogfood/structs.dn
cargo run --release -- test dogfood/geometry.dn
cargo run --release -- test dogfood/parameter_cases.dn
```

Med kompilatorn kopierad till `build/dunder` används samma kommandon som
`./build/dunder run ...` och `./build/dunder test ...`.

| Program | Vad det provar | Förväntade stdout-rader med `dunder run` |
| --- | --- | --- |
| [collections.dn](collections.dn) | Arrayändring, intervall, iteratorer och längd. | `-2`, `-1`, `0`, `1`, `13`, `3` |
| [recursion.dn](recursion.dn) | Rekursiv Fibonacci och Euklides algoritm. | `6765`, `21` |
| [sorting.dn](sorting.dn) | Insertion sort, index, kopierad arrayparameter, for/while och ordningskontroll. | `45`, `9` |
| [iterators.dn](iterators.dn) | Fold med callback, återanvändbar iterator, continue och returnerat intervall. | `36`, `20`, `0` |
| [arena/main.dn](arena/main.dn) | Bump-allokering, kapacitetsfel och rekursiv traversering över tre moduler. | `28` |
| [structs.dn](structs.dn) | Exporterad struct, uttryck som input, värdekopia och fältändring. | `25`, `32`, `3` |
| [utf8.dn](utf8.dn) | UTF-8, byte/kodpunkter, ordräkning, strängretur och text i structar. | `bytes=25`, `runes=20`, `words=5`, `🌍 Hej världen` |
| [tokenizer.dn](tokenizer.dn) | UTF-8-ord, ASCII-skiljetecken och citerade strängar med escapes. | `fn\|hälsa\|(\|namn\|)\|{\|print\|(\|"Hej 🌍"\|)\|}` |

[parameter_cases.dn](parameter_cases.dn) testar `(20) -> 4`, uttryck på
båda sidor, flera argument och genererade fall i en loop.
[geometry.dn](geometry.dn) testar structargument direkt och genom en lokal
bindning. De två filerna är bibliotek utan `main` och körs med `dunder test`.
[tests.dn](tests.dn) samlar importträdet och lägger till två modulsamspelstest.

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
Structfält och arrayelement är ännu begränsade till `i32`, `i64`, `bool`, `string` och `rune`.
Aggregatreturer och användardefinierade iteratorer återstår.

[text/scan.dn](text/scan.dn) implementerar `find`, `replace`, ordräkning,
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
