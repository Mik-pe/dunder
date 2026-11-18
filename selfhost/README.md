# Dunderkompilatorn

Kör från projektets rot för att bygga kompilatorn och köra ett program.

```sh
dunder_bin="$(./tools/bootstrap.sh)"
"$dunder_bin" run examples/hello.ddr
```

[Bootstrap](../bootstrap/README.md) beskriver de frysta kompilatorerna och hur
självbyggena jämförs. [Huvudingången](main_parallel.ddr) används på ARM64/macOS,
ARM64/Linux och x86-64/Linux. De tre seriella ingångarna är [main.ddr](main.ddr),
[linux.ddr](linux.ddr) och [x86_linux.ddr](x86_linux.ddr).

## Från källfil till program

1. [Drivrutinen](driver/driver.ddr) läser manifest och väljer mål.
   [Modulladdaren](driver/modules.ddr) löser importer och bevarar källpositionerna
   så att ett fel pekar på rätt fil, rad och kolumn.
2. [Frontenden](frontend/compile.ddr) samlar deklarationerna och typkontrollerar
   också funktioner utan anropare. [Funktionskontexten](frontend/frontend_context.ddr)
   håller det tillstånd som behövs för en kropp. [Ägartillstånd](frontend/ownership_state.ddr)
   och [lånekontroll](frontend/local_borrows.ddr) avgör vilka platser som får läsas,
   flyttas eller ändras. [Minnesmodellen](../docs/memory.md) beskriver språkreglerna.
3. För nåbara funktioner kan [skaläroptimeraren](opt/) förenkla typade uttryck.
   Varje nyemitterad funktion verifieras och sänks genom
   [MIR](mir/mir_pipeline.ddr). [Målbackend](backend/) och [runtime](runtime/)
   tillhandahåller maskinkod, binärformat och körtidsfunktioner.
4. [Länkaren](mir/mir_link.ddr) sammanfogar funktionsobjekten i ordning.
   [Funktionscachen](driver/fragments.ddr) sparar färdiga objekt för
   [inkrementella byggen](../docs/incremental.md). Drivrutinen publicerar
   programfilen först efter ett lyckat bygge.

## Tasks i kompilatorn

[Samordnaren](tasks/compile_parallel.ddr) kan typkontrollera och sänka flera
funktioner samtidigt. Arbetarna delar en fryst deklarationsbild och använder
egna arbetsytor. Samordnaren tar emot resultaten i samma ordning som vid
seriell kompilering. `--jobs 1` kör kedjan på en tråd. `check` stannar
efter semantisk kontroll. [Arbetsbudget och funktionsjobb](../docs/tasks.md#kompilatorn-som-taskkonsument)
· [MIR-pass och verifiering](../docs/optimization.md#gemensam-helfunktionskedja)

## Inbäddat standardbibliotek

Ändra källfilerna i [stdlib](../stdlib/) eller [flyttalsformatteraren](../runtime/float.ddr).
[Modullistan](../stdlib/modules.txt) anger vilka standardmoduler som följer med
kompilatorn. Regenerera [bundled.ddr](bundled.ddr) innan kompilatorn byggs om.

```sh
"$dunder_bin" run selfhost/embed.ddr
```

## Verifiering

`./tools/dev.sh` gör ett inkrementellt bygge och kör kärnsviterna.
För en del av sviten kan du välja testnamn med `--filter`. Den fullständiga
verifieringen börjar med ett nytt självbygge från värdens frö.

Testverktygen använder Python 3.11 eller senare, Node.js, en C11-kompilator
samt Git, curl, gzip och openssl. De nativa biblioteksproven använder även
libcurl, SQLite, OpenSSL 3 och Argon2. [HTTP](../docs/http.md) · [SQLite](../docs/sqlite.md)
· [Kryptobibliotek per plattform](../docs/crypto.md)

```sh
./tools/dev.sh --filter ssa
./tools/verify-selfhost.sh
```

Enskilda testgrupper kan listas, byggas och köras från projektets rot.

```sh
"$dunder_bin" build-tests tests/suite.ddr --list
"$dunder_bin" build-tests tests/suite.ddr --group 0 -o build/group0
./build/group0
```

[Låneproven](../tools/test_local_borrows.ddr) och [fältflyttarna](../tools/test_partial_moves.ddr)
korsbygger de tre nativa målen och kör värdens bilder. Med
`--runner TARGET=COMMAND` kan de även köra en korsbyggd bild i en lokal
målmiljö. Kommandot får bildens absoluta sökväg som sista argument.
[Målplattformar och körmiljöer](../docs/targets.md)
