# Dogfooding av Dunder

Det här är Dunder-program som kompileras och körs med Dunders egen backend.
De används också av kompilatorns integrationstester, med kontrollerad stdout
och definierade felstatusar. Importer skrivs utan filändelse.

Kör hela testsamlingen från reporoten:

```sh
cargo run --release -- test
# 12 passed, 0 failed
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
Structfält och arrayelement är ännu begränsade till `i32`, `i64` och `bool`.
Aggregatreturer och användardefinierade iteratorer återstår.

Arenan använder en fast `[i32; 96]` på stacken. `pool.reserve` kontrollerar
kapacitet och flyttar markören; den gamla markören är ett indexhandtag.
Varje trädnod upptar tre platser: värde, vänster index, höger index.
`-1` betyder tom länk. Det är fungerande bump-allokering inom den bufferten,
men ingen heapallokering eller färdig minnesmodell för språket. Bufferten
kopieras i dagens rekursiva anrop; detta är ett korrekthetsprov, inte ett
prestandaanspråk.
