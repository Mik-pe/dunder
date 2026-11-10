# Optimerad maskinkod

Optimeringarna ingår i vanliga `dunder build`, `run` och `test`. De använder
Dunders egen kodgenerator och kräver ingen extern optimerare, assembler
eller länkare. Säkerhetskontrollerna gäller även i optimerad kod.

## Tre tydliga steg

1. Typkontrollen fastställer typer, värdesemantik och ägda platser.
2. `src/native/optimize.rs` väljer enklare instruktioner direkt från det
   typade trädet. Den gemensamma kodgeneratorn producerar instruktioner
   och etikettfixeringar för båda processorarkitekturerna.
3. ARM64 skrivs direkt. x86-64 sänker den gemensamma instruktionsvokabulären
   till x86-maskinkod och väljer egna register och instruktioner.

Det finns ingen extra interpreter eller optimeringsprocess vid körning.
Optimeringsreglerna är lokala och kan utökas utan att ändra språkets
semantik eller kräva en separat generell mellanrepresentation.

## Vad som förenklas

- Rena, lyckade heltals- och booleska konstantuttryck beräknas vid bygge.
  Konstanta `if` väljer bara den körda grenen i maskinkoden; typkontrollen
  har redan kontrollerat källkoden.
- Heltalsjämförelser i villkor hoppar direkt från jämförelseflaggorna.
  De behöver inte först materialisera en bool och sedan testa den.
  `&&`, `||` och `!` behåller sin kortslutning.
- Enkla högra operander laddas direkt i sitt arbetsregister, utan att
  vänsterledet först sparas på stacken. Lokala smala heltal teckenutökas
  även efter skrivning genom en muterbar referens.
- Addition/subtraktion med små konstanter använder immediate-instruktioner
  med kvarvarande overflowkontroll. Identiteter som `x + 0` och `x * 1`
  förenklas. `x * 0` utvärderar fortfarande `x` före resultatet noll.
- Returer utan ägda resurser behöver inte spara resultatet runt tom städning.
  Arrayer och referensräknade strängar behåller sin städning.
- x86-64 väljer kortare MOV-instruktioner för små konstanter. MOV bevarar
  flaggorna, till skillnad från att nollställa ett register med XOR.

## x86-64-register

| Gemensamt arbetsregister | Fast x86-64-register |
| --- | --- |
| `x0`, resultat och första argument | `r12` |
| `x1`, vänster operand och andra argument | `r13` |
| `x2`, aritmetik/adressarbete och tredje argument | `r14` |
| Privat värdestackpekare | `rbp` |

Dessa register överlever Linux-systemanrop och är callee-saved i systemets
C-ABI. Samma mappning gäller på alla kontrollflödesvägar; inga cacheflushar
eller olika registerval behöver förenas vid grenar. Övriga arbetsregister
har tills vidare platser i den privata registerbanken. `rbx` reserveras för
värdstackens justering runt externa anrop. Kontrollerna för Dunders separata
värdestack och dess skyddssida finns kvar.

## Självhosting

Den Dunder-skrivna ARM64-backenden använder direkt LDUR/STUR för lokala
värden inom 256 byte från framepekaren. Större offset som ryms i 12 bitar
använder en immediate-adress; större ramar behåller den generella vägen.
Även adresser för borrows får den korta formen. Registerflytt till samma
register utelämnas. Den nya backendens stage 2 och stage 3 är byteidentiska.
Bootstrapens övriga optimeringar är ännu inte portade till selfhost.

## Korrekthet och mätning

En misslyckad konstantberäkning lämnas som runtimekod, så dess fel inträffar
på rätt plats och bara om vägen körs. Exempelvis ger både division och
remainder med `i32::MIN` och `-1` fortsatt overflow. Flyttal omassocieras
inte; NaN, signerad noll och jämförelser behåller tidigare regler.

`tests/optimization.rs` jämför heltalsresultat med en oberoende Rust-referens
och provar gränsvärden, sidverkningar mellan operander, kortslutning,
runtimefel och IEEE-fall på värdmålet samt ARM64 direkt eller via QEMU.
Selfhost-tester täcker stackoffsetens gränser, borrows och kompilatorns
fixpunkt. Befintliga minnes-, system- och Vulkan-tester bevakar ägarskap,
registerbevarande och anrop till riktiga systembibliotek.

[Runtime-sviten](../benchmarks/runtime/README.md) innehåller mätverktyg,
frysta program och råvärden från före/efter-körning. Resultaten gäller
de uppmätta arbetsfallen och maskinen. Native ARM64-prestanda och
C/C++-paritet är ännu inte uppmätta.

Nästa steg att pröva är registerallokering för lokaler över hela loopar,
kostnadsstyrd inlining och borttagning av bevisat överflödiga gränskontroller.
Varje steg behöver både semantiska regressionstester och mätningar innan
det kan motiveras som en prestandavinst.
