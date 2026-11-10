# Aktivt mål: ett typat, kompilerat Dunder

Användarens omstyrning 2026-11-10 ersätter den tidigare prioritetsordningen
med självassemblerande frö före användbart språk. Prioritera Dunders riktiga
syntax, statiska typer och program som kompileras och körs. Användaren valde
`:=` för deklaration med härledd typ, `: i32 =` för explicit typ och
oföränderlighet som standard.

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
