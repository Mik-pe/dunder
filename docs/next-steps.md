# Nästa steg för Dunder

## Lagra skalära poster sammanhängande i ägda arrayer

Låt en ägd array av poster med enbart skalära fält lagra elementen direkt, som en scratchvy redan kan göra. Med 65 536 insättningar av ett kolumnnummer och en koefficient gav Dunder 0.12.1 och `90573fc` 65 539 allokeringsanrop mot fem med två talarrayer. Prova summan 58 556 416 och att en klon kan växa till 257 poster och ändras via ett muterbart lån medan originalet behåller sitt värde. [Minimiprov](https://github.com/Mik-pe/dunder-tui/blob/7b0a4a5e1cf1f640ba6785645eeec73e272cbe3b/benchmarks/language/sparse_terms.ddr) · [Mätning i layoutlösaren](https://github.com/Mik-pe/dunder-tui/blob/7b0a4a5e1cf1f640ba6785645eeec73e272cbe3b/benchmarks/panels.md)

## Låna text från en strängkonstant

I 0.12.1 ger `view := LABEL.as_str()` E4003 när `LABEL` är en modulkonstant av typen `string`. Låt den lokala vyn läsa från konstantens lagring direkt. Med en namngiven mellanvariabel fungerar redan samma läsning. Kontrollera längd och byte med båda skrivsätten och behåll livstidskontrollen för tillfälliga ägda strängar. [Minimiprov](https://github.com/Mik-pe/dunder-tui/blob/81d9c4460bafe84ae1d3eff38b76ed4360070dcf/tests/dogfood/compiler/constant_string_view.ddr)

## Härled arraylikhet från en generisk anropare

I 0.12.1 ger en jämförelse mellan två konkreta `[]i64` E3004 när hjälpfunktionen anropas från `page_matches<T: Eq>`. Både `==` och `std.array.equal<i64>` träffar felet. Låt båda formerna klara lika och olika arrayer precis som med en konkret anropare. [Minimiprov](https://github.com/Mik-pe/dunder-tui/blob/af416d9b12c9f15e9d31504bf32668e0b462473d/tests/dogfood/compiler/array_equality_from_generic.ddr)

## Exportera samma typ från två moduler

I 0.12.1 ger `pub type Clock = time.MonotonicClock` E2001. Lägg till publika typalias så att en enum kan flyttas utan dubbla deklarationer och konverteringar. Prova att båda modulvägarna fungerar i typsignaturer, konstruktorer och mönster och att ett värde kan ges direkt till en funktion som tar den andra vägen. [Minimiprov](https://github.com/Mik-pe/dunder-tui/blob/f12c0e50108c26dd99bbf099e049c95d91288b50/tests/dogfood/compiler/public_type_alias.ddr)

## Flytta en returpost som lånar från mottagaren

I 0.12.1 fungerar `return result` för en markör som lånar från `self`. `return move result` ger E4003. Bevara lånets ursprung genom flytten och avvisa returer som lånar från en lokal ägare. För 8 192 markörer gav direkt konstruktion 8 193 postkloner, en fabriksmetod 16 385 och ytterligare en retur 24 577. Mät på nytt med kontrollsumman 598 016. [Kompilatorprov](https://github.com/Mik-pe/dunder-tui/blob/991198e1c1808c70c3f8145ce71ded91d47514b2/tests/dogfood/compiler/return_moved_cursor.ddr) · [Allokeringsprov](https://github.com/Mik-pe/dunder-tui/blob/991198e1c1808c70c3f8145ce71ded91d47514b2/benchmarks/language/borrowed_cursor.ddr)

## Hasha en lånad strängvy

Låt `vy.hash()` för `&str` ge samma värde som för en ägd sträng med samma UTF-8-byte. I 0.12.1 får anropet E2003. Att hasha 8 192 strängvyer med `vy.to_string().hash()` gav 8 195 allokeringsanrop, mot tre med `Hasher.write_bytes(vy.as_bytes())` och `finish()`. Prova tom vy och delsträngar med två-, tre- och fyrbyteskalärer. [Kompilatorprov](https://github.com/Mik-pe/dunder-tui/blob/fa3e99a60c8d3a4aa611408d852b0a5b5e58b62a/tests/dogfood/compiler/hash_text_view.ddr) · [Mätprogram](https://github.com/Mik-pe/dunder-tui/blob/fa3e99a60c8d3a4aa611408d852b0a5b5e58b62a/benchmarks/language/view_hash.ddr)

## Dimensionera korta arrayliteraler efter innehållet

I 0.12.1 reserverade 16 384 dynamiska arrayer byggda med `[i]` plats för 131 072 heltal. En textrad med ett spann fick också åtta platser. Låt literaler med känt antal element börja med mindre kapacitet. Mät högsta RSS för korta rader och tiden för upprepad `push`. Kontrollera att en klon kan växa till 1 025 värden medan originalet behåller sitt enda värde. [Minimiprov](https://github.com/Mik-pe/dunder-tui/blob/73e4121202b9791bc9d05241e84c757749bd0f31/benchmarks/language/short_arrays.ddr) · [Minnesmätning](https://github.com/Mik-pe/dunder-tui/blob/73e4121202b9791bc9d05241e84c757749bd0f31/benchmarks/text-storage.md)

## Ändra en enumvariants innehåll via ett exklusivt lån

Stöd `match &mut pages[index]` så att en arm kan ändra ett fält i den aktiva varianten. I 0.12.1 ger uttrycket E3006 och en skrivning via `match &pages[index]` ger E4002. Prova att den tomma varianten bevaras och att `Ready(16)` blir `Ready(17)` i samma array. Behåll skrivskyddet för delade lån. [Minimiprov](https://github.com/Mik-pe/dunder-tui/blob/de80ef98ba4f7b96fcb7a9a332356db46fc74d7b/tests/dogfood/compiler/match_mutable_payload.ddr)

## Skriv osignerade decimaltal i bytebuffertar

Lägg en `u64`-motsvarighet till `append_decimal` i `std.bytes` och låt `std.json.write_u64` använda den. I 0.12.1 gav 7 000 värden via `value.str()` och `append_utf8` 7 007 allokeringsanrop mot sju med JSON-skrivaren i samma förallokerade buffert. Kontrollera att prefixet bevaras och att talen 0, 2⁶³ och 2⁶⁴−1 skrivs exakt. [Minimiprov](https://github.com/Mik-pe/dunder-tui/blob/18f18243d01ae934861c98fa4fd2d43646a0e5da/benchmarks/language/unsigned_text.ddr)

## Återanvänd lagringen vid kopiering mellan fasta arrayer

I 0.12.1 gav 10 001 tilldelningar med `dst.bounds = src.bounds.clone()` för `[f64;2]` 20 011 allokeringsanrop. Med elementvis kopiering blev det nio. Låt kompilatorn återanvända målets lagring när den ägs exklusivt. Kontrollera summan 100 060 000, att källan kan ändras efter kopieringen och att en sparad kopia behåller sitt värde. [Minimiprov](https://github.com/Mik-pe/dunder-tui/blob/9aee09bb692c96999b99565ff35068640c465388/benchmarks/language/fixed_bounds.ddr)

## Tillåt lånade vyer i synkrona callbacks

I 0.12.1 fungerar en post med ett lånat arrayfält som funktionsargument. Samma post fångad med `fn[&samples]` ger E4003 även när callbacken anropas direkt. Följ ägarens livstid genom posten så att provet kan läsa 17. Behåll fel när en closure kan överleva ägaren. [Minimiprov](https://github.com/Mik-pe/dunder-tui/blob/09e3017de5467b418361b4f5729438e36d61e196/tests/dogfood/compiler/capture_borrowed_shape.ddr)

## Packa upp iteratorns koordinatpar utan allokering

I 0.12.1 gav både `for (x,y) in pairs` och `match pairs.next()` 506 881 allokeringsanrop över 253 440 par. En markör med två heltalsfält gav ett. Optimera direkt uppackning av `Option<(i32,i32)>`. Kontrollera summan 14 952 960 och att ett sparat par behåller sina koordinater efter nästa steg. [Minimiprov](https://github.com/Mik-pe/dunder-tui/blob/c97df61f7ef791637ca96553da3e58be946b47f4/benchmarks/language/iterate_pairs.ddr)

## Återanvänd enumvarianter utan fält

Vid 253 440 skrivningar av fyra fältlösa varianter i en enum som även kan bära RGB-data gav `match` 126 736 allokeringsanrop. Uppslag i en färdig tabell gav 16 på både macOS och Linux ARM64. Återanvänd de fältlösa värdena och behåll ägarskapet för varianter med data. [Mätprogram](https://github.com/Mik-pe/dunder-tui/blob/c97df61f7ef791637ca96553da3e58be946b47f4/benchmarks/language/enum_table.ddr)

## Skilj strängdata från syntax i konstanta arrayer

I 0.12.1 får `["[", "]", ","]` E3007. Kontrollera tokentypen när konstantutvärderaren letar efter arrayens slut. Prova `"]"` först, i mitten och sist samt tom array och avslutande komma. [Minimiprov](https://github.com/Mik-pe/dunder-tui/blob/cb25b80d1d80270aa0311449c6ab4fe30f52d20f/tests/dogfood/compiler/constant_bracket_table.ddr)

## Låt en array växa med en konstruktor

Lägg till `std.array.resize_with` som anropar en funktion en gång per nytt element och noll gånger vid krympning. För 253 440 celler på macOS och Linux ARM64 gav direkt konstruktion 507 103 allokeringsanrop mot 760 543 med `resize`. Verifiera att befintliga element bevaras och att konstruktorn kan lämna ett ägt värde utan krav på `Clone`. [Mätprogram](https://github.com/Mik-pe/dunder-tui/blob/cb25b80d1d80270aa0311449c6ab4fe30f52d20f/benchmarks/language/fresh_cells.ddr)

## Minska kostnaden för enkla metodanrop på lånade enumvärden

I 0.12.1 tog 8 192 variantkontroller via `is_reset(&self)` 9 µs på macOS ARM64 och 9,7 µs på Linux ARM64. Samma `match` direkt i loopen tog 5 respektive 5,5 µs. Låt optimeraren lägga in ett enkelt varianttest vid anropet. Kontrollera resultatet 450 560, bildtiden i en färgad buffert samt effekten på byggtid och kodstorlek. [Körbart prov](https://github.com/Mik-pe/dunder-tui/blob/42c65d8fcaa4edb6cdc1ba86fbe08da81182e185/benchmarks/language/enum_predicate.ddr) · [Mätningar](https://github.com/Mik-pe/dunder-tui/blob/42c65d8fcaa4edb6cdc1ba86fbe08da81182e185/benchmarks/results/enum-predicate-0.12.1.tsv)

## Låt ett returvärde låna från två parametrar

I 0.12.1 kan en lokal post låna två ägare. En funktion som bygger samma post från två parametrar får E4003 när den anger `from left`. Ge returkontraktet stöd för båda ursprungen så att exempelvis en diffiterator kan låna båda buffertarna direkt vid anropet. Prova att läsa båda värdena och att en mutation av vardera ägaren avvisas medan paret används. [Returprov](https://github.com/Mik-pe/dunder-tui/blob/bf2ccc57aa919cd85cb1ad418c6b1364632dbce9/tests/dogfood/compiler/return_two_owners.ddr) · [Fungerande lokalt par](https://github.com/Mik-pe/dunder-tui/blob/bf2ccc57aa919cd85cb1ad418c6b1364632dbce9/tests/dogfood/compiler/paired_views.ddr)

## Läs en Unicode-skalär vid ett byteindex i `&str`

Ge lånad text ett allokeringsfritt uppslag som lämnar skalären och nästa byteindex. Prova alla fyra UTF-8-längder samt index vid strängslut och mitt i en skalär. I 0.12.1 tog 8 192 uppslag på macOS ARM64 103 µs via `std.bytes.utf8.decode_prefix` och 39 µs direkt ur textvyn. På Linux ARM64 tog de 107 och 39 µs. Över hela provet begärde processerna 227 480 respektive 153 allokeringar och gav samma summa. Behåll prefixavkodarens svar för ofullständiga och ogiltiga byte. [Körbart minimiprov](https://github.com/Mik-pe/dunder-tui/blob/5fb758089574767296a58bc84442ea2c577be3cc/benchmarks/language/string_scalar.ddr)

## Härled elementtypen från fasta arrayargument

I 0.12.1 ger både `total(&[a, b])` med parametern `&[]T` och `Menu::labels([a, b])` med `move items: []T` E3003. Härled `T` från arrayens element. Med explicit elementtyp ger låneprovet summan 5 och det ägda provet strängarna `Norr` och `Söder` i ordning. Det ägda anropet fungerar redan med en dynamisk array. Behåll typfelet när elementet saknar metodens trait. [Lånat argument](https://github.com/Mik-pe/dunder-tui/blob/b22a99fa0d59702c82fa2126d7e59370f11f6dea/tests/dogfood/compiler/infer_array_argument.ddr) · [Ägt argument](https://github.com/Mik-pe/dunder-tui/blob/8a9ad4bf68d95a6deec9aa99298a4e9b000b2bbb/tests/dogfood/compiler/infer_owned_array.ddr)

## Bevara lånets ursprung i en matchgren

En metod med returtypen `&str from self` kan i 0.12.1 låna ett strängfält direkt. I `match &self.symbol` får `.Some(value) => value.as_str()` E4003 även när också reservsträngen ägs av mottagaren. Följ lånet genom mönstret och kontrollera båda grenarna. En strängvy från en lokal variabel ska fortfarande avvisas. [Minimiprov](https://github.com/Mik-pe/dunder-tui/blob/b22a99fa0d59702c82fa2126d7e59370f11f6dea/tests/dogfood/compiler/borrow_optional_field.ddr)

## Avsluta ett läslån när en retur lämnar loopen

I 0.12.1 ger `return move source` E4003 efter radvyns sista läsning i en loop över `std.text.lines`. Tillåt flytten på returgrenen när läsaren saknar destruktor och inga lån följer med returvärdet. Verifiera att en ensam rad kan flyttas utan kopia och att en flytt följd av `lines.advance()` fortfarande avvisas. [Minimiprov](https://github.com/Mik-pe/dunder-tui/blob/d772f4072c959adc2ea2677e1ed89eb27259d695/tests/dogfood/compiler/return_owner_from_line_loop.ddr)

## Tillåt ett valfritt muterbart lån vid uppslag

En metod som returnerar `Option<&mut i32> from self` får E3099 i 0.12.1. Stöd ett kortlivat lån till ett arrayelement när indexet finns och `.None` annars. Verifiera att ändringen syns i arrayen och att ett andra muterbart lån av samma element avvisas medan det första används. [Minimiprov](https://github.com/Mik-pe/dunder-tui/blob/aa3c7ec06326598063c61d311253d27e3ab6ebec/tests/dogfood/compiler/optional_mutable_element.ddr)

## Snabba upp nollställning och jämförelse på x86-64

I 0.12.1 tog `std.bytes.zero_memory` 28,64 µs för två heltalsarrayer med 2 304 celler vardera på översatt Linux x86-64. En typad loop tog 2,94 µs. Radvis jämförelse med `std.bytes.equal_memory` tog 44,49 µs mot loopens 20,57 µs. Mät på nativ Linux och Windows x64, optimera de två runtime-anropen och bevara ARM64-tiderna. Kontrollera udda radbredder och skillnader i båda arrayerna med samma kontrollsumma 45 120. [Mätprogram för båda implementationerna](https://github.com/Mik-pe/dunder-tui/blob/80d1e1cf10496477dce9a948a18d98145edf179c/benchmarks/language/cell_planes.ddr)

För 24 rader med 96 `i64` tog `equal_memory` 26,91 µs mot 1,06 µs med systemets `memcmp` på översatt Linux x64. Upprepa på nativ Linux x64. Bevara gränserna för lånade delarrayer, användardefinierad likhet och kontrollsumman 330 880. [Mätprogram](https://github.com/Mik-pe/dunder-tui/blob/af416d9b12c9f15e9d31504bf32668e0b462473d/benchmarks/language/row_equality.ddr) · [Rådata](https://github.com/Mik-pe/dunder-tui/blob/af416d9b12c9f15e9d31504bf32668e0b462473d/benchmarks/results/row-equality-0.12.1.tsv)

## Använd en enumvariant som funktion

Låt `Width.Length` användas där parametern har typen `fn(u16) -> Width`. I 0.12.0 får anropet E3003 och kräver omslaget `fn(n: u16) -> Width { .Length(n) }`. Prova att skicka konstruktorn till en funktion och kontrollera att resultatet blir `.Length(7)`. [Minimiprov](https://github.com/Mik-pe/dunder-tui/blob/a2cc8a10910e98ecd7f3f8157270fb6eb4c6199f/tests/dogfood/compiler/variant_function.ddr)

## Upprepa ett ägt värde i en array

Lägg till `array.repeat(move värde, antal)` för antal som bestäms vid körning. Flytta sista exemplaret, klona de tidigare och ge en tom array vid noll. För 4 096 arrayer med en post om 128 heltal undviker flytten 4 096 djupa arraykloner och 2 097 152 klonade byte jämfört med `array.resize` i 0.12.0. Verifiera att en ändring i en kopia lämnar de andra intakta. [Mätprogram för noll, en och fyra kopior](https://github.com/Mik-pe/dunder-tui/blob/6044f41affd9516162c4a318db08b21993509902/benchmarks/language/repeat_owned.ddr)

## Undvik allokering för omedelbart avlästa skalära returvärden

För 65 536 anrop till `std.terminal.columns` gav 49 152 giltiga bredder och 16 384 kontrolltecken totalt 49 160 allokeringsanrop med både `match` och `unwrap_or(-1)`. Fyra sparade bredder gav åtta anrop och samma summa. Minska kostnaden för `Some(i32)` över funktionsgränsen och behåll `.None` för kontrolltecken. Mät med `--runtime-profile` på macOS och Linux ARM64. [Prov för båda anropssätten](https://github.com/Mik-pe/dunder-tui/blob/95238cd3b27368b24eadf97552ba328914bccd8f/benchmarks/language/terminal_option.ddr)

Mät även `Result<u64,()>` för index som ska stanna vid noll eller maxvärdet. I 0.12.0 gav 65 536 index och tre `math.checked_*_u64` per index med `unwrap_or` 155 393 allokeringsanrop. Direkta heltalsuttryck gav 11. Båda gav summan 23 392 260 på macOS och Linux ARM64. Verifiera samma gränsbeteende och mät allokeringarna efter ändringen. [Körbart indexprov](https://github.com/Mik-pe/dunder-tui/blob/a2be35a2ccd947a9d502e7ff48682ec8f774ad73/benchmarks/language/saturating_indices.ddr)

## Lägg till Unicode-gemener i `std.text`

Behåll ASCII-beteendet i `text.lower` och lägg till en Unicode-variant. I 0.12.0 lämnas `blacK`, `İ` och `ΟΣ` oförändrade. Verifiera att Unicode-varianten ger `black`, `i` följt av U+0307 och `ος` med slut-sigma på macOS, Linux och Windows. [Minimiprov från TUI-bibliotekets färgläsare](https://github.com/Mik-pe/dunder-tui/blob/142420d3aafdd5d699aed95cddb9a46fec462625/tests/dogfood/unicode_lower.ddr)

## Läs JSON-objektets fält i en genomgång

Låt iteratorn även lämna en arrays barn i ordning. På macOS ARM64 tog summan av 4 096 redan tolkade heltal 25,34 ms via `doc.at` och 0,26 ms via nodlänkarna i 0.12.0. På Linux ARM64 tog det 20,66 respektive 0,19 ms. Verifiera summan 8 386 560 och linjär tidsökning med publikt API. [Mätprogram](https://github.com/Mik-pe/dunder-tui/blob/181c1540cb3d0340954ff153b3085db32e712853/benchmarks/language/json_array.ddr)

Ge `std.json.Doc` en iterator som lämnar nyckel och värdenod i källordning, även för dubbla och escapade nycklar. I 0.12.0 går `key_at` och `val_at` från första barnet för varje index. Att läsa ett redan tolkat objekt med 1 024, 2 048 och 4 096 fält tog 1,83, 7,40 och 29,69 ms i median på macOS ARM64. Kör samma prov med iteratorn och kontrollera linjär tidsökning och oförändrade kontrollsummor. [Mätprogram i Dunder](https://github.com/Mik-pe/dunder-tui/blob/247dd05/benchmarks/language/json_members.ddr)

Låt den som använder `@derive(Decode)` välja att avvisa ett upprepat känt fält. I 0.12.0 ger både `{"width":4,"width":9}` och `{"width":4,"w\u0069dth":9}` värdet 9 på macOS och Linux ARM64. Med kontrollen aktiverad ska båda ge fel vid den andra nyckeln och ett okänt fält fortfarande kunna hoppas över. [Minimiprov](https://github.com/Mik-pe/dunder-tui/blob/6ad7922/tests/dogfood/json_duplicate_fields.ddr)

## Komplettera `std.json` för egna kodare och fasta arrayer

- Ge `json.Writer` en publik metod för att avvisa ett värde och behålla det första felet. I 0.12.0 ger `writer.fail("utanför intervallet")` E2004 i en egen kodare. Prova ett ogiltigt värde inuti en array och kontrollera felet från både `finish()` och `finish_bytes()`. [Körbart rektangelprov](https://github.com/Mik-pe/dunder-tui/blob/bbd1564/tests/suite.ddr#L1563)
- Stöd `Encode` och `Decode` för `[T; N]`. I 0.12.0 ger skrivning och läsning av `[u16; 2]` E3004. Verifiera att `[12,24]` går att läsa tillbaka och att fel arraylängd ger ett avkodningsfel.
- Stöd `(A,B)` som JSON-array även inuti `[]T`. I 0.12.1 ger både markören `(u16,u16)` och ändrade celler av typen `[](i32,i32)` E3004 för `Encode` och `Decode`. Prova `[20,4]` och `[[3,5],[4,5]]` i båda riktningarna. Avvisa fel parlängd även i den andra posten och värden utanför elementtypen. [Minimiprov](https://github.com/Mik-pe/dunder-tui/blob/017c103c37eb7b3d4623261fb9dab89e5078394a/tests/dogfood/json_tuple.ddr)

## Verifiera Windows som kompilatorvärd

Dunder distribuerar en korsbyggd x86-64-kompilator för Windows. Kör följande på en riktig Windowsinstallation och dokumentera Windowsversion, källrevision och resultat.

Under Wine 8 byggde den publicerade Win64-kompilatorn från `c7a77b5` om sig själv byteidentiskt med ett och tre jobb. Språkservern, Unicode-argument, standardströmmar, barnprocesser och Win64-callbackar passerade. I pseudoterminal återställdes konsolläget och två barns utmatning gick till rätt skärmbuffert. Paketen utan beroenden och med sökvägsberoende passerade även med stängt standardfel. Det låsta Git-beroendet kunde läsas ur cache med `--locked --offline`. [Språkserver och CLI](../tools/test_lsp_and_dwarf.ddr) · [Tidigare nativa reproduktioner](https://github.com/Mik-pe/dunder/blob/v0.11.3/docs/next-steps.md#verifiera-windows-som-kompilatorvärd)

Uppgradering från 0.11.3 hämtade den publicerade exe-filen och gav status 3 när den gamla filen var öppen. Efter processens avslut kunde den verifierade ersättningen flyttas på plats. Den installerade filen matchade SHA-256 och byggde och körde ett språkprov. Upprepa proven på nativ Windows x64. [Release och kontrollsummor](https://github.com/Mik-pe/dunder/releases/tag/v0.12.0)

1. Hämta den publicerade exe-filen, kontrollera SHA-256 och bygg kompilatorn två gånger med ett jobb. Jämför filerna byte för byte. Upprepa med flera jobb.
2. Kör [provet för `check`, `build`, `run`, `test` och språkservern](../tools/test_lsp_and_dwarf.ddr) med kompilatorsökvägen som argument. Det använder ett lokalt paket i en sökväg med mellanslag och svenska tecken. Kör även [provet för miljö, standardfel och processavbrott](../tests/fixtures/process-unicode.ddr) samt [Win64-callbacken](../tests/fixtures/ffi/windows.ddr) med argumenten `"sök väg" "" "🦔"`.
3. Installera och uppgradera i en tom katalog. Prova även bytet medan den installerade filen används. [Installation och distribution](releases.md) · [Plattformskontrakt](targets.md)

Kör även `dunder run tests/fixtures/windows-terminal.ddr` i en konsol.
Provet jämför konsolläget före och efter tidig retur och läser tillbaka två barns utmatning från separata skärmbuffertar.

Privata filer på Windows behöver ett kontrakt för Windows åtkomstkontrollistor.
Verifiera åtkomsten som ett annat användarkonto och utred vilka garantier ett
beständigt katalogbyte kan ge när datorn startas om. [Filrättigheter och synkning](sys.md)

Under Wine 8 gav 0.12.0 rätt UTC-kalender och veckodag vid 1601-01-01, före Unixepoken och 2024-03-31. Tider utanför FILETIME-intervallet gav `.None`. [Tidsomvandling](../stdlib/time.ddr)

Kör `dunder run tests/fixtures/local-calendar.ddr -- UTC` på Windows. Ange värdens tidszon som `UTC`, `Europe/Stockholm` eller `America/New_York`. Provet kontrollerar årsskiften, början och slutet på sommartid, de ändrade amerikanska reglerna 2007 och FILETIME-gränser. Under Wine 8 passerade alla tre zonerna. [Kalenderprov](../tests/fixtures/local-calendar.ddr) · [Windows tidszonsregler per år](https://learn.microsoft.com/en-us/windows/win32/api/timezoneapi/nf-timezoneapi-gettimezoneinformationforyear)

## Mät tidsgränser över systemvila

På Apple M5 anger systemet 1 000 ns för `Deadline` och 42 ns för `Raw`. Mät båda klockorna och väggtiden före och efter faktisk systemvila. På macOS använder Dunder `CLOCK_MONOTONIC` för tidsgränser och `CLOCK_MONOTONIC_RAW` för rå tid. Enligt macOS manual för `clock_gettime(3)` räknar båda med systemvilan. En tidsgräns kan därför ha löpt ut vid väckning. [Klockkontrakt](standard.md) · [Mätprogram](https://github.com/Mik-pe/dunder-tui/blob/b9d76a2/benchmarks/language/monotonic_clock.ddr)

## Verifierat efter 0.12.0

En loop kan packa upp heltalspar ur en temporär array. Kompilatorn försökte tidigare frigöra stackkopian av varje par. Regressionstestet passerar med standardflaggor på macOS ARM64 och Linux ARM64 och x86-64. Ramprovet avbröts med 0.12.0 efter 3 347 byte på macOS. Med rättningen gav det samma 10 829 338 byte på alla tre som kontrollbygget med 0.12.0 och `--mir-memory=off` på macOS. [Regressionstest](../tests/fixtures/aggregate_arrays/temporary_pairs.ddr) · [Cellprov](https://github.com/Mik-pe/dunder-tui/blob/5eb4c33/tests/border_cells.ddr)

## Verifierat i 0.12.0

- En lokal post kan låna två ägare. Båda skyddas till sista läsningen. `for x in &mut values` lånar ett element per varv och förhindrar att arrayen flyttas eller växer under lånet. `swap(&mut a, &mut b)` byter också ägda resurser bakom muterbara parametrar. 4 095 byten av två arrayägande poster gav samma 19 allokeringsanrop som lokala flyttar. [Ägarskap](memory.md) · [Låneprov](../tools/ownership_probe_cases.ddr)
- `Option<&str> from value` behåller en delvys byteintervall och ägare. Med `from self.text` kan två grafem leva medan samma iterator går vidare. Returer av lokala källor och ett annat fält avvisas. [Iterator](../stdlib/text.ddr) · [Delvyer](../tests/fixtures/local_borrows)
- För 1 024 iteratorposter med en inre post minskade kopiorna från 5 312 till 3 264 och allokeringsanropen från 7 489 till 5 377. Resterande kopior uppstår bland annat när de inre posterna konstrueras. Indexläsaren gjorde ett allokeringsanrop. Båda gav 110 080. [Mätprogram](https://github.com/Mik-pe/dunder-tui/blob/f3ecfc1/benchmarks/language/iterator_item.ddr)
- 65 536 färguppslag gjorde 259 allokeringsanrop med direkt skickad, lånad respektive omedelbart matchad valfri färg. Den valfria vägen gjorde tidigare 65 795. Två vägar genom `Result<(),string>` gav nu vardera 4 103 anrop och samma 4 096 fel. Sparade resultat och resursägande nyttolaster täcks av regressionstester. [Färguppslag](https://github.com/Mik-pe/dunder-tui/blob/d1765d8/benchmarks/language/colors.ddr) · [Resultat](https://github.com/Mik-pe/dunder-tui/blob/a7cb034/benchmarks/language/unit_result.ddr)
- Lokala skrivskyddade heltalstabeller och fasta strängtabeller som modulkonstanter gjorde ett allokeringsanrop per process. Kontrollsummorna var 581 894 144 respektive 131 072. Ett kopierat koordinatpar behåller sitt värde vid senare skrivning. Både kopierad och lånad läsning gav 836 000 utan extra tupelkopior i loopen. Villkorsgrenen med två skalära fält gav 67 096 575 och noll tupelkonstruktioner. [Proven för tabeller och tupler](https://github.com/Mik-pe/dunder-tui/tree/b9d76a2/benchmarks/language)
- 24 radjämförelser à 96 `i64` med `==` tog 1,06 µs på macOS ARM64 och 0,87 µs på Linux ARM64 i Kiri. `std.array.equal` tog 0,95 respektive 0,82 µs. Median av tre körningars medianer. Tidigare tog `==` 9,85 och 8,43 µs. Tester täcker tomma och förskjutna intervall, olika längder, första och sista skillnad och användardefinierad `Eq`. [Mätprogram](https://github.com/Mik-pe/dunder-tui/blob/b9d76a2/benchmarks/language/row_equality.ddr) · [Regressioner](../tests/fixtures/local_borrows/views.ddr)
- På macOS tog första raden med `std.text.lines` 32 ns mot 14 ns med anroparens indexläsare. Alla 3 073 rader tog 39,00 respektive 54,46 µs. Hela provprocessen gjorde 27 mot 24 allokeringsanrop vid tidigt avbrott och 28 mot 25 för hela texten. Kontrollsummorna var 5 och 10 243. [Mätprogram med båda läsarna](../benchmarks/stdlib/line_scan.ddr)
- `std.array.append_move` gav 4 291 allokeringsanrop med reserverad kapacitet och 4 297 när destinationen växte. Alla 24 576 heltal behöll ordningen och båda profilerna visade noll arraykopior. [Mätprogram](../benchmarks/stdlib/append_move.ddr)
- `ByteQueue.remove(start,count)` kan ta bort ett markörsvar mellan tangenter och bevara ett ofullständigt UTF-8-tecken. Vid läsning av 1 MiB från en filbeskrivare i 128-byteblock tog mellanbuffert och kö 8,11 ms mot 7,73 ms för direkt bearbetning. Median av sju körningar med stor tidsspridning. Kön lade till ett allokeringsanrop totalt. [Mätprogram](../benchmarks/stdlib/byte_queue.ddr) · [Bytekön och dess test](../stdlib/bytes.ddr)
- För 5 244 emojier, 742 presentationsvarianter, 2 175 ligaturfall och tio kontrollfall stämde samtliga 48 059 bredder med referensprovet, även efter delning vid varje skalärgräns. Dunder gav `.None` för alla 13 delspann med kontrolltecken. [Korpus och delningar](https://github.com/Mik-pe/dunder-tui/blob/8601157/docs/parity.md#bredd-vid-delade-unicode-sekvenser)
- `terminal.columns` läser den lånade UTF-8-vyn framåt och söker bakåt för skriftspecifika ligaturer. I 65 536 anrop sjönk antalet allokeringsanrop från 196 625 till 65 545 på macOS och Linux ARM64. Med fyra sparade bredder sjönk det från 17 till 9. Båda vägarna gav 98 304. I separata prov med tre uppstartsallokeringar lade `.runes()` till 131 072 anrop och retur av `Some(i32)` lade till 65 536. [Mätprogram](https://github.com/Mik-pe/dunder-tui/blob/8601157/benchmarks/language/terminal_width.ddr) · [Breddmätningen](../stdlib/terminal/width.ddr)
- Diagramvyn gav identiska 4 829 492 byte över 1 100 bilder i pseudoterminal med 0.11.3 och 0.12.0. Median för oförändrad bild gick från 96 till 93 µs, för hel omritning från 162 till 160 µs. På fysisk Linux x86-64 gick växling mellan två storlekar av en formaterad bild från 4,44 till 4,21 µs. Under hela storleksprovet minskade allokeringsanropen från 338 866 till 941 med samma 38 färska allokeringar och kontrollsumman 9 912 320. [Diagramvyn](https://github.com/Mik-pe/dunder-tui/tree/v0.9.0/benchmarks) · [Storleksprovet](https://github.com/Mik-pe/dunder-tui/blob/b9d76a2/benchmarks/reshape.ddr)
- På fysisk Intel Core i5-1035G7 gick radfyllningen från 177,06 till 167,01 instruktioner per cell med de sammanslagna x86-stackkontrollerna. Radjämförelsen gick från 202,98 till 192,93. På ARM64 var funktionernas maskinkod identisk genom första retur, 78 respektive 90 instruktioner. Kalenderprovet gav samma celler över 1 100 bilder. Den låsta kalenderns tidigare PTY-median var 544 µs i Dunder och 89 µs i Ratatui och 11,40 % av Dunderinstruktionerna låg i `Frame::styled_rune`. Layoutprovet omfattar 30 körningar per representation och arkitektur. [Layoutdata](../benchmarks/layout/2026-11-22/README.md) · [Kalenderarbetslast](https://github.com/Mik-pe/dunder-tui/blob/v0.12.0/benchmarks/calendar.md)

Kör `line_scan std first time` och byt till `cursor` eller `all` för de andra radproven. För allokeringsprofiler, bygg med `--runtime-profile` och utelämna `time`. Kör `append_move reserved` och `append_move grow`. Skapa byteindata med `byte_queue input > in.bin` och kör `byte_queue direct < in.bin` respektive `byte_queue queue < in.bin`.

Plattformsvillkor, specialisering, separata fältlån, matchgrenar, uppackning av enhetsresultat och gemensam UTF-16-kodning verifierades redan i 0.11.3. [Proven och tidigare mätvärden](https://github.com/Mik-pe/dunder/blob/db06960/docs/next-steps.md)
