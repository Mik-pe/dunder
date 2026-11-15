# Nästa steg för Dunder

Föreslagen ordning efter ägarskapsgranskningen. Detta är en utvecklingsplan;
funktionerna nedan är inte implementerade. Nuvarande kontrakt finns i
[minnesmodellen](memory.md), [standardbiblioteket](standard.md) och
[status](status.md).

## 1. Lokala lån och lånade vyer

Gör det möjligt att namnge ett läslån till ett fält eller arrayelement och
använda ett array-/textintervall som vy utan ny buffert. Börja med lån som
stannar i samma scope; utöka sedan till returer med ett tydligt samband
mellan indata och returens livslängd. Ägaren får inte flyttas, ersättas eller
muteras på ett motstridigt sätt medan lånet används.

Det tar bort behovet av tillfälliga strängägare för rena läsningar och gör
JSON-, CSV- och HTTP-API:er möjliga utan materialiserade mellanbuffertar.
Verifiera både avvisade livstidsfel och att vyerna behåller originalets
adress utan allokering. Begränsade lån är redan möjliga under anrop;
den här etappen utvidgar samma regel.

## 2. Partiella flyttar och bättre ägda samlingar

Spåra initiering per structfält, så att flera oberoende fält kan flyttas ur
samma ägare och återstående fält städas exakt en gång. Dagens regel
förbrukar hela föräldern vid första fältflytten. Kompilatorns frysta
metadata och workerresultat innehåller fortfarande fältkopior på grund av
den begränsningen.

Utöka sedan arrayelement till structar, payload-enums och andra arrayer,
med samma flytt-, klonings- och cleanupregler som fristående värden.
Det ersätter parallella fältarrayer i bland annat HTTP-servern och
kompilatorn. Lägg till ägd borttagning som flyttar ut ett element utan en
oinitierad lucka; `swap` följt av `pop` fungerar redan när ordningen får
ändras.

Bygg generiska `Map<K, V>` och `Set<T>` ovanpå denna grund. Välj en enkel
modell med statisk specialisering och synliga ägarskapskrav. Nuvarande
`Map` är `string → i64` och `Set` innehåller strängar.

## 3. Automatisk cleanup för externa resurser och typade fel

Låt `@resource`-typer ange cleanup som kompilatorn kör vid scope-slut,
retur, `break`, `continue` och felvägar. Flytt lämnar över ansvaret; lån
skapar inget nytt cleanupansvar. Definiera särskilt vad som får blockera
och hur explicit stängning samverkar med automatisk cleanup. Ett glömt
`close` eller en tidig retur ska inte läcka en socket.

Inför sedan generiska `Result<T, E>` och enkel felpropagering. Samla dagens
blandning av `.ok`/`.error`, enums och statuskoder utan att dölja resursägare
på felvägen. Ett fallibelt konsumtionsanrop måste kunna lämna tillbaka en
resurs som inte togs över.

## Leveransregel

Behåll en gemensam regel: läsa lånar, lagra flyttar, duplicera begärs
uttryckligen. Varje etapp ska användas i kompilatorn och ett verkligt
Hundmat-program och verifieras på de tre nativa målen. Utöka syntaxen först
när en sådan konsument visar vad som faktiskt behövs.
