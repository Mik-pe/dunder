# Isolerade uppgifter

`task scope`, `spawn` och `await` kör oberoende arbete på riktiga OS-trådar
i libdispatchs återanvända trådpool på ARM64/macOS.
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
spawn in det äldsta aktiva jobbet innan nästa jobb lämnas till poolen.
Resultatet sparas tills scopet avslutas.

## Ägare och resultatnycklar

Scopet äger jobbens dispatchgrupper, workerarenor och cachade resultat.
Det väntar in alla aktiva jobb innan de återstående föräldraägda värdena
släpps och arenorna avmappas.
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

`await! jobs(id)` tar över det färdiga resultatets ägarskap. Det går att
först läsa med vanlig `await` och sedan överta originalet med `await!`.
Efter övertagandet ger varje ny läsning av samma nyckel, inklusive kopior
av nyckeln, taskfelet 112. Resultatvärdet överlever scopet som andra ägda värden.

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

Vid `spawn` klonas den typgodkända indatan direkt till en exklusiv
workerarena medan källvärdet fortfarande är lånat på föräldratråden.
Runtimen lämnar över arenans allokerartillstånd och den ägda kopian till
ett jobb i libdispatchs globala trådpool. Skalarer skickas som värdebitar.
En dispatchgrupp per jobb synkroniserar resultatet. Efter avslutat jobb
kopieras det efterfrågade resultatet till förälderns arena. En vanlig
`await` klonar förälderns cache medan `await!` tar över den.
Scopet återanvänder upp till 64 workerarenor om 512 MiB och avmappar
dem vid utgång.

## Fel och kompilering

Felaktig kapacitet eller resultatnyckel samt fel vid skapande, inväntan eller avmappning
avslutar processen med status 112. En misslyckad inväntan bevisar inte att arbetaren
har avslutats; felvägen får därför inte läsa eller frigöra dess lagring.
Vanliga runtimefällor i en worker behåller processavslut som felmekanism.
Cancellation, detach och återhämtning från taskfel ingår inte i första API:t.

Typkontrollen registrerar direkta effekter, anrop och spawnplatser under
den vanliga kroppsvandringen. Både build och check certifierar grafen när
även onåbara kroppar har kontrollerats. Ett bygge binder taskruntimens
symboler efter godkänd certifiering, före publicering. Funktionscachen
avstår för taskprogram tills certifierade effekter och beroenden kan
återställas tillsammans med fragmenten.

## Kompilatorn som taskkonsument

Den taskbaserade kompilatorn är implementerad och självhostad på
ARM64/macOS. Standardingången använder fyra workers; `--jobs N` eller
`--jobs=N` väljer 1–256. Det är värddatorns kompilatorjobb som använder
trådar, även vid korskompilering till Linux eller Wasm. Ett kompilerat
programs egna nåbara taskoperationer kräver fortfarande ARM64/macOS.

En enda `compile_session.Session` behåller frontend, typ- och lånekontroll,
symboler, nåbarhetsordning, provisorisk Code, imports och inlinebudget.
`compile_parallel` skickar funktionslokala operationer och metadata i separata
kolumner i `mir_task.Batch`. Varje arbetstråd bygger MIR lokalt, verifierar,
optimerar och sänker funktionerna i batchen med återanvända arbetsarenor.
Svaret innehåller slutobjekt och statistik i `mir_task.BatchAnswer`.
Föräldern tar över svaren med `await!` och länkar i källordning. [Batchformat och worker](../selfhost/mir_task.ddr)

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

`check`, `--mir=off` och `--profile-compiler` använder den
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
