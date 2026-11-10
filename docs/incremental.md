# Snabba återbyggen och agentutdata

```sh
dunder check examples/functions.ddr --agent
dunder build examples/functions.ddr --incremental --timings
dunder test --affected
```

## Läs kontrollresultat som JSON

`check --agent` skriver ett JSON-objekt per rad på stdout. En lyckad
kontroll rapporterar namn och typ för parametrar och lokala bindningar,
sedan en post med `kind: "ok"` och antal moduler. Ett källfel ger en post
med `kind: "error"`, felkod, sökväg, meddelande och spann samt exitkod 1.
`start` och `end` är nollbaserade UTF-8-byteoffset i den angivna filen.
Slutpositionen är exklusiv. `--target wasm32` väljer Wasms pekarbredd även
vid typkontrollen.

## Återanvänd maskinkod

`--incremental` aktiverar funktionscachen för nativebyggen och `run`.
`--timings` rapporterar hur många nåbara funktioner som återanvändes och hur
många som emitterades. Cachen finns i `build/.dunder-cache/functions` under
aktuell arbetskatalog. Man kan radera hela katalogen mellan två byggen.

Parser och typkontroll läser hela importträdet. Kompilatorn hashar varje
typad funktionskropp och inkluderar typ- och signaturtabeller, mål, de
inlinade beroendena och bildens import- och datalayout i nyckeln.
Ändringar i kompilatorn, dess runtime eller inbäddade bibliotek ogiltigförklarar
äldre fragment. En ändring i delad layout kan därför bygga om flera funktioner.

Ett fragment lagrar instruktioner före slutlig placering, hopp med symboliska
mål, strängliteraler och datareferenser. Länkningen placerar om dem i den
aktuella bilden och signerar Mach-O på nytt. En kontrollsumma identifierar
ofullständiga cachefiler och publicering sker med ett atomiskt namnbyte.
x86-64-backenden återanvänder den gemensamma instruktionsströmmen och sänker
den till x86 vid länkning.

## Kör berörda tester

`test --affected` kör alla grupper första gången. Därefter återanvänder den
godkända resultat för grupper vars transitivt nåbara funktionskroppar och
typkontext matchar. Även funktioner som används som värden ingår. En ändrad
tidsgräns ger en ny körning. Tester med nåbara systemanrop, råpekare eller
externa anrop körs varje gång eftersom omvärlden kan ha ändrats.
Misslyckade grupper behöver en ny godkänd körning. Vanlig `dunder test` kör
alltid hela sviten.

Resultatkvitton och kodfragment finns i den valda testutdatakatalogen
(`-o`). Sammanfattningen anger hur många grupper som hoppades över och hur
många som gick igenom eller misslyckades.
