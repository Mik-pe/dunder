# Aktivt mål: ett typat, kompilerat Dunder

Användarens omstyrning 2026-11-10 ersätter den tidigare prioritetsordningen
med självassemblerande frö före användbart språk. Prioritera Dunders riktiga
syntax, statiska typer och program som kompileras och körs. Användaren valde
`:=` för deklaration med härledd typ, `: i32 =` för explicit typ och
oföränderlighet som standard.

Filmoduler är implementerade. Användaren kräver importer utan
filändelse: `import math` läser math.dn. Lägg till explicita exporter och
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
