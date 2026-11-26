# Formattering, dubblering och byggtid på Apple M5

Den självbyggda kompilatorn från `b4bea71` jämfördes med Dunder 0.13.0
(`da8fcdb`) på ARM64/macOS. Varje mätpar körde båda versionerna. Startordningen
växlade mellan paren och ett inledande uppvärmningspar är uteslutet.
Andra processer var aktiva på maskinen. [Råtider i nanosekunder](samples.csv)

| Arbetslast | Par | 0.13.0, median | b4bea71, median | Par där b4bea71 var snabbare |
| --- | ---: | ---: | ---: | ---: |
| Formatera kompilatorns `compile.ddr` | 20 | 131,21 ms | 106,48 ms | 18/20 |
| Kör 125 miljoner kontrollerade dubbleringar | 16 | 461,25 ms | 338,02 ms | 14/16 |
| Bygg kompilatorn från samma källkod | 10 | 966,85 ms | 960,07 ms | 4/10 |
| Bygg blandkorpusen | 10 | 375,40 ms | 374,38 ms | 5/10 |

Formatteringen körde `fmt --stdout selfhost/frontend/compile.ddr` på källan
från `da8fcdb`. Alla 800 184 utdatabyte var identiska. I en separat körning
med `/usr/bin/time -l` sjönk maximalt resident minne från 81 854 464 till
72 269 824 byte.

De båda kompilatorerna byggde [samma loop](double.ddr). Körningen med argumenten
`104729 125000000` gav `494209340` för båda binärerna. Tiden i tabellen gäller
körningen av det färdiga programmet.

Helbyggena använde explicit källfil och automatiskt antal arbetare, tio på
denna värd. Självbygget använde `selfhost/main.ddr` från `da8fcdb`.
Det blandade programmet från [korpusgeneratorn](../../../tools/performance_mixed.ddr)
gav kontrollsumman `474148`. Skillnaderna i helbyggestid var under en procent
och varierade i riktning mellan mätparen.
