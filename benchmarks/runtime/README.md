# Runtime före och efter optimering

`tools/runtime-bench.rs` jämför två Dunder-kompilatorer med samma frysta
källor, körningsindata och säkerhetsregler. Mätverktyget är Rust/std och
ingår inte i Dunders byggväg. Dunderbyggen och de färdiga programmen körs
med tom miljö och en `PATH` utan externa verktyg.

```sh
cargo build --release
rustc --edition 2021 -O tools/runtime-bench.rs -o build/runtime-bench-driver
build/runtime-bench-driver /path/to/baseline /path/to/candidate build/runtime-results 30
```

Kör från repositoryts rot. Båda kompilatorerna måste stödja värdmålet och
samma källkod. Verktyget bygger båda varianterna, verifierar en uppvärmning
och startar sedan 30 nya processer per variant och arbetsfall. Ordningen
växlar mellan baseline/candidate och candidate/baseline. Kör utan andra
avsiktligt tunga jobb; OS-schemaläggning och CPU-frekvens kan ändå variera.

| Arbetsfall | Fryst arbete | Resultatkontroll |
| --- | --- | --- |
| `numeric` | 1 000 000 steg med funktionsanrop, kontrollerad heltalsaritmetik och modulo. | Oberoende Rust-beräkning av hela sekvensens summa. |
| `scan` | 3 000 lån och summeringar av 1 024 dynamiska arrayelement; ett element ändras varje varv. | Rust uppdaterar sin egen vektor och summerar den. |
| `text` | 2 000 genomgångar av en UTF-8-text med svenska, grekiska tecken och emoji; ord och kodpunkter räknas. | Rusts `chars()` och separat delning på de tre angivna blanktecknen. |
| `sort` | 120 in-place-sorteringar av 256 heltal via Dunders standardbibliotek. | Rusts `sort_unstable()` och en positionsviktad summa av alla resultat. |

Varvantal, numeriskt frö och text kommer från programargument. Programmens
resultat måste stämma efter varje mätning. Ingen felaktig eller avbruten
körning får bidra till statistiken. Algoritmer och källor är identiska för
båda varianterna; endast kompilatorn byts.

`samples.csv` innehåller råtider och faktisk arbetsordning. `summary.csv`
innehåller median, nearest-rank p95, binärstorlek och kvoten kandidat/baseline;
lägre kvot är bättre. Väggtiden omfattar processstart, argumentläsning och
utskrift av kontrollsumman. Det är inte CPU-cykler isolerade till innerloopen.

`metadata.txt` redovisar värd, CPU när `/proc/cpuinfo` finns, kompilatorfiler,
källor, indata, kontrollsummor och FNV-1a-fingeravtryck. FNV används enbart för
reproducerbarhet, inte som kryptografiskt integritetsskydd. En byggtid per
variant sparas som diagnostik; den är ingen statistisk byggprestandamätning.

Före/efter-mätningarna jämför de fyra arbetsfallen mellan två Dunder-versioner. Resultaten visar
effekten av denna kompilatorändring på dessa program och styrker varken
generell prestandaparitet eller samma snabbhet på andra processorer.
ARM64-körning under QEMU lämpar sig för korrekthetskontroll, inte för
anspråk om verklig ARM64-prestanda.

## Resultat 2026-11-10, Linux/x86-64

Baseline är `3039dbe47877c90975223ff67e09f4decf99752b`. Kandidaten byggdes
från den revisionen med optimeringsändringarna i samma commit som denna
rapport, före commit. Båda kompilatorerna byggdes med `cargo build --release`
och Rust 1.90.0. De exakta binärernas SHA-256 finns i
[metadata](2026-11-10-linux-x86_64/metadata.txt); metadatafältet
`candidate_checkout` visar därför basrevisionen, inte kandidatens slutliga
commit. Värden är en AMD EPYC 9V74 med Linux 6.18.57, x86-64.

| Arbetsfall | Median före | Median efter | Lägre mediantid | p95 före → efter | Binär byte före → efter |
| --- | ---: | ---: | ---: | ---: | ---: |
| Heltal och anrop | 45,570 ms | 37,830 ms | 17,0 % | 51,410 → 42,617 ms | 170 856 → 135 496 |
| Lånad arraysummering | 96,500 ms | 90,340 ms | 6,4 % | 102,613 → 101,777 ms | 178 384 → 141 120 |
| UTF-8 och ord | 58,236 ms | 53,322 ms | 8,4 % | 68,124 → 56,892 ms | 176 824 → 139 888 |
| Standardbibliotekets sortering | 148,574 ms | 130,508 ms | 12,2 % | 165,653 → 157,105 ms | 182 640 → 143 472 |

Samtliga 240 mätta körningar och åtta uppvärmningar gav exakt de oberoende
förväntade resultaten. Binärerna blev cirka 21 % mindre. Andra avsiktligt
tunga agentjobb pausades under mätningen, men värdens övriga schemaläggning
kontrollerades inte. Spridningen och den mindre förändringen i scans p95
syns i tabellen; mätningen är ingen garanti för varje enskild körning.

[Alla observationer i faktisk ordning](2026-11-10-linux-x86_64/samples.csv)
och [sammanställda råmått](2026-11-10-linux-x86_64/summary.csv) följer med.
Detta mäter den samlade ändringen i delad kodgenerering och x86-64-lowering;
det isolerar inte varje deloptimerings bidrag.

## ARM64 macOS och optimerad C

Program byggda med Rust-bootstrappen kördes på Apple M5 med 30 växelvis ordnade processer
per variant. Dunders kontroller för heltalsöverflöde och arraygränser är
aktiva. Tabellen visar medianen för Dunder dividerad med medianen för den
angivna C-varianten. Varje kolumn kommer från en egen parvis mätning.

| Arbetsfall | Dunder före → efter senaste loopändringen, ms | Mot clang `-O3 -ftrapv` | Mot clang `-O3` |
| --- | ---: | ---: | ---: |
| Numerik | 6,470 → 5,776 | 1,40× | 1,40× |
| Arrayläsning | 6,520 → 2,136 | 1,13× | 1,69× |
| UTF-8-text | 3,421 → 3,216 | 1,67× | 1,78× |
| Sortering | 4,898 → 2,907 | 1,75× | 1,70× |

Tiderna omfattar processstart, argument och utskrift. Alla program måste
producera samma kontrollsumma. Råvärden inklusive p95 och kompilatorernas
checksummor finns för [Dunder före/efter](2026-11-10-macos-arm64/loops/),
[clang med overflowkontroll](2026-11-10-macos-arm64/clang-trapv/) och
[clang med vanliga optimeringsflaggor](2026-11-10-macos-arm64/clang/).

Den självhostade Stage 3-kompilatorn har en separat kodgenerator. Program
byggda med den tog 3,37× så lång tid som clang `-O3` för numerik, 12,82×
för arrayläsning, 17,19× för UTF-8 och 7,37× för sortering. Samma arbetsfall,
indata och verifiering användes i 30 parvisa körningar. Stage 3-binärens SHA-256
var `ceebbe5822ca29edaf7b24cde9e3f5b994c7b040ae4426d0c9cfb1778043e48f`.
[Sammanställning och råmätningar](2026-11-10-macos-arm64/selfhost-clang/).

```sh
build/runtime-bench-driver --compare-c /usr/bin/clang /path/to/dunder build/versus-checked-c 30
build/runtime-bench-driver --compare-c-unchecked /usr/bin/clang /path/to/dunder build/versus-c 30
```
