# Nästa steg för Dunder

## Närmast

1. **Visa var ett program avbröts.** På macOS ger heltalsspill status 101 och ett ogiltigt arrayindex status 103. Båda lämnar standardfel tomt. Låt ett program byggt med `-g` skriva operation, källfil och rad till standardfel. Prova två felställen i samma funktion och kontrollera att vardera anger rätt rad och behåller sin statuskod. [Körfel och minnesmodell](memory.md)
2. **Ge UTF-8-avkodning ett typat fel direkt.** `std.bytes.decode` och `decode_range` returnerar `sys.TextResult`. Låt avkodningen användas direkt med `?` och ge skilda fel för ogiltig UTF-8 och ogiltigt intervall. Migrera befintliga anrop i kompilatorn och paketen. Prova tom indata och giltig text med flera byte per tecken. [Bytefunktionerna](../stdlib/bytes.ddr)
3. **Fyll luckorna i `std.math`.** Program för färgomvandling och exponentiellt avtagande har egna approximationer. Lägg till exponentialfunktion, naturlig logaritm och potens för `f64`. Ange noggrannhet och beteende vid domängränser. Jämför med oberoende referensvärden på nativa mål och Wasm, även för noll, NaN och oändlighet. [Matematik](standard.md#matematik)

## Språk och verktyg

- **Signerade 16-bitarsvärden.** Både `[]i16` och `Result<[]i16, E>` avvisas med `unknown type 'i16'`. Ett ljudprogram lagrar därför 16-bitars sampel i `[]i32`. Prova att värdena −32768 och 32767 ryms i två byte per element och behåller tecknet efter indexering, konvertering till `i32` och retur genom `Result`. [Heltal och datalayout](ffi.md)
- **Härled typen för en fristående tuppel.** `p := (1.0, 2.0)` kräver fortfarande en förväntad tuppeltyp. Låt komponenterna bestämma typen när de själva har entydiga typer. Prova även en tuppel med en ägd sträng och diagnostik för en komponent vars typ saknas. [Flervärdesreturer](syntax.md#flervärdesreturer)
- **Läs ett värde före uppdatering av samma tabell.** `antal := m.at(k)` följt av `m.put(k, antal + 1)` avvisas för en `Map<string, i32>`. Utred hur anropet kan använda det avlästa heltalet när uppdateringen börjar. Prova även att ett lån som läses efter en möjlig omhashning fortfarande avvisas. [Lån vid tabelluppslag](standard.md#generiska-samlingar)
- **Windows som kompilatorvärd.** Kör först korskompilerade `.exe`-filer på Windows x86-64 och verifiera Win64-FFI med callbacks. Porta sedan OS-lagret för två byteidentiska seriella självbyggen och fungerande `check`, `build`, `run` och `test` med mellanslag och svenska tecken i sökvägarna. Därefter följer parallell kompilering, språkserver, paket och distribution. [Målplattformar](targets.md)
- **Kodåtgärder och inkrementell analys i språkservern.** Börja med en kodåtgärd för saknat `mut`. Prova att ändringen tar bort felet och att en senare ändring i en import uppdaterar diagnostiken i berörda moduler. [Språkservern](../selfhost/driver/lsp.ddr)

## Biblioteksgränser

- **Skilj operativsystemets slumpkälla från kryptobibliotekens algoritmer.** Program som bara använder `crypto.random` importerar i dag samma modul som bindningarna till Argon2 och OpenSSL. Pröva en uppdelning med ett program som enbart behöver slumpbyte och ett som behöver en extern algoritm. Kontrollera beroenden, kapabiliteter och fel när ett systembibliotek saknas. [Kryptografi](crypto.md)
- **Förtydliga ägarens livstid vid FFI.** Ett grafikprogram behövde behålla en egen kopia av ett buffertargument. Orsaken är öppen. Granska hur API:t uttrycker giltighet under anropet och vem som äger data om ett bibliotek behöver använda dem senare. [C-FFI](ffi.md)

## Optimering

SIMD, autovektorisering och målspecifik schemaläggning för ARM64 och x86-64 behöver en representativ arbetslast med känt resultat. Jämför byggtid och körprestanda med samma kompilatorrevision när en mätning beställs. För kolumnlagring är jämförelsen en array per fält. En härledd representation behöver dessutom bevara lånereglerna när en kolumn växer. [Mätmetod och mål](prestanda.md)
