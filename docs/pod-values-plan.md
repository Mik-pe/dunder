# Plan för POD-värden och gemensam anropsmodell

Detta är ännu en design, inte implementerad funktionalitet eller uppmätt
prestanda. Kompilatorns profiler och POD-dogfood visar varför små poster
behöver en tydligare representation. Första delmängden är vanliga flata
structar med bool, signerade/osignerade heltal, rune och råpekare, utan
ägda barn. Någon ny källsyntax behövs inte.

## Värdeplats och lagringsägare

Inför en typad värdeplats som anger nominell layout, muterbarhet,
livslängd och lagring: framevärde, anroparägd snapshot, lånad plats eller
ägd heapbox. Behåll först dagens åttabytesceller och fältordning.
Packning är ett separat beslut.

POD-innehåll saknar ägda barn, men en heapbox måste fortfarande frigöras.
Att bara ändra `owned(ty)` till false vore därför fel. Befintliga
heapcontainrar kan initialt behålla boxade POD-fält; kopiering mellan en
frameplats och en container gör då boxningen explicit. En runtimepekare
behöver aldrig gissa om den kom från stack eller heap.

`fixed_pod` gäller i dag fasta skalärarrayer, inte alla vanliga structar.
`@c_layout` använder också heapkonstruktion och är inte denna mekanism.
`compile.Result` och `x86.Result` innehåller ägda fält och är inte POD.

## En deklarationsvald intern ABI

En gemensam, versionssatt signaturbeskrivning ska användas av caller och
callee oavsett om funktionskroppen använder typed IR eller legacyfrontend.
Optimeringsläge och parseracceptans får aldrig välja anropskonvention.

Första förslaget är värdeargument som pekare till färdiga privata snapshots,
POD-retur till en destination som anroparen tillhandahåller och fortsatt
skalärretur i `x0`. Befintliga argumentplatser `x0..x7` och ordningen för
stackargument behålls. `x8` kan bära den dolda returdestinationen i den
interna registervokabulären; callee måste spara den direkt eftersom kroppen
använder `x8` som scratch. Detta behöver verifieras för varje mål och är
inte ett nytt C-ABI.

Snapshoten tas när argumentet utvärderas, före nästa argument:
`consume(packet, replace(&mut packet))` måste få postens tidigare fältvärden
i det första argumentet. Det räcker inte att spara pekaren och kopiera efter
alla argument. Callee frigör inte anroparens snapshot.

Returdestinationen är först privat och skild från både argument och
befintliga variabler. `packet = make(...)` materialiserar färdigt resultat
före tilldelning, så att senare fältuttryck eller anrop inte ser ett
halvskrivet värde.

## Lån, adresser och projektioner

Vanliga structlån använder i dag en cell med payloadpekaren; C-layout-lån
pekar direkt på payload. `inline_place` är därför inte en tillräcklig
generell platsmodell. Behåll den nuvarande lånekonventionen i första steget.
Hela muterbara structlån och adressutsatta lokaler behåller uttryckligt
boxad lagring tills ersättning och adressidentitet är modellerade.

En projektion som `make_packet().field` läses medan resultatplatsen lever.
En hel POD-projektion kopieras till mottagarens lagring. Ingen pekare till
callee-frame eller återanvänd looptemporär får lämna funktionen som värde.
Kopiering av ett råpekarfält förlänger inte det utpekade objektets livslängd.

## IR och verifieringsordning

Den typade funktionsmodellen behöver nominella layout-id:n, adresserbara
värdeplatser, fältprojektioner och konstruktion/kopiering till destination.
Returen använder signaturens resultatdestination. När anrop tillkommer
ska de använda samma deklarationsbeskrivning som legacyfrontend.

Verifieraren behöver kontrollera layout och framegränser, fullständig
initiering, resultatdestination och att frameplatser inte flyr. Arbetsbudget
för framepayload får välja uttryckligt boxad lagring, men aldrig byta ABI
eller avvisa tidigare giltig kod.

Börja med gemensamma layout-/ABI-hjälpare och destinationsmaterialisering
i legacyfrontend. Ta sedan interna temporärer, värdeparametrar och enkla
lokaler. Typed IR kan därefter godta en liten anropsfri POD-delmängd, med
legacycaller och typed callee som ett konkret korsprov.

Korrekthetsgrinden omfattar senare muterande argument, rekursion, nästlade
och tidiga returer, självtilldelning, loopåterinträde, temporärprojektioner,
containerboxning, lån/råadresser och exakt utskrift före fälla. Alla mål och
scalar on/off måste använda samma ABI. Vanliga POD-structar blir inte
automatiskt C-structar, och callbacks får inga nya aggregatsignaturer här.
Fragmentidentiteten måste få ny ABI-/layoutversion och passningssätt.

Första steget bevisar inte registerreturer, allmän scalar replacement,
packad layout eller boxeliminering bakom råadresser. Det ska först visa
korrekt och mätbar borttagning av heapkonstruktion/kloning för denna
uttryckliga delmängd.
