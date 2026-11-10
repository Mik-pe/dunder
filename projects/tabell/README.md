# Tabell

Ett fristående CLI-projekt skrivet i Dunder. Tabell grupperar CSV-rader efter
en vald kolumn och summerar heltal i en annan, exempelvis kostnader i ören
per team. Projektet har eget manifest, parser, rapportmodul och tester.

Bygg Dunder i reporoten och kör sedan projektet:

```sh
cargo build --release
cd projects/tabell
../../target/release/dunder run -- expenses.csv team cents
../../target/release/dunder test
../../target/release/dunder build
```

Exemplet skriver samma CSV som [expected.csv](expected.csv):

```csv
group,total
Design,1000
Platform,1000
Örebro,250
```

Den byggda binären kan flyttas och köras utan Dunder eller projektets källfiler:

```sh
./build/x86_64-linux/tabell expenses.csv team cents
../../target/release/dunder build --target arm64-linux -o build/tabell-arm64
```

På macOS är standardkatalogen `build/arm64-macos`; på ARM64/Linux är den
`build/arm64-linux`. `tabell --help` visar kommandots argument.

## CSV-kontrakt

- Första posten är rubriker. Kolumnnamn måste vara unika och får inte vara tomma.
  Namn matchas exakt, med bevarade blanktecken och skiftläge.
- Komma avgränsar fält. Citerade fält får innehålla komma och radbrytningar;
  dubbla citattecken i ett citerat fält betyder ett citattecken.
- LF, CRLF och CR stöds, även i citerade fält. Ett inledande UTF-8-BOM tas bort.
  Unicode, tomma gruppnamn och tomma datafiler med giltig rubrikrad stöds.
- Alla dataposter måste ha samma antal fält som rubriken. Tomma rader är
  vanliga CSV-poster och hoppas inte över.
- Belopp är decimala `i64` med valfritt `+`/`-`, utan decimaltecken,
  tusentalsavskiljare eller omgivande blanktecken. Belopp och varje löpande
  gruppsumma kontrolleras mot i64-gränserna.
- Utdata har kolumnerna `group,total`, med grupper sorterade efter UTF-8-byte.
  Fält citeras vid behov. Sista posten avslutas alltid med LF.
- Fel ger exitkod 1, diagnostik på stderr och ingen CSV på stdout. Felaktigt
  antal argument ger exitkod 2. CSV-fel anger fysisk rad, även efter citerade
  poster med flera rader.

## Gränser och dogfooding

`sys.read_text` begränsar indata till 4 MiB UTF-8. CSV läses en post åt gången
från den inlästa texten. Dunders nuvarande processarenor återvinner inte
enskilda strängar eller dynamiska buffertar; många små fält kan därför nå
minnesgränsen innan filgränsen. Tabell är ännu inget verktyg för stora dataflöden.

Projektet provar punktseparerade modulnamn utan citattecken, mutable borrows,
structreturer med dynamiska fält, UTF-8-bytebuffertar, map, strängsortering,
programargument, fil-I/O och separata stdout/stderr. Parserns och rapportens
taggade tester finns nära funktionerna. Kompilatorns integrationstest
`tests/tabell.rs` jämför dessutom riktiga CLI-körningar med en oberoende referens.
