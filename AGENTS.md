# Utveckling av Dunder

Arbeta på `main` och pusha verifierade ändringar till `origin/main`.
Samordna parallellt arbete i separata filer eller arbetskataloger.

Tillåtna språk i repot är Dunder (`.ddr`) för kompilator, bibliotek, verktyg
och tester, tunt POSIX `sh` enbart för bootstrap-, installations- och
startskript som måste köras innan kompilatorn finns byggd, samt C/assembler
(`.c`/`.S`) enbart för fristående FFI-/ABI-testfixturer och
benchmark-referenser. Lägg inte till kod, skript eller tester i Python,
JavaScript/TypeScript, Rust, Go eller andra språk — om något saknas för att
skriva i Dunder, bygg ut standardbiblioteket eller ett Dunder-paket istället.

Bygg kompilatorn från det frysta fröet med `tools/bootstrap.sh`. Kontrollera
ändrat beteende med den självbyggda kompilatorn, förväntade resultat och
diagnostik. Kör `tools/verify-selfhost.sh` för hela testsviten.

Byggen och tester körs lokalt eller på en uttryckligen vald maskin, utan
Cargo eller GitHub CI. På macOS körs Linuxproven i Kiri med
`tools/verify-selfhost.sh --linux arm64` eller `--linux x86_64`.
Kör prestandamätningar bara på begäran.

`../dunder-hundmat` är Dunders testbädd med riktiga program, bland annat
Tabell, en HTTP-server och grafikexempel. Använd programmen för att hitta
och prova förbättringar i Dunder. Ändringar i Hundmat ska tjäna det syftet.
Lägg generella förbättringar av språket, kompilatorn och standardbiblioteket
här.

Använd syntetiska eller publika testfixturer. Utelämna namn och innehåll
från privata projekt i Dunders kod, dokumentation och commitmeddelanden.
