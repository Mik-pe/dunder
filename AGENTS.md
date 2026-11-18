# Utveckling av Dunder

Arbeta på `main` och pusha verifierade ändringar till `origin/main`.
Samordna parallellt arbete i separata filer eller arbetskataloger.

Använd Dunder för nya skript och när du ersätter befintliga Python-skript.

Bygg kompilatorn från det frysta fröet med `tools/bootstrap.sh`. Kontrollera
ändrat beteende med den självbyggda kompilatorn, förväntade resultat och
diagnostik. Kör `tools/verify-selfhost.sh` för hela testsviten.

Byggen och tester körs lokalt eller på en uttryckligen vald maskin, utan
Cargo eller GitHub CI. Kör prestandamätningar bara på begäran.

`../dunder-hundmat` är Dunders testbädd med riktiga program, bland annat
Tabell, en HTTP-server och grafikexempel. Använd programmen för att hitta
och prova förbättringar i Dunder. Ändringar i Hundmat ska tjäna det syftet.
Lägg generella förbättringar av språket, kompilatorn och standardbiblioteket
här.
