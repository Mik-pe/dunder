# Vad kostar en rad att kompilera?

På Apple M5 tog ett optimerat macOS-bygge av 101 024 rader på revision
`326ee46` 504 ms i median, ungefär 201 000 rader/s. För en miljon rader/s
måste hela processen rymmas inom 101 ms.

## Skalning och faser

Korpuset genererades med `tools/performance.py` och har 50 funktioner per
modul med 96 additioner i varje funktion. Med 5, 10 och 20 moduler plus
huvudmodulen tog byggena 131, 255 respektive 503 ms för 25 259, 50 514
respektive 101 024 rader. Det är nära linjärt för den här fördelningen av
funktionsstorlekar. Stora enskilda funktioner kan ha en annan kostnadsbild.

| Körning på hela korpuset | Total processtid | Läsning, lexning och namnupplösning | Kompilering | Mach-O och signering |
| --- | ---: | ---: | ---: | ---: |
| Standard, fyra workers | 504 ms | 75 ms | 361 ms | 60 ms |
| En worker | 839 ms | 72 ms | 704 ms | 59 ms |
| Åtta workers | 496 ms | 74 ms | 359 ms | 60 ms |
| Typkontroll med `check` | 172 ms | 75 ms | 94 ms | — |

Varje cell är en separat median av sju körningar med varm filcache och utan
inkrementell cache. Fasvärdena kommer från `--timings=json`. Processtiden
omfattar även start och avslut. Diagnostikkörningar med `--mir-opt=off` tog
503 ms och med `--mir=off` 330 ms. Den senare går via det seriella
kodgenereringsspåret och ger annan maskinkod. Båda programmen skrev `595500`.

## Var mellanrepresentationen kostar tid

Huvudtråden emitterar maskinkod och bygger sedan en mellanrepresentation
(MIR) genom att avkoda den. På korpuset skapades 498 017 MIR-operationer.
Varje operation har 29 obligatoriska kolumnposter för bland annat operander,
registereffekter och ursprungsposition. Huvudtråden packade funktionerna i
138 batcher om sammanlagt 80 790 264 byte i nyttolast till arbetstrådarna,
som packade upp kolumnerna igen. Svaren innehöll 7 407 844 byte.
[Definition och konstruktion av MIR](../../../selfhost/mir.ddr) ·
[Packning och uppackning](../../../selfhost/mir_task.ddr)

Separata klockprober på huvudtråden visade cirka 145 ms för emission av
funktionskropparna, 109 ms för att bygga MIR och 52 ms för att packa den.
Oberoende prober kan påverka körtiden. Arbetstrådarna kör samtidigt, så
dessa tider ska inte adderas till tabellens kompileringsfas. Ett försök med
kompakt kodning för konstanta och sekventiella kolumner ändrade medianen från
501 till 497 ms i sex alternerande mätpar och gav byteidentisk maskinkod.

I frontend kostade själva kopian av den platta tokenlistan, med 607 062
poster och 9,71 MB kolumndata, cirka 1,1 ms. Lexern, modulskanningen och
skapandet av den upplösta tokenlistan tog betydligt längre tid. I
Mach-O-steget tog SHA-256 över 123 sidor på 16 KiB ungefär 54 ms av 60 ms.
Proberna för dessa delsteg återskapade samma körbara binär som standardbygget.

Nästa experiment är att låta varje arbetstråd bygga sin MIR från maskinord,
etiketter, relokeringar och källmetadata. De räknade arrayerna för dessa indata
upptar cirka 6,1 MB före meddelandehuvuden och eventuella extra uppgifter
om externa etiketter. Det skulle flytta avkodningen från huvudtråden och
undvika att packa den breda MIR-tabellen. Även frontend och bildsignering
behöver bli snabbare för att hela bygget ska rymmas inom 101 ms.

[Råmätningar och räknare](throughput.json)
