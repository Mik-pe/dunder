# Byggtid med funktionslokalt MIR på Apple M5

| Program | Helt bygge före (ms) | Helt bygge efter (ms) | Fram till färdig maskinkod efter (ms) | Rader/s för hela bygget efter |
| --- | ---: | ---: | ---: | ---: |
| Referenskorpus | 505,2 | 92,4 | 84,7 | 1 093 694 |
| Unik konstant per kropp | 505,3 | 166,9 | 159,4 | 605 336 |
| Unika kroppar med blandade uttryck | 513,7 | 263,6 | 219,4 | 383 299 |
| Dunders kompilator | 641,2 | 541,4 | 445,2 | — |
| Tabell | 14,7 | 12,7 | 8,2 | — |

Den sista tidskolumnen summerar kompilatorns klockor för modulladdning,
byggförberedelse och kompilering. Processtiden omfattar även start,
Mach-O-formatering, ordinarie signering och publicering av den körbara filen.

## Vad som faktiskt byggdes

De första tre programmen är på 101 024 rader vardera och anropar 1 000
hjälpfunktioner. I referenskorpuset är alla hjälpkroppar identiska efter
namnupplösning. I den andra varianten har varje kropp en egen konstant. Den
tredje blandar beroenden av parametern med addition och subtraktion och har
också en egen konstant per kropp. De körbara filerna ska skriva `595500`,
`1095000` respektive `24000`. Självbygget kompilerar den låsta versionen av
`selfhost/main_parallel.ddr` och får sedan bygga och köra `examples/hello.ddr`.
Tabell körs på sin CSV-fil och resultatet jämförs med `expected.csv`.

En separat instrumenterad kompilator räknade 998 träffar
i den bygglokala skalärmodellcachen för referenskorpuset. De två varianterna
med unika kroppar hade 0 respektive 0
träffar. De optimerade referensfunktionerna har egna etiketter och binära
adresser. Modellåteranvändningen och sammanslagningen av säkra aritmetiska
uppdateringar står därför för en del av skillnaden mellan arbetslasterna.

## Datamängden vid arbetsgränsen

Med samma 498 017 MIR-operationer och byteidentisk utdata sänkte bytet av
arbetsgräns indatamängden från 80 790 264 byte till 6 539 668 byte.
För att isolera formatet användes den äldre skalära kodgenereringen i den
kompakta varianten. När även de nya skalära optimeringarna användes blev det
18 017 MIR-operationer och 419 668 byte på referenskorpuset.
Mätningen summerar innehållet i batchernas maskinords-, metadata- och
beskrivningskolumner före sändning. Uppgiftshuvuden och svar ingår inte.
[Baslinjens transportmätning](../2026-11-15-build-latency/throughput.md)

## Reproducera mätningen

Mätverktyget startar en ny process för varje bygge och växlar mellan de två
kompilatorerna. Medianerna bygger på tolv mätpar efter ett uppvärmningsbygge
per variant. Fyra workers, varm filsystemcache och avstängd inkrementell
byggcache används i båda fallen. Alla uppmätta bilder jämförs med samma
kompilators uppvärmningsbild. [Rådata och kontrollsummor](samples.json)

Arkivera eller checka ut basrevisionen och bygg båda kompilatorerna med
`tools/bootstrap.sh`. Ange basrevisionens källor även för den nya kompilatorn.

```sh
python3 benchmarks/compiler/2026-11-15-worker-lift/measure.py \
  /path/to/baseline/dunder /path/to/candidate/dunder \
  /path/to/source-at-6dffea8 /path/to/dunder-hundmat/tabell /tmp/dunder-report.json
```
