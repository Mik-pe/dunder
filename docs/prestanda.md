# Så mäter vi Dunder

Den aktuella [regionala IR-/workermätningen](../benchmarks/2026-11-12-regional-workers/README.md)
bevarar 1 260 prover: tidigare mot ny standard med fyra workers samt samma
nya binär med en, två och fyra workers. SSA är av i samtliga jämförelser.

| Arbetsfall | 1 worker, median/p95 ms | 4 workers, median/p95 ms |
| --- | ---: | ---: |
| Genererat bygge | 982,428 / 987,013 | 623,820 / 625,539 |
| `selfhost/main.ddr` | 1 023,754 / 1 025,300 | 823,461 / 826,993 |
| `selfhost/main_parallel.ddr` | 1 186,908 / 1 189,227 | 971,125 / 975,632 |
| Tabell | 20,488 / 21,590 | 18,153 / 19,042 |
| Hello | 5,343 / 5,875 | 5,390 / 5,656 |
| Owned | 5,970 / 6,796 | 5,743 / 8,545 |

Fyra workers ger 1,57×, 1,24× och 1,22× i de tre större fallen.
Det senaste paketet är samtidigt 0,64–1,56 procent långsammare i byggmedian
än tidigare standard med fyra workers. Dess minnestopp minskar i större
fall. Alla råsvansar, inklusive owned-fallets sämre p95, finns kvar.
Genererat bygge når 161 499 rader/s vid p95 med fyra workers; alla tre
workerantal klarar 100 000-gränsen. Separata `check`-prover är seriella
kontroller. Kontrollerade runtimebilder är byteidentiska, så ingen ny
C-runtimevinst påstås. Frontenden är fortsatt seriell och C-målet öppet.

Den tidigare [glesa SSA-v2-mätningen](../benchmarks/2026-11-12-ssa-pruned/README.md)
har 1 440 prover över tre separata jämförelser på samma frysta källor.
Med SSA på i båda kompilatorerna minskar byggmedianen 27,65 procent för
kompilatorkorpuset och 8,48 procent för Tabell mot tidigare SSA; Tabells
p95 ökar samtidigt 20,41 procent. Samma nya kompilator med SSA av/på ger:

| Arbetsfall | SSA av, median/p95 ms | SSA på, median/p95 ms | Medianförändring |
| --- | ---: | ---: | ---: |
| Genererat bygge | 619,219 / 620,729 | 624,279 / 631,928 | +0,82 % |
| Kompilatorkorpuset | 785,522 / 788,351 | 952,521 / 954,722 | +21,26 % |
| Tabell | 18,054 / 18,621 | 23,375 / 25,484 | +29,47 % |

SSA på klarar 100 000-gränsen för genererat bygge vid p95 med 159 866
rader/s. Runtime/C-geomedlet är 1,1841 på mot 1,1883 av; text och sort
med passet på ligger på 1,3376 respektive 1,2649. C-gränserna är alltså
öppna. SSA förblir av som standard. En separat av/av-serie ger 0,28–0,69
procent högre byggmedian med byteidentiska konsumenter; p95-regressioner
och alla råsvansar finns kvar i rapporten. Kompilatorkorpuset avser den
seriella ingången `selfhost/main.ddr`, inte hela bootstrapkedjan.

Den historiska [SSA-v1-mätningen](../benchmarks/2026-11-12-ssa-foundation/README.md)
har 30 försök per variant på ARM64/macOS. Samma kompilator med SSA av/på
gav följande byggmedianer i millisekunder:

| Arbetsfall | SSA av | SSA på | Förändring |
| --- | ---: | ---: | ---: |
| Genererat bygge | 606,871 | 613,026 | +1,01 % |
| Kompilatorkorpuset | 762,977 | 1 304,080 | +70,92 % |
| Tabell | 17,564 | 25,001 | +42,34 % |

Alla fyra runtimebilder ändrades, men runtime/C-geomedlet blev 1,181 mot
1,178 med SSA av. En separat SSA-optimerad kompilatorbinär, med samma
användarpolicy SSA av, ändrade byggmedianerna mindre än 0,3 procent och gav
sämre p95 i flera fall. Varken standardprofilen eller bootstrapens
producentprofil ändras på detta underlag. SSA förblir valbart med
`--mir-ssa=on`; C-målet är öppet. Kompilatorkorpuset är fortsatt den seriella
ingången `selfhost/main.ddr`, inte hela bootstrapkedjan.

Den tidigare [parallella kompilatormätningen](../benchmarks/2026-11-12-parallel-compiler/README.md)
har 30 försök per variant på ARM64/macOS. Den skiljer två jämförelser:
samma taskbaserade kompilator med en respektive fyra workers, och tidigare
seriell kompilator mot den nya standarden med fyra workers. Tabellen visar
medianer i millisekunder; den första jämförelsen isolerar workerantalets effekt.

| Arbetsfall | Samma binär: 1→4 workers | Tidigare seriell → ny standard med 4 |
| --- | --- | --- |
| Genererat bygge | 953,718 → 609,106 | 953,519 → 615,609 |
| Självbygge | 874,940 → 709,677 | 862,428 → 712,768 |
| Tabell | 20,284 → 18,017 | 20,069 → 18,048 |

Självbygget avser det fasta kompilatorkorpuset `selfhost/main.ddr`, som
skriver den seriella kompilatoringången. Tiden för `main_parallel.ddr`
och hela frö-/fixpunktkedjan i `bootstrap.sh` ingår inte i dessa värden.

Den dåvarande standardkandidatens genererade p95 var **619,768 ms**, motsvarande
**163 003 källrader/s**. Det klarar 100 000-gränsen för det uppmätta
genererade korpuset. Frontend, sänkning, ordnad sammanfogning och publicering
ingår; enbart MIR-verifiering och optimering körs parallellt. `check` och
kompilatorprofilering använder fortsatt den delade seriella vägen.

Runtimebilderna var byteidentiska i den jämförelsen. Den gav
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
