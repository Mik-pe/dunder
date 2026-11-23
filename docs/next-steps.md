# Nästa steg för Dunder

## Använd samma paketrot för inbäddade filer

Med `root = "src/lib.ddr"` klarar `@embed_file("src/data.txt")` bibliotekets egna tester men ger E3099 när en app importerar biblioteket. Med `"data.txt"` blir utfallet det omvända. Utgå från bibliotekets manifestkatalog i båda fallen. Verifiera filinnehållet för både `@embed_file` och `@embed_bytes` vid direktbygge, test och bygge som beroende. [Tvåpaketsrepro för 0.11.1](https://github.com/Mik-pe/dunder-tui/tree/v0.7.0/tests/dogfood/embed)

## Verifiera Windows som kompilatorvärd

0.9.0 innehåller en korsbyggd x86-64-kompilator för Windows. Kör följande på en riktig Windowsinstallation och dokumentera Windowsversion, källrevision och resultat.

1. Hämta den publicerade exe-filen, kontrollera SHA-256 och bygg kompilatorn två gånger med ett jobb. Jämför filerna byte för byte. Upprepa med flera jobb.
2. Kör `check`, `build`, `run`, `test` och språkservern i ett projekt med mellanslag och svenska tecken i sökvägen. Använd ett lokalt paketberoende. Prova miljö, processavbrott, standardfel och callback över C-gränsen.
3. Installera och uppgradera i en tom katalog. Prova även bytet medan den installerade filen används. [Installation och distribution](releases.md) · [Plattformskontrakt](targets.md)

Privata filer på Windows behöver ett kontrakt för Windows åtkomstkontrollistor.
Verifiera åtkomsten som ett annat användarkonto och utred vilka garantier ett
beständigt katalogbyte kan ge när datorn startas om. [Filrättigheter och synkning](sys.md)

## Delade återlån och lånefel

Om en callback till `alloc.with_bytes` försöker returnera ett lån av blocket avvisas det med E4003. Låt diagnostiken visa användarens anrop tillsammans med den specialiserade signaturen. [Bytevyer och ägarskap](memory.md)

- Låt en metod med `&mut self` uppdatera en samling och returnera en delad vy med `from self`. Godkänn läsning från vyn och avvisa mutation av samlingen så länge vyn används.
- Vid E4005 för ett temporärt argument, visa var lånet når en rå adress. Prova både ett temporärt värde och samma värde i en namngiven lokal. [Reproduktioner av båda lånefallen](https://github.com/Mik-pe/dunder-tui/blob/v0.7.0/docs/dunder.md)

## Strömmande Unicode och terminal-I/O

1. Lägg till en UTF-8-prefixavkodare för `&[]u8` som skiljer en färdig skalär från ofullständig och ogiltig indata. Prova varje delningspunkt i en flerbytessekvens.
2. Lägg till en iterator över utökade grafem med lånade UTF-8-intervall i `std.text`. Verifiera mot Unicode-versionens officiella grafemprov. [Segmentering och provdata](https://github.com/Mik-pe/dunder-tui/blob/v0.7.0/tests/grapheme_cases.ddr)
3. Ge standardbiblioteket en terminalresurs för storlek, raw mode och återställning. Kontrollera att ursprungligt läge återställs även vid tidig retur på POSIX och Windows.
4. Låt `std.process.Command` använda ett redan öppet handtag för en vald standardström. Starta två barn med varsin terminal och kontrollera att förälderns standardströmmar förblir intakta. [Nuvarande PTY-prov](https://github.com/Mik-pe/dunder-tui/blob/v0.7.0/tools/pty.ddr)

## Slå upp importalias i språkservern

Skapa `alpha.ddr` med `pub fn lookup(v: string) -> string { v.clone() }` och
`beta.ddr` med `pub fn lookup(v: i32) -> i32 { v }`. Kör `dunder check main.ddr` med följande rotfil.

```dunder
import alpha as text_box
import beta as int_box
fn main() { print(text_box.lookup("x")); print(int_box.lookup(42)) }
```

Kompilatorn godkänner alla tre modulerna. På `lookup` i det andra anropet visar
`textDocument/hover` strängsignaturen och `textDocument/definition` pekar på
`alpha.ddr`. `textDocument/completion` efter `int_box.loo` ger en tom lista.
Alla tre svaren ska använda heltalsfunktionen i `beta.ddr`.

## Följ upp mätningar för SIMD

Arbetslasten i [layoutmätningen](../benchmarks/layout/2026-11-22/README.md) har ett oberoende numeriskt facit för både punktposter och separata kolumner. ARM64-serien kördes på fysisk maskin. Upprepa x86-64-serien på fysisk hårdvara inför ett beslut om vektorisering eller instruktionsschemaläggning. Behåll låneprovet för kolonntillväxt vid en framtida representationsändring.

## Mät temporära värden när små poster läggs i en array

Fyra omgångar med 4 096 tvåfältsposter gav samma kontrollsumma och 16 387 allokeringsanrop för vanliga poster och `@c_layout` på 0.11.1. Antalet färska allokeringar var 4 099 respektive 4. Följ värdet från konstruktion till `push`, redovisa poolåterbruk separat och mät nyttan av att konstruera direkt i arrayen. [Körbart jämförelseprov](https://github.com/Mik-pe/dunder-tui/blob/v0.7.0/benchmarks/language/records.ddr)
