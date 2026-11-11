# Så mäter vi Dunder

Den aktuella självhosten har en [gemensam runtime- och byggmätning](../benchmarks/2026-11-11-whole-function-optimization/README.md#final-serial-profile-with-detached-function-objects)
med 30 försök per variant, råtider, kontrollerade resultat och full lokal
verifiering. Standardprofilens genererade bygge når 107 909 källrader/s vid
p95 med en byggarbetare. Runtime/C-geomedlet är 1,168× och textfallet 1,325×;
C-målet återstår. Byggmedianen kostar fortfarande 2,89× på det genererade
korpuset och 3,13× på självbygget jämfört med kompilatorn före helfunktions-IR.
Kontroll utan kodgenerering har jämförbar tid. Hela acceptanskontraktet nedan
är ännu inte uppfyllt.

Den lokala [runtime-sviten](../benchmarks/runtime/README.md) jämför nu
baseline och optimerad maskinkod med fyra frysta arbetsfall, oberoende
resultatreferenser och 30 växelvisa körningar. Den är ett avgränsat
före/efter-prov. Fyra arbetsfall har också jämförts med clang `-O3` och
`-O3 -ftrapv` på ARM64/macOS. Acceptansgränserna nedan återstår.

Två egenskaper hör ihop. Den optimerade **standardprofilen** ska både bygga minst 100 000 källrader/s och ge konkurrenskraftig körprestanda. Samma kompilatorrevision, säkerhetsregler och profil ska användas för båda mätningarna. Alla siffror nedan är föreslagna acceptansgränser. Vi har nu [byggmätningar och råvärden](../benchmarks/README.md) för ett genererat stresskorpus. Det är första mätunderlaget; hela kontraktet nedan är ännu inte uppfyllt.

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

Lås en korpus med importer, externa och interna anrop, heltalskod, slices, poster, villkor, loopar, flyttar och lån. Låt en körbar testdrivrutin anropa kod från alla delar och kontrollera resultatet. Märk genererad stresskod separat och särredovisa ett bygge av den självhostade kompilatorn så snart den finns. En tokenizer eller 100 000 identiska deklarationer kan hjälpa vid felsökning men kvalificerar inte som bevis för helkompilatorn.

Kör minst 30 försök för vanliga arbetsfall. Publicera råvärden, median och p95 samt arbetsordningen. Återställ ändringar, cache och utdata utifrån arbetsfallet inför varje försök. Använd innehållsändringar som påverkar resultatet. Byggtider med enbart varm Dunder-cache underkänner ett anspråk om ren byggprestanda.

## Vad det innebär att mäta sig med C och C++

Föreslagen första gräns är ett geometriskt medel på högst **1,10×** referensens körtid och inget enskilt arbetsfall över **1,25×**. En faktor på 1,10 betyder tio procent längre körtid för Dunder. Gränsen gäller den publicerade sviten. Den gör en bred ambition möjlig att pröva utan att en snabb innerloop får representera alla program.

Börja med summering över en slice, en vektoriserbar numerisk loop, parser över riktig UTF-8-källkod, sortering av poster och trädbyggande i en arena. Sviten ska fånga såväl registertryck och grenar som allokering och minnesbandbredd. Lägg till ett verkligt program, exempelvis Dunders lexer över den egna källan, med en motsvarande C- eller C++-implementation. Lås indata och referenskod innan resultaten används som acceptansbevis.

Bygg referenserna med optimerande C/C++-kompilator, exempelvis Apple Clang med `-O3`. Det är ett mätverktyg för referensprogrammen. Matcha mål-CPU, tillåtna instruktioner, algoritm, layout, allokeringsstrategi och numeriska regler. Dunders ordinarie säkerhetskontroller är på. Redovisa eventuella explicita kontroller i referensprogrammen och visa separat jämförelse om en sådan kontroll ändrar resultatet märkbart. Begränsa tillåtna indata så att C/C++ har definierat beteende i samtliga körningar.

Verifiera identiskt resultat på samma indata först. Använd körningsindata och observerbara resultat så att ingen kompilator kan räkna ut eller kasta bort arbetet vid byggtid. Varva ordningen mellan programmen, använd minst 30 observationer per variant och redovisa spridning och kvot för varje arbetsfall. Vid ett missat mål, studera först maskinkoden och tid per fas. Mät om hela sviten efter ändringen så att vinster i körtid kan vägas mot kostnaden i byggtid.
