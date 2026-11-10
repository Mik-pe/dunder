# Aktivt mål för GLM 5.3: bygg Dunder och använd Dunder

Uppdrag från användaren, 2026-11-10. Arbeta vidare i denna worktree på
`dogfood/arm64`. Läs även README.md, docs/arkitektur.md och docs/prestanda.md.

## Slutmål och godkännandebevis

Leverera en självhostad Dunder-kompilator för arm64-macos. Från en ny checkout
ska den dokumenterade, granskningsbara frökedjan bygga kompilatorn. Därefter ska
`dunder build` kompilera kompilatorns riktiga Dunder-källa till D1, D2 och D3,
där D2 och D3 är byteidentiska och båda klarar samma språktester. Verktyget ska
användas för att utveckla sig självt, inte bara köra ett fristående demo.

Minnessäkerhet utan GC, direkt ARM64-kodgenerering utan generell IR/SSA och
egen länkning, Mach-O-skrivning och ad hoc-signering följer arkitekturdokumentet.
Den normala byggvägen får inte anropa LLVM, GCC, clang, extern assembler,
länkare eller codesign, eller transpilera till C. Externa verktyg får användas
för oberoende inspektion, differentiella tester och C/C++-referenser. Befintlig
Python under tools/dev är utvecklingsstöd för att beskriva och kontrollera
ursprungsfröet; den får inte bli ett dolt permanent beroende i slutprodukten.

Arbeta vidare från varje verifierat delsteg. En plan, första exitkod, parserdemo
eller endast ett påstående om färdig implementation uppfyller inte slutmålet.

## Börja där förra körningen fastnade

Den tidigare sessionen `ses_e3db03358ffeOOSNbPvn9qdOx3` slutade 2026-11-09
23:25 svensk tid. Den lämnade origin/etapp0.s, origin/calltest.s,
tools/dev/seedasm.py och tools/dev/diffenc.py utan commits. Dess logg visar
95 jämförda instruktionsord utan skillnader, men inga fungerande körbara bilder:
försöken slutade med OSError 86, Bad CPU type in executable. Sista anropet
nekades på grund av åtkomst till /tmp i en icke-interaktiv körning. Det var
inte ett nytt beslut från användaren att stoppa projektet.

Konkreta fel att kontrollera först i seedasm.py:

- CPU-typen skrivs som `0x1000000C`, men installerad mach/machine.h anger
  CPU_TYPE_ARM64 = 12 | 0x01000000 = `0x0100000C`.
- `sizeofcmds` verkar anges som `32 + cmds_size`; kontrollera mot formatet,
  eftersom fältet ska beskriva laddkommandona, inte hela mach_header_64.
- CodeDirectory-magivärdet är `0xFade0C17`. Kontrollera själv mot Apples
  definitioner och en giltig referensbild; anta inte att föregående sessions
  slutsatser om signaturformatet är korrekta.

Kontrollera hypoteserna med lokala SDK-definitioner och byteinspektion. Skilj
CPU-/headerfel, laddar-/entrypointfel och signaturfel åt. Gör ett litet
reproducerbart prov per fel. När ett prov misslyckas: kontrollera förutsättningar
och jämför det faktiska fältet innan du provar fler varianter. Förändra inte
mängder av laddkommandon samtidigt. Spara regressionstest för verkliga fel.

## Kontrollpunkter i ordning

1. **Körbar och reproducerbar bild.** Återskapa det dokumenterade ursprungsfröet.
   Kör etapp0 med minst två olika returvärden och kör provet med riktigt
   funktionsanrop. Verifiera faktisk processstatus, ARM64-typ, laddkommandon,
   entrypoint och signatur oberoende. Samma indata ska ge samma byte. Bevara
   den differentiella kodarverifieringen. Commit först när proven fungerar.
2. **Fröet bygger sig självt.** Koda det lilla fröverktyget i dess eget läsbara
   fröspråk. Det ska lösa symboler och bygga sin egen källa byteidentiskt samt
   ett andra program med anrop. Checka in ursprung, källa, kontrollsummor och
   exakta reproduktionskommandon. Python får bara återskapa/kontrollera det
   separata dokumenterade ursprunget, inte ersätta självbygget.
3. **Riktigt `dunder build`.** Manifest till körbar fil via egen verktygskedja.
   Lägg till typade heltal, parametrar, lokaler, riktiga anrop, villkor, loopar
   och kontrollerad aritmetik. Testa varierad källa och körningsindata,
   källpositioner vid fel, overflow/division och Apples ABI inklusive stack,
   bevarade register och mer än åtta heltalsargument. Trasiga byggen ska inte
   ersätta en tidigare giltig binär. Dokumentera den faktiskt stödda delmängden.
4. **Använd Dunder till verkligt arbete.** Skriv lexer i Dunder som läser fil
   vid körning, ger token med källpositioner och läser sin egen källa. Använd
   den i nästa kompilatorsteg. Fortsätt med diagnostik, parser och backend.
   När minne införs ska ägande, flytt, lån och initiering kontrolleras: lagliga
   program godtas, dangling slices/use-after-move/dubbelfrigöring avvisas,
   och städning vid retur, grenar och loopar verifieras med riktiga fall.
5. **Självhosting.** Bygg D1 → D2 → D3 från riktig kompilatorkälla. Spara
   SHA-256, binärjämförelse och resultat från samma positiva och negativa
   testkorpus med D2 och D3. Verifiera reproducerbarhet från ren checkout.
6. **Mät och optimera.** Följ docs/prestanda.md. Mät hela vägen från källa
   till signerad körbar fil och kontrollera körresultatet. Publicera rådata,
   korpus och faktiska begränsningar. Målet är minst 100k verkliga källrader/s
   och konkurrenskraftig körprestanda mot C/C++. Märk delmängder och syntetisk
   stresskod; gör inga anspråk på att hela kontraktet klaras utan dess bevis.

## Arbetsform och beständig status

- Starta med att skriva docs/status.md med status RUNNING, aktuell kontrollpunkt,
  nästa verifierbara handling och en kort checklista. Använd även din todo-lista.
- Uppdatera status efter verifierade delsteg och före kontextkomprimering.
  Spara exakta kommandon, exitstatus, resultat och commit-id. Håll rapporten
  kort och använd filer för stora råloggar. Skilj verifierat, misslyckat och
  ännu oprövat. Efter komprimering: läs uppdraget och status och fortsätt.
- Gör små lokala commits på dogfood/arm64 efter verifierade kontrollpunkter.
  Bevara och granska de fyra befintliga filerna. Återställ inte andras arbete.
- Du får ändra och testa projektet i denna worktree och dess byggkataloger.
  Använd katalogen i miljövariabeln TMPDIR för temporära externa testfiler.
  Åtkomsten är förberedd för denna körning; använd inte hårdkodad /tmp.
- Git-metadata för den gemensamma worktreen är tillåten. Lämna main-checkouten
  /Users/mikpe/dev/dunder, andra projekt, globala inställningar och remoten
  ifred. Ingen push eller extern publicering. Ingen installation krävs för
  att komma igång: systemets verktyg finns för oberoende verifiering.
- Arbeta självständigt utan frågor eller väntan på interaktiva svar. Ta små,
  reversibla implementationbeslut och dokumentera dem. Fortsätt efter vanliga
  tekniska bakslag. Vid återkommande fel ska du byta till orsaksanalys och
  reproduktion, inte upprepa samma anrop eller sänka acceptanskraven.
- Avsluta inte efter en delseger. Om du når en verklig extern gräns som du
  inte kan lösa: isolera den, prova oberoende delarbete och lämna status BLOCKED
  med exakt första fel, kommandot, vad som uteslutits och vad som krävs härnäst.
  Markera DONE bara när självhostingsbeviset finns och prestandakontraktets
  uppnådda/missade delar redovisas ärligt. En verktygsprocess med exitkod 0
  eller din egen sammanfattning är inte i sig bevis för att målet är klart.
