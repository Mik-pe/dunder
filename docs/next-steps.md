# Nästa steg för Dunder

## Verifiera Windows som kompilatorvärd

0.9.0 innehåller en korsbyggd x86-64-kompilator för Windows. Kör följande på en riktig Windowsinstallation och dokumentera Windowsversion, källrevision och resultat.

1. Hämta den publicerade exe-filen, kontrollera SHA-256 och bygg kompilatorn två gånger med ett jobb. Jämför filerna byte för byte. Upprepa med flera jobb.
2. Kör `check`, `build`, `run`, `test` och språkservern i ett projekt med mellanslag och svenska tecken i sökvägen. Använd ett lokalt paketberoende. Prova miljö, processavbrott, standardfel och callback över C-gränsen.
3. Installera och uppgradera i en tom katalog. Prova även bytet medan den installerade filen används. [Installation och distribution](releases.md) · [Plattformskontrakt](targets.md)

Privata filer på Windows behöver ett kontrakt för Windows åtkomstkontrollistor.
Verifiera åtkomsten som ett annat användarkonto och utred vilka garantier ett
beständigt katalogbyte kan ge när datorn startas om. [Filrättigheter och synkning](sys.md)

## Förbättra diagnostik vid generiska lån

Om en callback till `alloc.with_bytes` försöker returnera ett lån av blocket avvisas det med E4003. Låt diagnostiken visa användarens anrop tillsammans med den specialiserade signaturen. [Bytevyer och ägarskap](memory.md)

## Jämför lånade bytevyer utan kopiering

`del := &data[start..slut]` kan indexeras när `data` är en lånad bytearray.
Anropet `bytes.equal(&facit, &del)` ger däremot E4005. Samma fel uppstår om
intervallet skrivs direkt i anropet. `bytes.equal_ranges(&facit, 0, facit.len,
data, start, slut)` fungerar utan att kopiera. Utred hur `bytes.equal` kan
jämföra två vyer och samtidigt garantera att deras adresser bara används
under anropet. Prova även tomma vyer och olika långa intervall.

## Följ upp mätningar för SIMD

Arbetslasten i [layoutmätningen](../benchmarks/layout/2026-11-22/README.md) har ett oberoende numeriskt facit för både punktposter och separata kolumner. ARM64-serien kördes på fysisk maskin. Upprepa x86-64-serien på fysisk hårdvara inför ett beslut om vektorisering eller instruktionsschemaläggning. Behåll låneprovet för kolonntillväxt vid en framtida representationsändring.
