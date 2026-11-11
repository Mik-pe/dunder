# Kodgenerering i Dunder

Vanliga byggen går genom [typkontrollen och kodgeneratorn](../selfhost/compile.ddr).
Kompilatorn avger instruktioner, lokala etiketter och relokeringar per funktion.
Den tar med de runtimefunktioner som programmet behöver. Overflow, ogiltiga
index och ogiltiga konverteringar behåller sina kontroller även vid korsbyggen.

## Typad funktionsmodell och lokalt optimeringspass

Standardbygget provar först en kompakt, typad och strukturerad
[funktionsmodell](../selfhost/opt_ir.ddr). Den återanvänder samma arena mellan
funktioner. [Parsern](../selfhost/opt_parse.ddr) godtar i första etappen
anropsfria funktioner med `i32`, `i64`, `bool`, delade `&[]i64`-parametrar
och exklusiva `&mut []i64`-parametrar:
lokaler, tilldelningar, aritmetik, kortslutning, `if`, `while`, retur,
`break`, `continue`, räknade `for`-loopar, arraylängd, indexläsning och
elementskrivning. Högst åtta parametrar och en skalär eller tom retur
stöds. Ägda värden, storleksändring eller byte av arraylagring, flyttal,
osäker kod och anrop använder den befintliga vägen.

Alla grenar kontrolleras innan optimeringen börjar, även om ett villkor
är konstant. Om modellen inte stöder kroppen eller når sin resursgräns
tar den befintliga kompilatorn över, inklusive feldiagnostik. Gränserna
är 4 096 token, 2 048 noder, 256 lokaler och konservativt djup 64.
Detta är en lokal IR per funktion; någon helprograms-IR eller SSA byggs inte.

[Gemensamma språkregler](../selfhost/scalar_rules.ddr) används av både
den fulla parsern och modellparsern för ASCII-namn, reserverade namn,
operatorprioritet och kontextuell bredd på signerade heltalsliteraler.
Breddkontrollen validerar decimalparserns befintliga resultat utan att
skapa en extra resultatpost per literal.
En accepterad modell har kontrollerats av modellparsern; en avvisad modell
går vidare till den fulla parsern för stöd och diagnostik.

[IR-verifieraren](../selfhost/opt_verify.ddr) kontrollerar modellen före
och efter passet: parallella vektorer, index, typer, nodroller,
räckvidd för lokaler, initiering, parameterordning, returer, looputgångar,
trädägarskap och konservativa effekter. Cykler och delade exekverbara
noder avvisas; bortkopplade arbetsnoder efter förenkling är tillåtna.
Ett internt invariantfel ger ett kompilatorfel före emission. Det räknas
inte som en vanlig modellbegränsning och döljs inte genom återfall.
Verifieraren kontrollerar var bevis får användas; själva bevisregelns
sundhet provas separat och följer inte automatiskt av formkontrollen.
Verifierarens flaggor och scopestack återanvänds mellan funktioner och
passfaser i ett separat arbetsminne per kompilatoranrop. Varje anrop
initierar modellens aktuella del även efter ett tidigare verifieringsfel.
Form-, typ- och säkerhetskontrollerna körs fortfarande varje gång.

[Passet](../selfhost/opt_pass.ddr) förenklar konstanter, lokala värden och
konstanta grenar inom en fast arbetsbudget på 65 536 steg. Det räknar
bara ut aritmetik som är giltig med språkets kontrollerade heltalsregler.
Ett överflödande uttryck måste fortfarande nå samma runtime-fälla.
Kortslutning och den observerbara ordningen mellan möjliga fel bevaras.

För en loop med `i = 0`, villkoret `i < values.len` och en enda avslutande
ökning `i = i + 1` kan passet bevisa indexläsningar före ökningen och
själva ökningen säkra. Arrayen måste vara en oföränderlig inparameter.
Andra skrivningar till indexet, `continue` eller nästlade loopar hindrar
beviset. Summeringens overflow-kontroll finns kvar. Regeln bygger på
kroppens struktur och gäller oberoende av funktionsnamn eller modul.

En räknad `for i in low..high` har en egen strukturerad nod. Båda
gränserna är `i32` och utvärderas en gång, nedre först, innan iteratorn
kommer i scope. Iteratorn är oföränderlig i källkoden och kroppen har ett
inre scope. `continue` når ökningen; `break` når utgången. En dold
övre gräns behålls över hela loopen även om källans lokaler ändras.
En loop garanterar aldrig att funktionen returnerar ett värde.

Elementskrivning är en separat typad nod med uttryckligt minnesskrivningseffekt.
Den utvärderar och gränskontrollerar destinationen före högerledet och
bevarar adressen över högerledets indexläsningar. Den exklusiva referentens
skrivrätt skiljs från själva parameterbindningens oföränderlighet.
Arrayens dataadress och längd är stabila eftersom modellens slutna
operationsmängd inte kan byta descriptor, växa, exponera adresser eller
anropa annan kod. Innehållsvärden cachas inte över skrivningar. Nya
muterbara index och skrivningar behåller sina gränskontroller.
Arrayvärdesiteration använder fortsatt den allmänna vägens snapshot.

[Registerallokeringen](../selfhost/opt_alloc.ddr) använder konservativa
livstidsintervall, förlänger dem över loopars bakåtkanter och placerar
lokaler i tolv register eller i framen. [Emissionen](../selfhost/opt_emit.ddr)
behåller arraydata och längd, avger direkta jämförelsegrenar och tar bara
bort kontroller som modellen har bevisat överflödiga. Inkommande argument
sparas före registerplaceringen så att överlappande argumentregister är
säkra. x86-64 och Wasm sänker fortsatt den delade ARM-instruktionsvokabulären;
detta inför ingen separat direkt backend för dem.

`--scalar-opt=off` stänger av den nya modellen, passet och registervägen
för jämförelse och felsökning. Tidigare lokala kodgenereringsförbättringar
är fortfarande aktiva. `--timings=json` visar antal modellerade och
emitterade funktioner, omskrivningar, bevisade indexläsningar och
parseråterfall. Avvisade signaturer och cacheträffar ingår inte i
återfallsräknaren. Funktionscache och `test --affected` skiljer lägena åt;
CLI-cachen behåller varma fragment separat för båda lägena.

`tools/test-optimizer.py` jämför optimerad kod med avstängt pass och kan
även jämföra med en äldre kompilator genom `--baseline`. Den provar
resultat, fällor, diagnostik, bindningsposter och vilka funktioner som
använder modellen. `tools/test-opt-emitter.py` provar emission och spill
separat. Båda ingår i `tools/verify-selfhost.sh`.

## Nästa etapper mot C-nivå

Målet prövas över hela den publicerade sviten enligt
[prestandakontraktet](prestanda.md). En snabb summeringsloop räcker inte.
Varje etapp behöver jämförelse med avstängt pass, kontrollerade resultat
och 30 försök för runtime, genererat bygge, självbygge och Tabell.
Median, p95 och binärstorlek ska redovisas; en högre p95 får inte döljas
bakom snabbare genererad stresskod.

| Etapp | Mekanism och förväntad nytta | Krav före aktivering |
| --- | --- | --- |
| Bredare lokal modell | Skalära anrop, fler typer och ägda iterationer öppnar fler verkliga funktioner. Räknade loopar och exklusiv elementmutation är implementerade. | Anropseffekter, register som förstörs vid anrop, bevarade adresser och låneursprung måste vara explicita. Samma frontend ska successivt ersätta återstående dubblerad semantik. |
| Starkare heltalsbevis | Intervall, induktionsvariabler och reduktion av division/rest med konstanta delare minskar kostnaden i numerik. | Bevara overflow, nollfall, negativa operander och ordningen mellan möjliga fällor. Arbetsbudget och konservativt återfall ska vara deterministiska. |
| Egna målbackends från modellen | x86-register och Wasm-lokaler kan väljas direkt från de typade värdena. | Samma semantiska kontrakt på alla mål, med faktisk Linuxkörning för Linuxanspråk. Dagens översatta ARM-register är inte fysisk x86-registerallokering. |
| Begränsad specialisering och inlining | Små heta hjälpfunktioner kan slippa anropskostnad och ge lokala bevis över anropsgränsen. | Fast budget per funktion och anropsställe, begränsad kodtillväxt och cacheidentitet för samtliga påverkande sammanfattningar. |
| Vektorer och minnesbandbredd | SIMD, loopuppdelning och sammanhängande åtkomst kan närma sig C i bredare dataloopar. | Bevis för aliasering, varje åtkomst och varje relevant mellanresultat. Kontrollerad summering får inte ersättas med en omordnad reduktion utan sådana bevis. |
| Kortare bygglatens | Återanvänd typade resultat och beroendesammanfattningar, och bearbeta oberoende funktioner parallellt. | Stabil diagnosordning, byteidentisk utdata, en arena per arbetare och mätning av privat kroppsändring respektive delad layoutändring. |

Prioritera nästa mekanism efter profiler och maskinkod för arbetsfallen
som fortfarande missar målet. Mät allokering och kopiering i lexer, parser
och bibliotek separat när de dominerar; fler maskinkodspass löser inte
automatiskt dessa kostnader. Separata valbara fastider för modellbygge,
förenkling, registerallokering och emission blir användbara när den lokala
vägen får större täckning.

## Övrig kodgenerering och runtime

Lokala värden läses direkt från sin frameplats med rätt bredd och
teckenutökning. Deras stabila adress beräknas först när ett lån, en rå
adress eller en tilldelning behöver den. Fält och arrayelement behåller
sin separat sparade adress och reglerna för omvärdering efter mutation.
Skalära tilldelningar till lokaler skriver direkt till frameplatsen.

En ensam skalär högeroperand utan anrop eller postfix låter vänstervärdet
stanna i ett register. Tätare operatorer, anrop och sammansatta uttryck
använder den allmänna stackvägen. Utvärderingsordning, operatorprioritet,
kortslutning och aritmetikkontroller är desamma. Looparnas interna
jämförelser behöver heller inte spilla operanderna till stacken.

Rune-literaler avkodas under kompileringen och emitteras som skalära
konstanter. Även Unicode och escape-sekvenser behöver därför ingen
strängkonvertering eller rune-array under körning. En ensam rune-literal
kan använda samma registerväg som andra skalära konstanter.

Ett helt ägt temporärt uttryck kan överföra sitt värde till en deklaration,
ett argument, en retur, en tilldelning eller en loop. Kompilatorn spårar
uttryckets exakta ägarplats och nollställer den vid överföringen. Befintliga
lokaler och projektioner av fält eller element kopieras fortsatt. Parenteser
behåller ursprunget; nya operator- och anropsresultat får egna ägarplatser.
Arrayer behåller den kapacitet en vanlig värdekopia skulle visa, även om en
större underliggande buffert återanvänds. Sammansatta värden med nästlade
ägda samlingar använder tills vidare den rekursiva kopieringsvägen.

[ARM64-backenden](../selfhost/arm64.ddr) använder direkt LDUR/STUR för
lokala värden nära framepekaren och immediate-adresser där offseten ryms.
[x86-64](../selfhost/x86.ddr) och [Wasm](../selfhost/wasm_native.ddr)
sänker den gemensamma instruktionsströmmen till respektive mål. Wasm använder
bland annat `memory.copy` för kopiering av sammanhängande block.
Native-runtime kopierar buffertar åtta byte åt gången och avslutar med en
byte-loop för resten. Varje ordläsning ryms inom den begärda längden,
även vid ojusterade adresser. Strängars och arrayelements ägarskap behålls.

Lexern klassificerar de fasta nyckelorden utan hashtabell och avkodar
vanliga literaler direkt från giltig UTF-8. Escape-sekvenser använder den
separata avkodningsvägen med samma diagnostik och källpositioner.

Direkt `for`-iteration över en typad strängs `.bytes()` eller `.runes()`
kan strömma från en behållen strängsnapshot. Den vägen bygger eller
kopierar ingen konverteringsarray. Lagrade konverteringar och uttryck som
inte matchar den begränsade direkta vägen använder vanliga arrayer.
Källan utvärderas en gång och behålls genom `continue`, `break` och retur.

`check` och kontroll av oanropade kroppar använder ett emissionsläge utan
lagrade maskinord, etikettpositioner eller relokeringar. Virtuella räknare
bevarar deterministiska positioner för de sänkningshjälpare som fortfarande
körs. Den nya modellens delmängd valideras utan kodgenerering. En
självständig semantisk frontend för hela språket återstår.

Byggkedjans valbara [fastider i text eller JSON](incremental.md#mät-byggkedjans-faser)
gör kostnaden synlig. Optimeringar jämförs med samma källor och säkerhetsregler
genom `tools/performance.py`; `--include-check` lägger till kontrolltider,
och byggsviten omfattar även Tabell som fristående konsumentprojekt.
Med samma binär i båda positionerna och
`--baseline-flag=--scalar-opt=off` isoleras optimeringslägets effekt.

Korrektheten provas med referensresultat för signerad division och rest,
IEEE 754, kortslutning och diagnostik. Minnesproven kör även om samma
Wasm-instans och kontrollerar att de frigjorda blocken återanvänds.
[Körbara kontrakt](../tools/runtime-contracts.py) · [Funktionscache](incremental.md).

Tidigare prestandamätningar och de då använda kompilatorrevisionerna finns i
[benchmarkarkivet](../benchmarks/README.md).
Den aktuella självhostens [före/efter- och av/på-mätning](../benchmarks/2026-11-11-verified-mutation/README.md)
omfattar både runtime, kompletta byggen och `check`.
Sortering och självbygge blir snabbare; kostnaden för modellverifiering
på genererad stresskod redovisas separat. Nästa kostnadsarbete behöver
profilera modellbygge, analys och verifiering utan att försvaga kontrollerna.
