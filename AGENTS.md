# Utveckling av Dunder

Arbeta på `main` och pusha verifierade ändringar till `origin/main`.
Samordna parallellt arbete i separata filer eller arbetskataloger.

Använd Dunder för nya skript och när du ersätter befintliga Python-skript.

Bygg kompilatorn från det frysta fröet med `tools/bootstrap.sh`. Kontrollera
ändrat beteende med den självbyggda kompilatorn, förväntade resultat och
diagnostik. Kör `tools/verify-selfhost.sh` för hela testsviten.

Byggen och tester körs lokalt eller på en uttryckligen vald maskin, utan
Cargo eller GitHub CI. Kör prestandamätningar bara på begäran.

Fristående program, grafikbindningar, Tabell och HTTP-servern finns i
`../dunder-hundmat`. Lägg generella språk- och kompilatorförbättringar här.
