# Generics som en del av Dunders typsystem

**Designförslag.** Behåll explicita typargument och monomorfisering. Flytta
beslutet om *vilken instans en användning avser* till namnbindningen och
typkontrollen. Då får en typ samma betydelse som fält, funktionsparameter och
typargument. Dagens syntax och exempel finns i [språkbeskrivningen](syntax.md#generiska-deklarationer).

## Var dagens modell brister

Den självbyggda kompilatorn vid `243ca53` godtar en funktion som tar
`(i32, bool,)` och returnerar värdet som `(i32, bool)`. Med samma tupel
inuti en generisk struct avvisar den returen med `type mismatch`.

```dunder
struct Box<T> { value: T }
fn convert(move x: Box<(i32, bool,)>) -> Box<(i32, bool)> { x }
fn main() {
    v := Box<(i32, bool,)> { value: (42, true) }
    print(convert(move v).value.0)
}
```

En annan följd syns i ett program som bara jämför heltal.

```dunder
fn main() {
    Map := 1
    limit := 2
    print(Map < limit && limit > 0)
}
```

Här rapporterar kompilatorn att `Map` behöver två typargument. Byter man
variabelnamn till `count` skriver programmet ut `true`. Båda felen uppstår
i steget som söker efter namn följda av `<`, bygger en instansnyckel av
argumentens text och skriver om programmet före ordinarie typkontroll.
[Specialisering](../selfhost/frontend/generics.ddr) · [Tolkning av typargument](../selfhost/frontend/generic_arguments.ddr)

Tidpunkten för kontrollen påverkar också vilka program som godtas. En oanvänd
generisk funktion kan anropa en obefintlig fri funktion och ändå passera
`dunder check`. När `Box<i32>` väl används kontrolleras däremot alla dess
metodkroppar. En oanropad `count` som läser `self.value.len` stoppar till och
med ett anrop till en annan metod som bara returnerar `42`.

## Namn, typer och identitet

1. **Bind deklarationen en gång.** Varje funktion, struct, enum och metod får
   ett deklarations-ID med sin ägarmodul. En typparameter identifieras av sin
   deklaration och sin plats i parameterlistan. Ett `impl` binder sina
   parametrar till mottagartypens parameterplatser. Importalias leder till
   den exporterade deklarationen. Samma gränssnitt används för moduler och
   för källtext som skickas direkt till kompilatorn.
2. **Tolka typen en gång.** Den ordinarie typparsern får representera även
   öppna typer, exempelvis `Box<[]T>` som en tillämpning av `Box` på en
   array av parameter nummer ett. I ett uttryck slår parsern först upp
   namnet i dess lexikala omgivning. Ett lokalt heltal kan följas av
   jämförelseoperatorn `<`. En bunden generisk funktion kan följas av
   typargument både vid anrop och som funktionsvärde, exempelvis `id<i32>`.
3. **Låt typsystemet bestämma likhet.** Inom ett bygge är nyckeln
   `(deklarations-ID, ordnad följd av kanoniska typ-ID:n)`. För en metod
   ingår mottagarens typargument. Därmed ger två importer av samma `Box`
   och två likvärdiga tupelstavningar samma instans. Två separat deklarerade
   structar förblir olika trots identiska fält. Strukturella typer jämförs
   rekursivt med sina vanliga typregler, inklusive arraylängd, låneslag och
   för funktioner även anropskonvention, effektivt parameterläge och
   returlånets källparameter. Typregistret har redan strukturell uppslagning
   för bland annat funktionstyper. [Typregistret](../selfhost/frontend/frontend_types.ddr)

För exemplet ovan binds `T` i `Box` till samma tupel-ID vid parametern och
returtypen. Kompilatorn reserverar en struct och beräknar dess layout en gång.
För detta räcker en representation av bundna typuttryck och referenser till
deklarationer. Kroppens tokenföljd kan ligga kvar oförändrad. När kroppen
typkontrolleras slås en typparameter upp i instansens tabell över argument.
Källpositionen för varje användning finns då kvar för felmeddelanden.

## Vad kontrolleras när?

| Tidpunkt | Ansvar |
| --- | --- |
| Vid deklarationen | Kontrollera syntax, signaturer, namn, synlighet och uttryck som kan avgöras oberoende av typparametrarna. Ett okänt fritt funktionsnamn är ett fel även i en oanvänd mall. Spara den bundna kroppen och markera operationer som behöver en konkret typ. |
| När en typ används | Sätt in typargumenten i fält och enumvarianter. Kontrollera typens deklarerade krav och beräkna layout, ägarskap och städning med samma regler som för en vanlig deklaration. |
| När en funktion eller metod används | Kontrollera de typberoende operationerna för just denna kropp. För en metod som läser `self.value.len` måste den konkreta fälttypen stödja `len`. Övriga metoder kan användas oberoende. Ett funktionsvärde räknas också som användning. |

Den konkreta signaturen går genom Dunders ordinarie regler för implicit lån,
`move` och returlån. Privata namn och rättigheter hör till definitionsmodulen.
Ett fel som beror på ett typargument ska visa operationen i definitionen
och den användning som valde argumentet, med instanskedjan när den behövs.

**Samlingarnas kontrakt.** `Map<K, V>` och `Set<K>` lovar att nycklar både
kan hashas och jämföras. Det kravet gäller redan vid exempelvis
`Map<K, V>::new()`. Lägg det på typdeklarationen som ett strukturellt
krav på de två publika metodsignaturerna. Samma deklarationsmöjlighet ska
vara tillgänglig för användartyper. Kontrollera lagringsbarhet där ett
värde lagras. En enskild metod kan därutöver kräva exempelvis kloning.
Detta behåller samlingarnas kontrakt när metodkroppar börjar kontrolleras
var för sig. För `?` behöver kompilatorn känna igen standardbibliotekets
exporterade `Result` via dess deklarations-ID. Andra enums med samma namn
eller varianter följer vanliga enumregler. [Samlingskontrakt](standard.md#generiska-samlingar) · [Dagens kontraktsprov](../tools/test_collections.ddr)

## Från bunden deklaration till körbar kod

Dunder känner till de konkreta typerna vid bygge. Monomorfisering ger då
direkta anrop och ordinarie layout och städning på både native och Wasm.
Kostnaden är att flera kombinationer av typargument kan ge fler
maskinkodskroppar. Källspråkets regler för typidentitet är desamma om en
framtida kodgenerator kan dela maskinkod mellan vissa instanser.

Samordnaren håller en arbetskö för typer och funktioner. Vid första begäran
reserverar den instansens ID och gör signaturen tillgänglig innan kroppen
kontrolleras. När en kropp typkontrolleras läggs de konkreta typer och
funktioner som kroppen refererar till i kön. Ett rekursivt anrop med samma
nyckel återanvänder signaturen.
En kedja av 70 *olika* funktioner är ändlig och ska godtas. Dagens kompilator
stoppar den när upptäcktsdjupet passerar 64. En värdetyp med oändlig layout
ska ge layoutfel. Ett anrop som hela tiden skapar nya typer, som
`grow<T>()` som anropar `grow<[]T>()`, ska ge ett kontrollerat fel som
visar hur argumentet växer. En övre resursgräns för antalet upptäckta
instanser skyddar även fall där tillväxten är svår att känna igen.

Samordnaren färdigställer de gemensamma identiteterna i bestämd ordning
och fryser dem före parallell kodgenerering. Arbetarnas lokala typnummer
kan då stanna inom varje kropp. Beständiga cachenycklar måste kunna
återskapas från paket, modul, deklaration och typargument. Återanvändning
av maskinkod beror även på använd mallkropp, valda metoder, ABI, mål och
kompilatorversion. [Parallell kompilering](tasks.md#kompilatorn-som-taskkonsument) · [Funktionscachen](incremental.md)

## Införande och acceptans

1. Lägg exemplen ovan i den befintliga generics-sviten. Lägg till prov för
   två importalias till samma typ, två olika modulers publika typer med
   samma namn och fält samt funktionstyper där bara returlånets källparameter skiljer.
2. Inför bundna deklarationer och öppna typuttryck i frontenden. Anslut
   både källtext-API:t och modulladdaren. Byt till typade instansnycklar
   för egna deklarationer och standardbiblioteket i samma steg.
3. Flytta typkraven till deklarationerna och inför kontroll per metod och
   arbetskön. Prova oanvända mallar, fel i anropade metoder, vanlig
   rekursion, den ändliga kedjan och växande typargument. Fel ska peka
   tillbaka på användarens ursprungliga källrader.
4. Ta bort tokenexpansionen, den separata typargumentparsern och
   namnkontrollerna för samlingarna. Bygg från fröet och kör hela
   självhostsviten. Bygg generics-proven för alla fyra mål, kör dem på
   värden och Wasm och jämför seriella, parallella och inkrementella
   nativebilder byte för byte för samma mål och källkod.
