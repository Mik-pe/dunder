# Isolerade uppgifter

`task scope`, `.spawn` och `.join()` använder OS-trådar. ARM64/macOS använder
`libdispatch`. ARM64/Linux och x86-64/Linux använder en långlivad POSIX-trådpool
med futex-väckning. För x86-64/Windows genererar kompilatorn en trådpool med
`CreateThread`, `WaitOnAddress` och `WakeByAddressSingle`. Windowskoden
korsbyggs i testsviten. Nåbara taskoperationer på wasm-målen (`wasm32`,
`wasm32-wasi`) och med `--runtime-profile` avvisas före kodemission.

```dunder
fn sum(values: []i64) -> i64 {
    mut total: i64 = 0
    for value in &values { total = total + value }
    total
}

fn main() {
    task scope jobs {
        first: Task<i64> = jobs.spawn(sum, [1, 2, 3])
        second := jobs.spawn(sum, [10, 20])
        print(first.join())
        print(second.join())
    }
}
```

Programmet skriver `6` och `30`. Om kapaciteten utelämnas i `task scope jobs { ... }`
(eller `task scope jobs(shared cfg) { ... }`) används standardgränsen `4` samtidiga
arbetare; med explicit parameter anges ett positivt `i32`-värde `task scope jobs(N)`.
När gränsen är nådd samlar nästa `.spawn(...)` in det äldsta aktiva jobbet innan
nästa jobb lämnas till poolen. Ett fristående uttryckssatsanrop
`jobs.spawn(worker, arg)` (eller `jobs.spawn(worker)`) där `worker` returnerar
`()` körs automatiskt som detached utan resultat­handtag och återanvänder slutna
kontrollposter i O(kapacitet) minne.
`jobs.wait_any(&tasks)` tar en `[]Task<T>` (eller lån därav) och returnerar
indexet för första färdiga jobbet, eller `-1` för en tom lista.

### Kapabilitetsstyrda I/O-scopes och konfigurerbar arenastorlek

Rena beräkningsscopes (`task scope jobs`) förbjuder I/O och osäkra anrop.
Ett scope kan i stället uttryckligen bevilja delmängder av modulens egna
kapabiliteter (`filesystem`, `network`, `process`, `stdio`) och valfri
arenastorlek per arbetare med:

```dunder
capability network, filesystem

fn worker() -> i32 { 42 }

fn main() {
    task scope workers(4) with (network, filesystem) arena 8 * 1024 * 1024 {
        task := workers.spawn(worker)
        print(task.join())
    }
}
```

Granskade bindningar i `std.net`, `std.fs`, `std.crypto`, `std.time`,
`std.sync` och paketet `sqlite` är märkta `@thread_safe` så de får anropas från
arbetare med motsvarande beviljade kapabilitet, medan godtycklig C-FFI fortfarande
avvisas utan `unsafe`. På Linux återanvänder scopet långlivade arbetartrådar
inom scopets livstid. For koordinering i minnet utan delad muterbar heap erbjuder
[`std.sync`](standard.md#moduler) begränsade kanaler (`Channel<T>` med `Sender`/`Receiver`
och `select2`/`select2_cancel`), uppdaterbara frysta ögonblicksbilder (`Published<T>`
och `ReadGuard<T>`), `Semaphore`, `CancelToken` (kopplad till [`std.signal`](standard.md#moduler))
samt skalära `AtomicI64` och `AtomicBool`.

Resultatvärden sparas tills scopet avslutas eller tas över en gång av `.join()`.

## Ägare och resultathandtag

Scopet äger jobbens dispatchgrupper, workerarenor och väntande resultat.
Det väntar alltid in alla aktiva jobb och avmappar workerarenorna i strikt
**LIFO-upprullningsordning** — **innan** yttre `defer`-block och yttre
`@resource(dispose)`-metoder körs eller föräldraägda värden släpps.
Detta gäller vanlig blockutgång, `return`, `?`, `break` och `continue`. Ett
resultat som aldrig efterfrågas behöver inte kopieras tillbaka från arbetaren.

`Task<R>` är ett typat, 8 byte stort flyttbart handtag (`move_only`, ej `Clone`).
Det kräver ingen egen heapallokering, kan lagras i arrayer (`[]Task<R>`) och `Option<Task<R>>`
och flyttas in i funktioner som tar `move Task<R>`, medan `for t in move tasks` flyttar ut elementen ett i taget.
`task.join()` konsumerar handtaget enligt Dunders vanliga ägarskaps- och flyttregler:
ett andra `.join()`, försök att anropa `.join()` via `&Task<R>`, `tasks[i].join()` utan `.remove(i)`
eller `task.clone()` stoppas redan vid kompilering.
Resultatvärdet flyttas till anroparen och överlever scopet precis som andra ägda värden.

Om ett `Task<R>`-handtag lämnas tillbaka ut ur ett redan stängt `task scope` kontrollerar
`.join()` den levande scopelistan i samordnaren och avslutar kontrollerat med status `112`
i stället för att läsa friat minne. Scope-namnet är en lexikal kapabilitet som bara får
användas med `.spawn(...)` och `.wait_any(...)`; det får inte kopieras, lånas eller
lämna sitt block.

Vanliga workerparametrar läser lånade snapshots. `move`-parametrar tar över
sin jobbägda indata. `.spawn` och delade taskdata är uttryckliga gränser för
isolering: runtime kan behöva kopiera data mellan separata workerarenor.
Dessa isoleringskopior är skilda från vanliga funktionsanrop, som lånar.

## Återanvändbar arbetsyta

```dunder
struct Workspace { values: []i32 }
fn initialize() -> Workspace { Workspace { values: [] } }

fn work(n: i32, ws: &mut Workspace) -> i32 {
    ws.values.clear()
    for i in 0..n { ws.values.push(i) }
    ws.values.len
}

fn main() {
    task scope jobs(2, initialize) {
        a := jobs.spawn(work, 20)
        b := jobs.spawn(work, 30)
        print(a.join() + b.join())
    }
}
```

Det andra argumentet till scopet är en namngiven säker funktion utan
parametrar som returnerar ett ägt record. Runtimen anropar den en gång per
skapad workerarena. När ett jobb i samma scope får en ledig arena får det
också exklusiv tillgång till dess arbetsyta genom sin andra parameter.
Fältens värden och arraykapacitet finns kvar när nästa jobb börjar. Vilken
arena ett jobb får beror på kapacitet och insamlingsordning. Beräkningar som
ska ge samma svar vid olika workerantal behöver återställa sitt lokala
analystillstånd inför jobbet.

Arbetsytan skapas och stannar i workerarenan. Kompilatorn kontrollerar
initialiserarens transitiva effekter och alla lagrade typer, även i opaque
records. Tal, lokala strängar, enums (inklusive payload-enums som `Result`),
tillåtna arrayer, recordfält samt `@resource(dispose)`-fält som ägs per
arbetare (exempelvis en egen `sqlite.Database`-koppling) stöds — de sistnämnda
får sin `dispose()` körd på ägartråden när scopet stängs. Däremot avvisas
adresser, lokala lån, callbacks och taskhandtag. Separata scope äger separata arbetsytor.

## Gemensam läsbar indata

`task scope jobs(limit, shared world)` (eller `task scope jobs(shared world)` med
standardkapacitet `4`) tar en egen isolerad snapshot av `world`
när scopet öppnas. Värdet måste uppfylla **`Sendable + Sync`** (exempelvis en
transparent post eller en `opaque struct` med verifierad `impl Sendable`/`impl Sync`,
som `Map<string, []string>`). Samma krav **`T: Sendable + Sync`** gäller
`std.sync.Published<T>` så att icke-trådsäkra resurser som `fs.File`, `TcpStream`
eller `sqlite.Database` aldrig kan delas via samtidiga `&T`-lån.
Varje worker får samma snapshot som sista parametern `world: &World`. För ett
scope med arbetsyta skrivs `task scope jobs(limit, initialize, shared world)`
och parametrarna blir `(input: Job, workspace: &mut Workspace, world: &World)`.

Föräldern kan ändra sitt ursprungliga värde efter öppningen. Den ägda
snapshoten djupklonas isolerat och samtliga nåbara referensräknare i `[ptr - 8]`
fryses innan någon arbetare observerar `&T`:

| RC-värde (`[ptr - 8]`) | Betydelse | `retain` | `release` |
| --- | --- | --- | --- |
| `rc > 0` | Vanlig icke-atomär per-tråd-RC | `rc += 1` | `rc -= 1`, frigör vid `0` |
| `rc == 0` | Statisk/odödlig literal | no-op | no-op |
| `rc == -1` | Frigjord-objekt-fälla (UAF/dubbel-free-skydd) | fälla | fälla |
| `rc == -2` | Fryst oföränderlig sträng | `string_isolated_clone` till anroparens lokala arena | no-op |
| `rc <= -3` | Fryst sammansatt nyttolast (`desc_id = (-rc) - 3`) | gränskontrollerat uppslag i skrivskyddad `.rodata`-tabell `TYPE_DESC_TABLE[desc_id]` (status `112` vid ogiltigt index) | no-op |

**`Sync` + fryst-RC-förutsättning:** `string`, `[]T`, `Map` och `Set` är `Sync`
*enbart* därför att `Published<T>` och `task scope (shared ...)` fryser nåbara
RC-huvuden (`rc <= -2`) innan andra trådar observerar `&T`. Varje `unsafe`-primitiv
som exponerar `&T` över trådgränser för `T: Sync` måste antingen frysa `T` först
eller förhindra icke-atomär RC-kloning. När scopet stängs inväntas alla jobb och
alla workerarenor avmappas innan snapshoten frigörs.

## Vad en worker får använda

Spawn kräver en namngiven säker Dunder-funktion (inklusive specialiserade
generiska funktioner som `jobs.spawn(work<string>, arg)`). Utöver eventuell
`workspace: &mut Workspace` och `world: &World` från scopet tar en worker och
därmed `.spawn` **högst ett** säkert (`Sendable`) värdeargument (en vanlig
läsparameter `T` eller en ägande `move T`). När en uppgift behöver flera
indata samlas de i en `Sendable`-struct (eller en `Sendable`-tupel):

```dunder
struct JobInput {
    id: i32,
    label: string,
}

fn process_job(input: JobInput) -> string {
    f"{input.id}:{input.label}"
}

fn example() {
    task scope jobs {
        job := jobs.spawn(process_job, JobInput { id: 1, label: "dunder" })
        print(job.join())
    }
}
```

Indata och returtyp måste uppfylla den inbyggda kompilatorbevisade traiten
[`Sendable`](syntax.md#traits-och-impl-trait-for-type) (ägarskapsöverföring mellan
trådar), medan delade referenser över trådgränser (`Published<T>` och
`task scope (shared ...)`) dessutom kräver [`Sync`](syntax.md#traits-och-impl-trait-for-type):

- Primitiva tal, `bool`, `rune`, `()`, `string` och enums utan payload är både `Sendable` och `Sync`.
- Payload-enums (inklusive `Result<T, E>`), ägda/fasta arrayer (`[]T`, `[T; N]`), transparenta structar och tupler ärver `Sendable` respektive `Sync` strukturellt från sina delar.
- **Verifierad säker `impl Sendable / Sync for Typ {}`**: tillåts i säkra moduler (utan `capability unsafe`) för icke-`@resource` `opaque struct`-typer (t.ex. `Map<K, V>`, `Set<K>`, `MethodIndex`, `toml.Tree`, `alloc.Pool<T>`) och generiska omslag (`Reverse<T>`); kompilatorn kontrollerar strukturellt varje privat fält och avvisar `@resource` och råpekare (`ptr`).
- **`unsafe impl Sendable / Sync for Typ {}`** (kräver `capability unsafe`): krävs för okontrollerade åsidosättningar (`@resource`, råpekare eller interiör muterbarhet). Flyttbara handtagsresurser som `TcpStream`, `TcpListener`, `UdpSocket`, `fs.File` och `sqlite.Database` har `unsafe impl Sendable` med strikt `move`-överföring (sändarens ägarslot nollställs) men är **inte** `Sync`, medan trådsäkra synkroniseringshandtag som `sync.Sender<T>`, `sync.Receiver<T>`, `sync.Published<T>`, `sync.Semaphore`, `sync.CancelToken`, `sync.AtomicI64`, `sync.AtomicBool` och `net.Waker` deklarerar både `unsafe impl Sendable` och `unsafe impl Sync`.

Lån, råpekare (`ptr`), funktionsvärden/closures, scratch-vyer, `Task<R>`
och oklassificerade `@resource`- eller `opaque struct`-typer är varken `Sendable`
eller `Sync` och avvisas statiskt. Strängar, payload-enums och sammansatta arrayer går genom dedikerad
isolerad djupkloning så att varje tråd får helt egna allokeringar med egen
referensräkning i sin arena utan delade icke-atomära räknare.
Samtliga tvärtrådsprimitiver i `std.sync` (`AtomicI64`, `AtomicBool`, `Semaphore`,
`CancelToken`, `Channel`, `Published`) och `std.net.Waker` allokerar sitt delade
tillstånd direkt från operativsystemets sidor (`mmap`/`munmap` på macOS/Linux,
`VirtualAlloc`/`VirtualFree` på `x86_64-windows`) — aldrig från en trådlokal
minnespool.

Arbetaren får använda lokala lån, strängar, scratch och allokering i sin
egen runtime samt de kapabiliteter som uttryckligen beviljats via scopets
`with (...)`-klausul. I övrigt avvisar den transitiva effektanalysen unsafe, FFI,
muterbar global lagring, callbacks, obeviljad I/O/argv,
okända indirekta anrop och nästlad task-scheduling. Även hjälpfunktionernas
anrop och semantiskt kontrollerad död kod ingår. Enbart en funktionssignatur
bevisar inte att en worker är tillåten.

Vid `.spawn` klonas (eller, för flyttbara `Sendable`-`@resource`, flyttas med
`move`) den typgodkända indatan till en exklusiv workerarena.
Runtimen lämnar över arenans allokerartillstånd och ägarkopian till
ett jobb. På macOS används libdispatch och en dispatchgrupp per jobb. På Linux
och Windows återanvänder scopet arbetstrådarna. Skalärer skickas som värdebitar. Efter avslutat jobb
kopieras det efterfrågade resultatet till förälderns arena och `.join()` tar över ägarskapet.
Som standard har en arbetare 16 MiB reserverat virtuellt arenautrymme (minst
256 KiB, avrundat till 16 KiB; kan anpassas med `arena <storlek>`) och en
stack på 8 MiB, åtskilda av ett skyddat adressintervall på 16 MiB. Scopet
avmappar reservationen vid utgång. Ett scope utan arbetsyta behåller högst
64 lediga arenor. Med arbetsyta begränsar scopets kapacitet antalet arenor.

## Fel och kompilering

Felaktig kapacitet, stängt eller felaktigt scope vid `.join()`, ogiltigt
`TYPE_DESC_TABLE`-index samt fel vid skapande, inväntan eller avmappning
avslutar processen med status 112. En misslyckad inväntan bevisar inte att arbetaren
har avslutats; felvägen får därför inte läsa eller frigöra dess lagring.
Vanliga runtimefällor och `panic()` i en worker följer Dunder 1.0:s abort-only-kontrakt
(processavslut utan stackupprullning eller `defer`/`dispose`);
återhämtningsbara fel returneras via `Result<T, E>` och kooperativ avbrytning sker med `std.sync.CancelToken` och `std.signal`.

Typkontrollen registrerar direkta effekter, anrop och spawnplatser under
den vanliga kroppsvandringen. Både build och check certifierar grafen när
även onåbara kroppar har kontrollerats. Ett bygge binder taskruntimens
symboler efter godkänd certifiering, före publicering. Funktionscachen
stöder även taskprogram via innehållsadresserade fragment.

### Taskruntimens minneslayout

Varje offset nedan räknas i byte från postens början. Föräldern äger
samordnaren och indexet. Varje aktivt jobb har ensam tillgång till sin
allokeringsarena. Scopet inväntar aktiva jobb före frigöring av resultaten.
[Scopets emitter](../selfhost/tasks/task_scope_emit.ddr) · [Nativa jobb](../selfhost/tasks/task_native.ddr)

| Post | Storlek | Fält vid byteoffset |
| --- | ---: | --- |
| Samordnare | 32 | 0 initierad (`-1` för ext-samordnare), 8 ägartråd, 16 senast utfärdad generation, 24 huvud för lista av levande scope |
| Scope | 192 | 0 generation, 8 tillstånd (1 öppet, 2 stängt), 16 gräns, 24 aktiva jobb, 32 utfärdade jobb, 40 indexkapacitet, 48 indexpekare, 56/64 köhuvud/-slut, 72 ägartråd, 80/88 ledig lista/antal, 96 arbetsytans initierare, 104 delat värde, 112 dess städfunktion, samt utökade fält vid 120–184 (arena-storlek per arbetare, ledig lista för detached-jobb, semafor/väntcell för worker-lane, 152 nästa levande scope, 160 samordnarpekare) |
| Indexpost | 16 | 0 lånad jobbpekare, 8 exakt resultattyp |
| Jobbpost | 192 | 0 jobbtyp, 8 nästa jobb i kön, 16 ordningsnummer (−1 för detached), 24 scope, 32 aktiv, 40 nästa lediga post, 48 resultatstatus (4 cachad, 10 övertagen av `.join()`), 64 nativ kontrollpost, samt utökade fält vid 96–184 (återvunnen arena, arbetsyta, delad plats, arena-storlek, lane) |
| Jobbtyp | 64 | 0 initierad, 8 resultattyp, 16 starta, 24 hämta, 32 kassera, 40 klona resultat, 48 städa resultat, 56 reserverat |
| Nativ kontrollpost | 144 | 0 tillstånd, 8 dispatchgrupp eller `pthread_t`, 16 indata, 24 resultat, 32 arenabas, 40 förälderns resultat, 48 allokeringsposition, 56 väntfel, 64 avmappningsfel, 72 återvunnen arena, 80 arbetsytans initierare, 88 arbetsyta, 96 lån av scopets delade plats, samt utökade fält vid 104–136 (worker-pool-lane) |

Kontrollpostens tillstånd är 0 klar för start, 1 inlämnad, 4 cachad, 5 startfel,
6 väntfel, 8 kasserad och 9 avmappningsfel. (Resultatstatus 10, övertagen av
`.join()`, finns på jobbpostens fält vid byteoffset 48, inte på kontrollposten.)
Vid väntfel kan arbetaren fortfarande köra. Kontrollpost, trådresurser och
arena måste då förbli giltiga.

## Kompilatorn som taskkonsument

Den taskbaserade kompilatorn är standardkompilator och självhostad på alla tre
nativa värdar (ARM64/macOS, ARM64/Linux och x86-64/Linux). Utan `--jobs` väljs
arbetsbudgeten från värddatorns tillgängliga CPU-kapacitet via `budget.ddr`;
`--jobs N` eller `--jobs=N` väljer ett explicit tak på 1–256. Värddatorns
kompilatorjobb kör på trådar även vid korskompilering till andra mål eller Wasm.

Byggsamordnaren äger symboler, nåbarhetsordning och imports. Vid rena och
delvis cachade byggen kan den skicka tillgängliga funktioner till arbetarna
medan tidigare funktionsobjekt sänks. Typning och MIR delar samma budgetpool;
samordnaren räknas in i budgeten när den själv kompilerar kroppar, medan ren
samordning vid dränering inte reserverar en plats. `check` parallelliserar
semantisk kontroll utan kodemission, och `--jobs 1` samt `--profile-compiler`
kör samma gemensamma jobbmodell i enkelspår utan arbetartrådar. Varje källjobb
får bara sin funktions token. Deklarerade typer och signaturer delas genom en fryst bild och varje worker återanvänder sin egen session. Samordnaren tar emot typade resultat i samma ordning som den seriella genomgången och binder deras anrop, effekter och symbolbehov.
`--timings=json` redovisar hur många skalära och generella kroppar som
arbetarna typade och hur många frontend-arbetsytor som användes.
`frontend.windows` räknar grupper av källjobb. `frontend.peak_total_jobs`
räknar det största sammanlagda antalet utestående käll- och backendjobb.
[Frontendarbetare](../selfhost/tasks/frontend_tasks.ddr)

Modulladdaren kan lexa flera redan upptäckta moduler parallellt. Arbetarna
returnerar källspann och avkodade stavningar för literaler. Samordnaren tar
emot resultaten i modulordning, löser deklarationer och importer och bevarar
ordningen på diagnostiken.

När nåbarheten är känd uppskattar samordnaren återstående typkontroll från
deklarationernas redan kända tokenspann och antalet kroppar. Med en worker
eller liten återstående arbetsmängd kontrolleras dessa kroppar direkt i
samordnarens session. Vid tillräckligt oberoende arbete delar arbetarna
en fryst deklarationsbild och läser varsin funktionskropp. Båda fallen
använder samma typkontroll och rapporterar fel i deklarationsordning.
`compile_parallel` skickar funktionslokala operationer och metadata i separata
kolumner i `mir_task.Batch`. Små startbatcher fyller lediga arbetsplatser.
Därefter avgör både antalet funktioner och mängden maskinord när en batch skickas.
Vid en tung backendkö använder samordnaren de platser som redan är lediga för
nya typjobb och kan själv fortsätta typa kroppar medan MIR-jobben kör. När
flera kroppar i följd har sänkts från den typade skalärmodellen reserverar
samordnaren platser för typning även med en tung backendkö. Varje
arbetstråd bygger MIR lokalt, verifierar, optimerar och sänker funktionerna i
batchen med arbetsytor som återanvänds över batchgränserna.
Passinställningar och terminal-ID:n delas som en snapshot för hela scopet.
Svaret innehåller slutobjekt och statistik i `mir_task.BatchAnswer`.
Föräldern tar över svaren med `.join()` och länkar i källordning. [Batchformat och worker](../selfhost/tasks/mir_task.ddr)

`ObjectData` är en vanlig transparent post med primitiva arenor och går
genom språkets befintliga snapshot-/klonregler. Den ger inget undantag för
opaque-typer eller delat managed-minne. Föräldern kontrollerar version,
ursprungsspann, plattform, lokala ord-/byteoffsetar, gränskarta, etiketter,
relokeringar och import-site-ID:n före ordnad acceptans. Strukturell
validering certifierar inte godtyckliga maskinords semantik: resultatet
måste dessutom höra till exakt det utlämnade jobbet i samma taskscope,
ordinal och deklaration. Föräldern kör ingen andra sänkning; den äger cache,
ordnad länkning och slutlig bildpublicering.

Efter länkning kan kompilatorn också hasha kodsidorna i större Mach-O-bilder
parallellt. Hashningen använder samma `--jobs`-budget och skriver digesterna
i sidordning. Det gäller både fristående binärer och `.app`-paket.

Cacheträffar lämnar redan färdiga objekt utan ny MIR-verifiering eller
optimering. Cacheformat och objektstruktur valideras fortfarande. Beroenden och budgetdebitering återspelas i samma ordning
som vid ett kallt bygge. Tidigare jobb tas emot före senare källfel, så
färdigordningen ändrar inte första diagnostiken. Workerantalet ingår inte
i cacheidentiteten och ändrar inga optimeringsbudgetar.

Kompilatorns funktionscache fungerar också med parallell MIR-bearbetning;
begränsningen ovan gäller källprogram som själva innehåller tasksyntax.

Fulluttryckets exakt identifierade, unikt ägda recordtemporär kan överföras
utan en extra djupkopia inom samma runtime. Den generiska normaliseringen
bevarar nästlade arrayers synliga kopieringskapacitet; strängar, delade
enums och inline C-layout är stoppunkter. Lokaler och fältprojektioner
är skild från språkets allmänna ägarflyttar. Isoleringskopiorna vid
spawn/join behövs fortfarande mellan workerarenorna.

[Mätning av taskprogram](../benchmarks/tasks/README.md) · [Kompilatormätningar](../benchmarks/results.md)
