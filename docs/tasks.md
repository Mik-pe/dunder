# Isolerade uppgifter

`task scope`, `spawn` och `await` kör oberoende arbete på riktiga OS-trådar.
Den första implementationen stöder ARM64/macOS utan runtimeprofilering.
Nåbara taskoperationer på övriga mål avvisas före kodemission.

```dunder
fn sum(values: []i64) -> i64 {
    var total: i64 = 0
    for value in values { total = total + value }
    total
}

fn main() {
    task scope jobs(4) {
        first: TaskID<i64> = spawn jobs sum([1, 2, 3])
        second := spawn jobs sum([10, 20])
        print(await jobs(first))
        print(await jobs(second))
        print(await jobs(first))
    }
}
```

Programmet skriver `6`, `30`, `6`. Kapaciteten är ett positivt `i32`-värde
som begränsar antalet aktiva jobb i scopet. När gränsen är nådd samlar nästa
spawn in det äldsta aktiva jobbet innan ytterligare en tråd startas.
Resultatet sparas tills scopet avslutas.

## Ägare och resultatnycklar

Scopet äger trådar, inputsnapshots och cachade resultat. Det väntar in alla
jobb innan det släpper några av dessa föräldraägda värden. Detta gäller
vanlig blockutgång, `return`, `break` och `continue`. Ett resultat som aldrig
efterfrågas behöver inte kopieras tillbaka från arbetaren.

`TaskID<R>` är en typad 64-bitarsnyckel. Den kan kopieras, returneras och
lagras i arrayer; en kopia skapar ingen ytterligare trådägare. Den är inte
ett heltal och stöder inte heltalskonvertering eller aritmetik. Scope-namnet
är däremot en lexikal kapabilitet som bara får användas med tasksyntaxen.
Det får inte kopieras, lånas eller lämna sitt block.

`await jobs(id)` kontrollerar scope, generation, ordinal och exakt resultattyp
innan jobbets kontrollblock läses. Gamla nycklar fungerar därför inte med ett
nytt scope, även om allokeraren återanvänder samma minnesadress. Generationer
återanvänds aldrig vid overflow. Varje await lämnar en vanlig värdekopia av
det cachade resultatet. Ändring av en returnerad array påverkar varken cachen
eller nästa await.

## Vad en worker får använda

Spawn kräver en namngiven säker Dunder-funktion med exakt en värdeparameter.
Indata och returtyp måste tillhöra den kontrollerade klondomänen:

- Primitiva tal, bool, rune, unit och enums utan payload.
- Ägda skalärarrayer och fasta skalärarrayer.
- Transparenta records vars fält rekursivt uppfyller samma regler.

Strängar, payload-enums, borrows, pekare, funktionsvärden, scratch-vyer,
TaskID:n och oklassificerade opaque-typer får inte passera jobbgränsen.
Dagens vanliga kopieringsväg för strängar och payload-enums delar
referensräknade block, vilket inte uppfyller den första isoleringsregeln.

Arbetaren får använda lokala lån, strängar, scratch och allokering i sin
egen runtime. En separat transitiv effektanalys avvisar unsafe, FFI,
muterbar global lagring, callbacks, omgivningstjänster som I/O och argv,
okända indirekta anrop och nästlad task-scheduling. Även hjälpfunktionernas
anrop och semantiskt kontrollerad död kod ingår. Enbart en funktionssignatur
bevisar inte att en worker är tillåten.

Föräldern skapar först en fristående inputsnapshot. Arbetaren klonar denna
till sin egen pool innan den anropar källfunktionen. Efter lyckad join
klonas resultatet till förälderns pool innan barnpoolen återvinns. Inga
vanliga managed-handtag delas över gränsen och vanlig RC behöver därför
inte göras atomisk för denna modell.

## Fel och kompilering

Felaktig kapacitet eller resultatnyckel samt fel vid create/join/reclaim
avslutar processen med status 112. Ett join-fel bevisar inte att arbetaren
har avslutats; felvägen får därför inte läsa eller frigöra dess lagring.
Vanliga runtimefällor i en worker behåller processavslut som felmekanism.
Cancellation, detach och återhämtning från taskfel ingår inte i första API:t.

Taskprogram får en separat full semantisk certifiering före emission.
Den riktiga emissionen behåller skaläroptimering, inlining och MIR-passen.
Funktionscache avstår för taskprogram tills certifierade effekter och
beroenden kan lagras och återställas tillsammans med fragmenten.
Vanliga seriella program gör ingen extra certifieringsgenomgång.

Kompilatorn använder ännu inte tasks för sina egna funktionspass. Nästa
steg är ägda funktionsjobb och ordnad sammanfogning av relokerbara objekt;
frontendens symboler, diagnostik och inlinebudget behåller en ägare.
Trådarnas nytta ska mätas inklusive kopiering, start, join och sammanfogning.
