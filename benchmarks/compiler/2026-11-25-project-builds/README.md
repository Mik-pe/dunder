# Byggtider på Apple M5 den 25 november 2026

**325 666 rader/s vid p95 i autoläge** på det blandade korpuset. Målet är 100 000 rader/s. Med en arbetare blev resultatet 77 174 rader/s. Ett oförändrat bygge klarade 50 ms i autoläge. Ändringen av en privat funktionskropp överskred gränsen på 100 ms med p95 104,334 ms i autoläge och 120,008 ms med en arbetare. För den privata inlinade funktionen var p95 131,685 ms med auto och 214,289 ms med en arbetare.

## Mål på det blandade korpuset

| Byggfall | Auto median, ms | Auto p95, ms | 1 jobb median, ms | 1 jobb p95, ms | Högsta RSS auto / 1, MiB | Mål vid p95 |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Rent | 364,542 | 378,265 | 1 584,043 | 1 596,238 | 75,1 / 75,7 | ≥ 100 000 rader/s |
| Oförändrat | 6,042 | 6,481 | 6,056 | 6,316 | 5,5 / 5,5 | ≤ 50 ms |
| Privat funktionskropp | 101,502 | 104,334 | 117,442 | 120,008 | 61,5 / 69,4 | ≤ 100 ms |
| Privat inlinad funktion | 126,004 | 131,685 | 200,012 | 214,289 | 73,7 / 69,8 | ≤ 100 ms |
| Delad postlayout | 381,303 | 400,186 | 1 599,160 | 1 615,607 | 76,9 / 77,5 | Identisk med rent bygge |
| Rent efter layoutändring | 363,691 | 372,843 | 1 585,129 | 1 592,178 | 75,1 / 75,7 | Kontroll |

Korpuset har 123 188 tokenbärande rader i 22 moduler, 2 793 880 byte, 680 124 token och 1 042 funktioner. Det genererade programmet gör interna och importerade anrop, skickar lånade arrayer och använder gemensamma poster, loopar och villkor. Det rena programmet ska skriva `474148`. Varje mätning efter en källändring kontrollerade den ändrade summan. [Korpusgenerator](../../../tools/performance_mixed.ddr) · [Mål och mätmetod](../../../docs/prestanda.md)

Den första privata ändringen påverkade en slutberäkning. Den andra ändrade en funktion som inlinas hos 50 anropare. En separat körning av samma projektbyggen med en räknare i kompilatorn visade 1 respektive 51 ombyggda funktioner. Programmen gav då 474 149 respektive 474 199. Omordning av två fält i den delade posttypen byggde om 1 042 funktioner och gav samma binär byte för byte som ett rent bygge från de ändrade källorna. Vid oförändrat bygge godkändes projektkvittot innan funktionskompileringen startade. Alla räknarprov gav samma binär som ordinarie kompilator. [Antal återbrukade och ombyggda funktioner per fall](function-counts.json)

På föregående kompilatorrevision `7a989c664203` gav samma korpus i autoläge p95 121,218 ms för den privata funktionskroppen och 145,744 ms för den inlinade funktionen. Referensserien hade 30 försök i varje fall och kördes tidigare samma dag. [Råprover för referensen](mixed-reference.csv)

## Projekt och självbygge

| Projekt | Fall | Auto median, ms | Auto p95, ms | 1 jobb median, ms | 1 jobb p95, ms | Högsta RSS auto / 1, MiB |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Tabell | Rent | 33,546 | 34,053 | 41,010 | 41,315 | 22,3 / 12,7 |
| Tabell | Oförändrat | 2,508 | 2,648 | 2,423 | 2,655 | 2,7 / 2,6 |
| Tabell | Privat funktionskropp | 37,388 | 38,393 | 40,366 | 41,022 | 22,0 / 12,7 |
| Tabell | Rent kontrollbygge efter ändring | 47,280 | 49,346 | 55,295 | 56,321 | 22,3 / 12,7 |
| HTTP-server | Rent | 97,497 | 98,932 | 126,110 | 127,346 | 56,5 / 26,3 |
| HTTP-server | Oförändrat | 2,936 | 3,020 | 2,885 | 2,985 | 2,8 / 2,8 |
| HTTP-server | Privat funktionskropp | 102,006 | 104,704 | 122,439 | 124,745 | 54,3 / 26,4 |
| HTTP-server | Rent kontrollbygge efter ändring | 107,232 | 108,614 | 136,304 | 141,192 | 56,6 / 26,3 |
| TUI-räknaren | Rent | 355,504 | 360,511 | 509,791 | 516,252 | 73,7 / 43,1 |
| TUI-räknaren | Oförändrat | 3,460 | 3,555 | 3,500 | 3,581 | 2,9 / 2,9 |
| TUI-räknaren | Privat funktionskropp | 380,206 | 384,254 | 501,545 | 507,194 | 77,7 / 43,0 |
| TUI-räknaren | Rent kontrollbygge efter ändring | 386,430 | 388,561 | 539,233 | 544,962 | 73,9 / 43,2 |
| Kompilatorn | Rent | 905,990 | 925,260 | 2 477,755 | 2 495,290 | 391,7 / 185,3 |

Tabell, HTTP-server och TUI-räknaren klarade 50 ms för oförändrat bygge i autoläge. Tabell klarade 100 ms för en privat ändring i autoläge. HTTP-server och TUI-räknaren överskred 100 ms för en privat ändring i autoläge.

Tabells sex enhetstester och CLI-prov godkändes. Den byggda binären gav den förväntade CSV-filen byte för byte. HTTP-serverns fyra enhetstester och nätprov godkändes, inklusive hälsokontroll och samtidiga anrop. TUI:s 118 enhetstester, rörelseprovet och terminalprovet godkändes. Räknaren visade `0 → 1 → 0` vid piltangenter och avslutade med `q`. Vid varje privat redigering byttes en sträng ut. Tabell fick en ny rubrik på summakolumnen, HTTP-servern ett nytt svar på `GET /healthz` och räknaren en ny etikett före siffran. Provet läste tillbaka `QA1000` till `QA1029` för auto och `QA2000` till `QA2029` för en arbetare. Varje ändrad projektbinär jämfördes byte för byte med ett separat rent kontrollbygge. Samma projektsviter passerade på Linux ARM64, där HTTP-provet gav 128 svar för åtta samtidiga klienter. Tidsmätningarna gjordes på macOS. Självbygget gav samma SHA-256 som den körande kompilatorn.

## Ingående källor

| Projekt | Moduler | Tokenbärande rader | Källbyte | Token | fn-token | Emitterade funktioner | Binär, byte |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Blandat korpus | 22 | 123 188 | 2 793 880 | 680 124 | 1 042 | 1 042 | 2 889 344 |
| Tabell | 16 | 2 628 | 103 820 | 27 562 | 329 | 50 | 49 376 |
| HTTP-server | 31 | 9 035 | 367 728 | 88 317 | 1 038 | 159 | 164 288 |
| TUI-räknaren | 41 | 10 532 | 516 687 | 117 092 | 1 320 | 279 | 246 368 |
| Kompilatorn | 197 | 74 287 | 4 536 921 | 1 009 129 | 5 096 | 3 828 | 5 302 496 |

Varje laddad modul räknades en gång. Tomma rader och rena kommentarer utelämnades. Kolumnen `fn-token` räknar även funktionstyper och anonyma funktioner. Kolumnen intill anger hur många funktioner kompilatorn faktiskt genererade vid det rena bygget. Tabell och HTTP hämtades från Hundmat på `b08de816`. Räknaren hämtades från Dunder TUI på `e281b84f`. [Hundmat](https://github.com/Mik-pe/dunder-hundmat/tree/b08de81602a5a70bd5cebd19f88b6a7763946cb8) · [Räknaren](https://github.com/Mik-pe/dunder-tui/tree/e281b84f6943a96e004a2160c4ea2835c8ce08f8/examples/counter)

## Verifiering av kompilatorn

Den självbyggda kompilatorn klarade 86 av 86 testsviter på macOS ARM64, 81 av 81 på Linux ARM64 och 81 av 81 på Linux x86-64. Det korsbyggda Windows-programmet och fyra självbyggen över två generationer med en respektive tre arbetare gav samma PE-fil under Wine 8, SHA-256 `12f276e7c22c9f7aeeb51043128a825b8bdec0a4562109def780d376df57fb86`. De publicerade byggtiderna gäller macOS.

## Genomförande

Mätningarna kördes på Mac17,2 med Apple M5, 24 GiB och macOS 27.2.1 (26B131). Auto valde 10 byggarbetare. En arbetare valdes med `--jobs 1`. Den självbyggda kompilatorn på revision `8c82aa0ecd4c` hade SHA-256 `ea2b51feb3f3127169eb57f445ee2ed2ddaa074b84d5972f5e8053709ddf5bed`. Det blandade korpusets SHA-256 var `10fbdedb002723ca4fac9a5aa6923f881e6f9c2c8d0784821f8e7f5800bbc5a3`. [Kompilatorkällor](https://github.com/Mik-pe/dunder/tree/8c82aa0ecd4c93e861fc7ca154dc81ddf0ee23d9)

Varje kombination har 30 försök med varm OS-filcache. Datorn hade vanlig bakgrundslast och annat byggarbete, särskilt under självbygget. Alla observationer ingår. Tiden mäts i föräldraprocessen före start av `dunder build -o <fil>` fram till avslutad process med signerad binär på disk. RSS kommer från `wait4` för kompilatorprocessen. Medianen är medelvärdet av det 15:e och 16:e sorterade provet. P95 är det 29:e provet. Första bygget efter maskinomstart har ännu ingen mätserie.

För rena byggen fick varje körning tom kompileringscache och utdatakatalog. Efterföljande oförändrat bygge använde samma kommando, källa, cache och utfil. Auto gick först i udda rundor och en arbetare först i jämna rundor. Självbygget använde `dunder build selfhost/main.ddr -o <fil>`. Projektredigeringarna kördes i en separat serie. Projektordningen roterade, jobblägena växlade och varje projekt och jobbläge behöll sin egen cache mellan nya redigeringar. Varje rent kontrollbygge fick en tom cache och en ny utfil.

Faserna mättes i separata rena körningar med `--timings=json`. De mäter tid inne i kompilatorn. Kontrollsumman för de laddade källorna beräknades på modulerna i seriell laddningsordning, med längden före varje moduls innehåll. [Faser per projekt och jobbantal](phases.csv) · [Källor och binärer](corpora.csv) · [Samtliga tid- och minnesprov](samples.csv) · [Exakta medianer, p95 och högsta RSS](summary.csv) · [Byggverktyg](../../../tools/performance_project.ddr)
