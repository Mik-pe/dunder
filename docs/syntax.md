# Dunders första implementerade syntax

Beslut 2026-11-10: prioritera det typade källspråket och kompilera riktiga
program. Håll stavning och struktur regelbundna så att både människor och
kodagenter kan använda vanliga mönster för funktioner, uttryck och block.
Detta är ett prövbart designval, inte ett påstående om mätt agentpreferens.

## Deklarationer och muterbarhet

```dunder
count := 42            // härledd i32, oföränderlig
wide: i64 = 42         // explicit i64, oföränderlig
ready: bool = true     // explicit bool, oföränderlig

var total := 0         // explicit muterbar i32
var big: i64 = 0       // explicit muterbar i64
total = total + 1      // tilldelning till befintlig muterbar bindning
```

`:=` deklarerar ett nytt namn i det aktuella blocket och härleder dess typ.
`: Typ =` gör samma sak med en explicit typ. Ett nytt `:=` med samma namn i
samma scope är ett fel. Ett inre block får skugga ett yttre namn.

Bindningar och funktionsparametrar är oföränderliga som standard. `var` är
det explicita valet av muterbarhet. Typen ändras aldrig efter deklarationen.
All deklaration kräver initiering. Det finns inga oinitierade lokaler i denna
delmängd. Oföränderlighet kräver inte att uttrycket kan beräknas vid byggtid.

## Typer och uttryck

| Typ | Betydelse |
| --- | --- |
| `i32` | Signerat 32-bitars heltal; standard för okontextualiserade heltalsliteraler. |
| `i64` | Signerat 64-bitars heltal. |
| `bool` | `true` eller `false`; ingen implicit konvertering till/från heltal. |
| `()` | Inget returvärde; implicit returtyp för funktioner utan `->`. |

Literaler får typ från en annotering, funktionens parameter/returtyp eller
den andra heltalsoperanden. Befintliga värden konverteras aldrig implicit
mellan `i32` och `i64`. `i64(value)` breddar explicit; `i32(value)` smalnar
med en körningskontroll. En literal utanför sitt typintervall är ett typfel.

```dunder
small := 42
large := i64(small) + 1
ok := large == 43
```

Operatorer i fallande prioritet: unärt `-`/`!`, `*`/`/`/`%`, `+`/`-`,
`<`/`<=`/`>`/`>=`, `==`/`!=`, `&&`, `||`. `&&` och `||` kortsluter.
Bool stöder likhet/olikhet och logiska operationer. Heltalsoperationer kräver
matchande typer. Division avrundar mot noll; resten har täljarens tecken.

Spill i `+`, `-`, `*`, unärt minus, divisionen `MIN/-1` och avsmalnande
konvertering avslutar processen med status 101. Division/rest med noll ger
102. Reglerna gäller även optimerade byggen. Wraparound är ännu inte en
språkoperation. Statuskoderna är den första felmekanismen; källkopplade
runtime-diagnoser återstår.

## Funktioner och kontrollflöde

```dunder
fn fibonacci(n: i32) -> i64 {
    var previous: i64 = 0
    var current: i64 = 1
    var i := 0
    while i < n {
        next := previous + current
        previous = current
        current = next
        i = i + 1
    }
    return previous
}

fn main() {
    result := fibonacci(20)
    if result == 6765 {
        print(result)
    } else {
        print(false)
    }
}
```

Parameter- och returtyper i funktionssignaturer är explicita. Framåtanrop,
rekursion och ömsesidig rekursion tillåts. Parametrarna är oföränderliga;
kopiera till en `var`-lokal om de behöver ändras. Funktioner får högst åtta
parametrar i den första backendversionen; fler avvisas med ett tydligt fel.

`main` tar inga argument och returnerar `()` eller `i32`. En `i32`-retur blir
processens exitstatus enligt operativsystemets vanliga regler. Övriga
funktioner med returvärde måste returnera på varje väg. Kontrollen behandlar
loopar konservativt: lägg en retur efter loopen om funktionen returnerar ett värde.

`if` och `while` kräver `bool`, aldrig ett heltal. `else if`, `else`, `break`
och `continue` stöds. Block ger lexikala scope. `print` är den första inbyggda
I/O-operationen: ett `i32`, `i64` eller `bool`, följt av radbrytning.

Vanliga enkla satser avslutas med radbrytning eller valfritt semikolon.
`}` får avsluta den sista satsen i ett block. Parenteser tillåter flerradiga
anrop och uttryck; en rad får fortsätta efter en binär operator.
Kommentarer skrivs `// ...` eller `/* ... */`. Källan är UTF-8 och namn
får innehålla bokstäver som `å`, `ä` och `ö`; nyckelorden är engelska.

## Nästa språkbeslut

Poster och fältåtkomst, strängar, moduler/importer, arrayer/slices, ägande och
lån ska utformas genom riktiga program och feltester. Inga syntaxer för dem
utlovas av denna version. Självhosting och den minimala frökedjan är separata
senare projektsteg och blockerar inte användningen av källspråket här.
