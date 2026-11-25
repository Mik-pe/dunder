# Mätresultat

Samla nya mätresultat här med revision, arbetslast och mätvärden.
Tidigare körrecept finns i Git-historiken.

Den 25 november mättes rena och oförändrade byggen av Tabell, HTTP-servern
och en terminalräknare. Serien omfattar även privata ändringar, ett blandat
korpus på 123 188 rader och kompilatorns självbygge.
[Projektbyggen med median, p95 och råprover](compiler/2026-11-25-project-builds/README.md)

Rader och kolumner kördes med samma kompilatorrevision på Apple M5 samt i
en lokal, översatt Linux-gäst för x86-64. [Mätvärden och råprover](layout/2026-11-22/README.md)

[Parallell byggtid på Apple M5](compiler/2026-11-18-parallel-build-latency/README.md) mäter modullexning, funktionsschemaläggning och Mach-O-signering mot samma låsta källor.

[Byggtid, körprestanda och schemaläggning med den gemensamma funktionsrepresentationen](compiler/2026-11-15-shared-frontend/README.md) jämför sju byggkorpusar, fem native-program och tre schemaläggningsändringar på Apple M5.

[Funktionspipelinens mätning under samtidig verifieringslast](compiler/2026-11-15-function-pipeline-load/README.md) omfattar sju byggkorpusar, maximalt resident minne och fem native-program med Tabell.

Funktionslokalt MIR på Apple M5 med källorna låsta till `6dffea8` och Tabell till `978af73`. Byggmedianen gick från 505,2 till 92,4 ms för referenskorpuset och från 641,2 till 541,4 ms för självbygget. [Resultat och rådata](compiler/2026-11-15-worker-lift/README.md)

På Apple M5 tog ett självbygge med den nya kompilatorn 458,91 ms. Kontrollserien med oförändrad kompilatorkälla isolerar vinsten från den genererade maskinkoden. [Nativ maskinkod och självbygge, 2026-11-15](compiler/2026-11-15-native-code/README.md)

På Apple M5 gick självbygget från 677,5 till 625,8 ms efter ändringarna i lexern, parsern och MIR. [Mätserie för tre rena byggen, 2026-11-15](compiler/2026-11-15-build-latency/README.md)

Mätserierna från 10–12 november finns med sina rådata i [arkivet](archive/).
För att reproducera ett äldre resultat, använd verktygen och källkopiorna som
hör till mätningen. [Historiskt resultatregister med körunderlag](https://github.com/Mik-pe/dunder/blob/e67fa3d887be84a99707c20d5266105c43edb93a/benchmarks/results.md)
