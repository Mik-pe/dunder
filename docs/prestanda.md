# Så mäter vi Dunder

Den senaste [parallella kompilatormätningen](../benchmarks/2026-11-12-parallel-compiler/README.md)
har 30 försök per variant på ARM64/macOS. Den skiljer två jämförelser:
samma taskbaserade kompilator med en respektive fyra workers, och tidigare
seriell kompilator mot den nya standarden med fyra workers. Tabellen visar
medianer i millisekunder; den första jämförelsen isolerar workerantalets effekt.

| Arbetsfall | Samma binär: 1→4 workers | Tidigare seriell → ny standard med 4 |
| --- | --- | --- |
| Genererat bygge | 953,718 → 609,106 | 953,519 → 615,609 |
| Självbygge | 874,940 → 709,677 | 862,428 → 712,768 |
| Tabell | 20,284 → 18,017 | 20,069 → 18,048 |

Den nya standardkandidatens genererade p95 är **619,768 ms**, motsvarande
**163 003 källrader/s**. Det klarar 100 000-gränsen för det uppmätta
genererade korpuset. Frontend, sänkning, ordnad sammanfogning och publicering
ingår; enbart MIR-verifiering och optimering körs parallellt. `check` och
kompilatorprofilering använder fortsatt den delade seriella vägen.

Runtimebilderna är byteidentiska. Den aktuella jämförelsen mot C ger
geometriskt medel **1,178×** och största kvoten **1,308×**, över målen
1,10× och 1,25×. Snabbare kompilering innebär därför inte att hela
acceptanskontraktet nedan är uppfyllt. Tidigare
[taskmätningar](../benchmarks/2026-11-12-isolated-tasks/README.md) redovisar
språkets isolerade arbetsfall och den dåvarande seriella byggkostnaden
separat från denna kompilatormätning.

Den lokala [runtime-sviten](../benchmarks/runtime/README.md) jämför nu
baseline och optimerad maskinkod med fyra frysta arbetsfall, oberoende
resultatreferenser och 30 växelvisa körningar. Den är ett avgränsat
före/efter-prov. Fyra arbetsfall har också jämförts med clang `-O3` och
`-O3 -ftrapv` på ARM64/macOS. Acceptansgränserna nedan återstår.

Två egenskaper hör ihop. Den optimerade **standardprofilen** ska både bygga minst 100 000 källrader/s och ge konkurrenskraftig körprestanda. Samma kompilatorrevision, säkerhetsregler och profil ska användas för båda mätningarna. Siffrorna nedan är acceptansgränser. [Byggmätningar och råvärden](../benchmarks/README.md) skiljer genererat stresskorpus, självbygge och verkliga konsumenter åt; hela kontraktet är ännu inte uppfyllt.

## Från kommando till färdig ARM64-binär

Starta klockan utanför processen omedelbart före `dunder build`. Stoppa den när kommandot har avslutats och den färdiga signerade filen ligger på disk. Modulupptäckt, läsning, parsning, typ- och lånekontroll, optimering, kodgenerering, länkning och signering ingår. Kör sedan binären och verifiera ett känt resultat. En felaktig binär underkänner provet.

Referensmaskinen till att börja med är en Apple M5 med 24 GiB minne. Publicera maskinmodell, OS-version, antal byggarbetare, kompilatorrevision, korpusrevision och byggflaggor med varje resultat. Kör från en ny process varje gång. Rapportera en arbetare separat från standardinställningen så att ökad parallellism syns.

| Arbetsfall | Mätning och första mål |
| --- | --- |
| Rent optimerat bygge | Tom Dunder-cache och tom utdatakatalog per försök. OS-filcache får vara varm och märks som sådan. På ett korpus med minst 100 000 källrader ska även p95-byggtiden ge minst 100 000 rader/s. Ett exakt 100 000-raderskorpus har alltså en budget på 1,0 s. |
| Första körningen efter omstart | Rapportera separat. Här syns även fysisk inläsning. |
| Bygge utan ändring | Mät latens i millisekunder från en ny process. Föreslaget interaktivt mål är p95 ≤ 50 ms på referenskorpuset. |
| En privat funktionskropp ändras | Mät hela vägen till ny körbar fil, inklusive omlänkning och signering. Föreslaget mål är p95 ≤ 100 ms på samma korpus. |
| Delad typ-layout ändras | Redovisa tid och antal ombyggda funktioner. Jämför byte för byte med resultatet av en ren byggning från det ändrade trädet. |

Räkna fysiska källrader med minst en språktoken i de moduler som faktiskt ingår. Blankrader och rena kommentarer bidrar aldrig till täljaren. Varje fil räknas en gång. Publicera även antal byte, token och funktioner, slutlig binärstorlek, toppminne samt tid per fas. För inkrementella byggen är millisekunder och faktiskt ombyggda funktioner mer meningsfulla än rader/s.

Lås en korpus med importer, externa och interna anrop, heltalskod, slices, poster, villkor, loopar, temporärer och lån. Låt en körbar testdrivrutin anropa kod från alla delar och kontrollera resultatet. Märk genererad stresskod separat och särredovisa ett bygge av den självhostade kompilatorn så snart den finns. En tokenizer eller 100 000 identiska deklarationer kan hjälpa vid felsökning men kvalificerar inte som bevis för helkompilatorn.

Kör minst 30 försök för vanliga arbetsfall. Publicera råvärden, median och p95 samt arbetsordningen. Återställ ändringar, cache och utdata utifrån arbetsfallet inför varje försök. Använd innehållsändringar som påverkar resultatet. Byggtider med enbart varm Dunder-cache underkänner ett anspråk om ren byggprestanda.

## Vad det innebär att mäta sig med C och C++

Föreslagen första gräns är ett geometriskt medel på högst **1,10×** referensens körtid och inget enskilt arbetsfall över **1,25×**. En faktor på 1,10 betyder tio procent längre körtid för Dunder. Gränsen gäller den publicerade sviten. Den gör en bred ambition möjlig att pröva utan att en snabb innerloop får representera alla program.

Börja med summering över en slice, en vektoriserbar numerisk loop, parser över riktig UTF-8-källkod, sortering av poster och trädbyggande i en arena. Sviten ska fånga såväl registertryck och grenar som allokering och minnesbandbredd. Lägg till ett verkligt program, exempelvis Dunders lexer över den egna källan, med en motsvarande C- eller C++-implementation. Lås indata och referenskod innan resultaten används som acceptansbevis.

Bygg referenserna med optimerande C/C++-kompilator, exempelvis Apple Clang med `-O3`. Det är ett mätverktyg för referensprogrammen. Matcha mål-CPU, tillåtna instruktioner, algoritm, layout, allokeringsstrategi och numeriska regler. Dunders ordinarie säkerhetskontroller är på. Redovisa eventuella explicita kontroller i referensprogrammen och visa separat jämförelse om en sådan kontroll ändrar resultatet märkbart. Begränsa tillåtna indata så att C/C++ har definierat beteende i samtliga körningar.

Verifiera identiskt resultat på samma indata först. Använd körningsindata och observerbara resultat så att ingen kompilator kan räkna ut eller kasta bort arbetet vid byggtid. Varva ordningen mellan programmen, använd minst 30 observationer per variant och redovisa spridning och kvot för varje arbetsfall. Vid ett missat mål, studera först maskinkoden och tid per fas. Mät om hela sviten efter ändringen så att vinster i körtid kan vägas mot kostnaden i byggtid.
