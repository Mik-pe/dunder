# Dunder ⚡

Dunder ska vara ett kompilerat systemspråk med förutsägbar minneslayout, minnessäker kod utan garbage collector och en egen verktygskedja. Första plattformen är ARM64 på macOS.

Vi vill kunna skriva `dunder build`, få en optimerad körbar fil och fortsätta arbeta direkt. Målet är minst **100 000 källrader per sekund** från ren byggkatalog till färdig binär. Koden som byggs med de inställningarna ska också kunna mäta sig med optimerad C och C++.

Projektet är i designfas. Syntaxen är öppen. Börja med [arkitekturen och byggordningen](docs/arkitektur.md). [Mätkontraktet](docs/prestanda.md) anger när vi får säga att vi nått målen.
