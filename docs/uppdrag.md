# Aktivt mål: ett typat, kompilerat Dunder

Användarens omstyrning 2026-11-10 ersätter den tidigare prioritetsordningen
med självassemblerande frö före användbart språk. Prioritera Dunders riktiga
syntax, statiska typer och program som kompileras och körs. Användaren valde
`:=` för deklaration med härledd typ, `: i32 =` för explicit typ och
oföränderlighet som standard.

Användarens senaste filändelseval är `.ddr`. Källfiler, importerarnas
filuppslagning, CLI, manifest, exempel och tester ska använda den; själva
importuttrycken ska fortsatt sakna filändelse.

Filmoduler är implementerade. Användaren kräver importer utan
filändelse: `import math` läser math.ddr. Lägg till explicita exporter och
typkontroll över modulgränser. Dunder-kod ska fortsätta kompileras med egen
backend och bildskrivare utan GCC eller LLVM.

Fortsätt genom program som faktiskt använder språket: rekursion, moduler,
for-loopar, iteratorer, arrayer och arenaallokering. De första dogfood-programmen
och en indexbaserad arena över en fast array finns nu i `dogfood/`.
Nästa minnesmodell ska utvecklas vidare utan att beskriva den fasta bufferten
som färdig heapallokering eller lån.

`dunder test` ska själv vara dogfood. Användaren valde taggade tester och
föreslog kopplade block: `@test { (20) -> 4 } fn div_by_five(x: i32) -> i32 { x / 5 }`.
Stöd vanliga uttryck i argument och förväntade värden samt lokala variabler,
booleska assertions och kontrollflöde i blocket. Det senaste kravet är
struct-uttryck som input, exempelvis `(Point { x: 3, y: 4 }) -> 25`.
Detta är implementerat tillsammans med grundläggande typade structar,
värdekopiering, fältåtkomst och retur med sista uttrycket.

Nästa dogfooding är UTF-8-strängar med inspiration från V. Implementerat:
oföränderlig `string`, `rune`, bytebaserad `.len` och indexering, separata
byte-/rune-vyer, snitt, sammanslagning, jämförelser och strängreturer.
Textfunktioner, textstatistik och en liten UTF-8-tokenizer är skrivna i
Dunder och ingår i testträdet. Beskriv den första strängarenans 16 MiB-gräns
och processlivstid tydligt; kalla den inte färdig ägande-/lånekontroll.

Nästa språksteg är bra stöd för flyttal. `f32` och `f64` är nu implementerade
med IEEE-aritmetik, kontexttypade literaler, explicita konverteringar,
matteoperationer, klassificering, parametrar/returer och samlingar.
Numeriska dogfood-program och en Dunder-skriven decimalformatterare driver
stödet. Beskriv återstående matematikbibliotek och strängparser tydligt.

Nästa steg är ett sys-bibliotek med filsystemsfunktioner för macOS.
`import sys` är nu inbäddat och ger textläsning/-skrivning, append, metadata,
kataloger, rename och borttagning med inspekterbara resultat. Detta driver
också säkra struct- och arrayreturer med lagring hos anroparen. Fortsätt
dogfooda genom riktiga filer i egna testkataloger och dokumentera gränserna:
UTF-8, vanliga filer, 4 MiB per läsning och den befintliga strängarenan.

Användaren bekräftade båda formerna av enums: namngivna heltalsvärden och
varianter med data. Båda är implementerade med `enum`/`pub enum`, kontextuell
`.Variant`, värdekopiering och statiskt uttömmande `match`. Payloads lagras
direkt med en tagg; dispatch använder grenar utan vtables. `sys.read`
dogfoodar `ReadResult { Ok(string), Err(IoError) }` och bevarar okända felkoder.
Koppla inte ihop enums med ett krav på en viss allokerare. Generics, allmänt
allokerings-API och färdig ägar-/lånemodell återstår; se [enumreglerna](enums.md).

Första acceptans: typade funktioner och lokaler, tydliga typfel, explicit
muterbarhet, kontrollflöde, verklig kompilering till native kod och flera
körda exempel med verifierade resultat. Språkets användare skriver Dunder,
och behöver inte skriva ARM64-instruktioner. Dokumentera den implementerade
syntaxen och begränsningarna.

Behåll arkitekturens riktning mot egen kodgenerering, länkning, bildskrivning
och signering. Rust får bootstrapkompilera den första implementationen;
vanlig Dunder-kompilering använder den egna backendvägen. Kalla inte detta
självhosting och gör inga prestandaanspråk utan mätkontraktets bevis.

Arbeta på språkgrenen och bevara fröexperimenten och andra checkoutar.
Ingen push, merge till main eller extern publicering ingår i detta steg.
