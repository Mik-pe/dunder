# Isolerade uppgifter

`task scope`, `spawn` och `await` kör oberoende arbete på riktiga OS-trådar
via pthreads på ARM64/macOS.
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

Scopet äger trådar, inputsnapshots, poolade workerarenor och cachade
resultat. Varje jobbs `input_snapshot` frigörs tidigt så snart jobbet
samlas in, medan scopet väntar in samtliga jobb innan återstående
föräldraägda värden släpps och poolade workerarenor avmappas vid utgång.
Detta gäller vanlig blockutgång, `return`, `break` och `continue`. Ett
resultat som aldrig efterfrågas behöver inte kopieras tillbaka från arbetaren.

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
- Ägda och fasta arrayer med elementtyperna ovan.
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

Föräldern skapar först en fristående `input_snapshot`. Arbetaren klonar denna
till en workerarena om 512 MiB innan den anropar källfunktionen. Efter lyckad
join klonas resultatet till förälderns pool, jobbets `input_snapshot`
frigörs direkt vid insamlingen och barnarenan återställs till en
scope-lokal pool med upp till 64 återanvändbara workerarenor för
efterföljande jobb inom samma scope (samtliga avmappas när scopet
stängs). Inga vanliga managed-handtag delas över gränsen och vanlig RC
behöver därför inte göras atomisk för denna modell.

## Fel och kompilering

Felaktig kapacitet eller resultatnyckel samt fel vid create/join/reclaim
avslutar processen med status 112. Ett join-fel bevisar inte att arbetaren
har avslutats; felvägen får därför inte läsa eller frigöra dess lagring.
Vanliga runtimefällor i en worker behåller processavslut som felmekanism.
Cancellation, detach och återhämtning från taskfel ingår inte i första API:t.

Taskprogram får en full semantisk certifiering före kodemission. Vid
vanlig `dunder check` (utan separat agentrapport) certifieras
tasksemantiken direkt i en enda genomgång (`in_place_certify`); vid
kodbygge körs certifieringen först, varefter den riktiga emissionen
behåller skaläroptimering, inlining och MIR-passen. Funktionscache avstår
för taskprogram tills certifierade effekter och beroenden kan lagras och
återställas tillsammans med fragmenten. Vanliga seriella program gör
ingen extra certifieringsgenomgång.

## Kompilatorn som taskkonsument

Den taskbaserade kompilatorn är implementerad och självhostad på
ARM64/macOS. Standardingången använder fyra workers; `--jobs N` eller
`--jobs=N` väljer 1–256. Det är värddatorns kompilatorjobb som använder
trådar, även vid korskompilering till Linux eller Wasm. Ett kompilerat
programs egna nåbara taskoperationer kräver fortfarande ARM64/macOS.

En enda `compile_session.Session` behåller frontend, typ- och lånekontroll,
symboler, nåbarhetsordning, provisorisk Code, imports och inlinebudget.
`compile_parallel` strömmar ägda MIR-funktioner i ett enda `task scope`
som ett FIFO-glidfönster med plattpackade `mir_task.Batch`-jobb och
motsvarande `mir_task.BatchAnswer`-svar (adaptiv batchstorlek mellan 2 och 8
funktioner), vilket håller ned antal `spawn`/`await`-omgångar. Arbetarna
återanvänder scopets poolade workerarenor och kör samma `mir_pipeline` som
den seriella drivrutinen — med `mir_pipeline.CoreWorkspace` som hoppar
över SSA-arbetsminne när `--mir-ssa=off` — följt av verifiering,
framepromovering, valbar SSA och värdepass samt nödvändig efterverifiering.
Samma worker sänker sedan varje funktion med den gemensamma `mir_lower`-gången
direkt till ägd numerisk `ObjectData`. Varje `spawn` skapar en OS-tråd
som arbetar mot återanvänd sänknings-/pipelinearena ur scopets pool.
Resultatet innehåller slutobjekt och status/statistik, inte hela
MIR-funktionen. Varken `Session` eller globala tabeller delas muterbart.

`ObjectData` är en vanlig transparent post med primitiva arenor och går
genom språkets befintliga snapshot-/klonregler. Den ger inget undantag för
opaque-typer eller delat managed-minne. Föräldern kontrollerar version,
ursprungsspann, plattform, lokala ord-/byteoffsetar, gränskarta, etiketter,
relokeringar och import-site-ID:n före ordnad acceptans. Strukturell
validering certifierar inte godtyckliga maskinords semantik: resultatet
måste dessutom höra till exakt det utlämnade jobbet i samma taskscope,
ordinal och deklaration. Föräldern kör ingen andra sänkning; den äger cache,
ordnad länkning och slutlig bildpublicering. Frontend och typning är
fortfarande seriella hos Session-ägaren.

Cacheträffar lämnar redan färdiga objekt utan ny MIR-verifiering eller
optimering. Cacheformat och objektstruktur valideras fortfarande. Beroenden och budgetdebitering återspelas i samma ordning
som vid ett kallt bygge. Tidigare jobb tas emot före senare källfel, så
färdigordningen ändrar inte första diagnostiken. Workerantalet ingår inte
i cacheidentiteten och ändrar inga optimeringsbudgetar.

`--jobs=1`, `check`, `--mir=off` och `--profile-compiler` använder den
identiska seriella Session-drivrutinen. `check` behöver inga backendarenor.
Kompilatorns funktionscache fungerar också med parallell MIR-bearbetning;
begränsningen ovan gäller källprogram som själva innehåller tasksyntax.

Fulluttryckets exakt identifierade, unikt ägda recordtemporär kan överföras
utan en extra djupkopia inom samma runtime. Den generiska normaliseringen
bevarar nästlade arrayers synliga kopieringskapacitet; strängar, delade
enums och inline C-layout är stoppunkter. Lokaler och fältprojektioner
behåller värdesemantiken. Detta inför inga allmänna moves och tar inte bort
isoleringskopiorna vid spawn/join.

Den historiska [parallella kompilatormätningen](../benchmarks/2026-11-12-parallel-compiler/README.md)
skiljer samma binär med en respektive fyra workers från jämförelsen med
den dåvarande seriella kompilatorn. Rapportens genererade bygge med fyra
workers nådde p95 619,768 ms, motsvarande 163 003 källrader/s. Runtimebilderna är
identiska; runtime/C-geomedlet är 1,178× och största kvoten 1,308×.
Det återstående C-målet löses inte av snabbare kompilering. Rapporten
föregår regional SSA och worker-lokal sänkning och mäter inte deras kostnad.

Den tidigare [språkmätningen med 30 rundor](../benchmarks/2026-11-12-isolated-tasks/README.md)
visade 3,59× och 3,78× parad förbättring i två rena taskarbetsfall. Den
redovisar separat vad själva taskstödet kostade seriella program.
