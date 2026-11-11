# Kodgenerering i Dunder

Vanliga byggen går genom [typkontrollen och kodgeneratorn](../selfhost/compile.ddr).
Kompilatorn avger instruktioner, lokala etiketter och relokeringar per funktion.
Den tar med de runtimefunktioner som programmet behöver. Overflow, ogiltiga
index och ogiltiga konverteringar behåller sina kontroller även vid korsbyggen.

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
körs. Detta skiljer bort utdataallokeringen; en separat typad syntaxmodell
och en självständig semantisk frontend återstår.

Byggkedjans valbara [fastider i text eller JSON](incremental.md#mät-byggkedjans-faser)
gör kostnaden synlig. Optimeringar jämförs med samma källor och säkerhetsregler
genom `tools/performance.py`; `--include-check` lägger till kontrolltider,
och byggsviten omfattar även Tabell som fristående konsumentprojekt.

Korrektheten provas med referensresultat för signerad division och rest,
IEEE 754, kortslutning och diagnostik. Minnesproven kör även om samma
Wasm-instans och kontrollerar att de frigjorda blocken återanvänds.
[Körbara kontrakt](../tools/runtime-contracts.py) · [Funktionscache](incremental.md).

Tidigare prestandamätningar och de då använda kompilatorrevisionerna finns i
[benchmarkarkivet](../benchmarks/README.md).
Den aktuella självhostens [före/efter-mätning](../benchmarks/2026-11-10-selfhost-performance/README.md)
omfattar både runtime och kompletta byggen.
