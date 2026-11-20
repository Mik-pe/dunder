# sys på macOS och Linux

`import sys` laddar Dunders medföljande systembibliotek. Den första
implementationen stöder ARM64/macOS, ARM64/Linux och x86-64/Linux med Dunders egen
native-backend. Källfiler har ändelsen `.ddr`; importer skrivs utan suffix.

```dunder
import sys

fn main() -> i32 {
    written := sys.write_text("hello.txt", "Hej från Dunder 🌍\n")
    if !written.ok {
        print(sys.error_message(written.error))
        return 1
    }
    content := sys.read_text("hello.txt")
    if !content.ok {
        print(sys.error_message(content.error))
        return 1
    }
    print(content.value)
    info := sys.metadata("hello.txt")
    if info.ok { print(info.size) }
    0
}
```

Biblioteket är inbäddat i kompilatorn. Projektet behöver ingen lokal
`sys.ddr`, installerad standardbibliotekskatalog eller nätverksåtkomst.
Programmet behöver inte kompilatorn eller bibliotekets källfiler vid körning.
API:t finns i [stdlib/sys.ddr](../stdlib/sys.ddr); den lilla OS-gränsen
implementeras i [native-backenden](../selfhost/runtime/runtime_sys.ddr).

## Funktioner och resultat

| Funktion | Returtyp | Beteende |
| --- | --- | --- |
| `sys.read(path)` | `sys.ReadResult` | UTF-8-läsning med `Ok(string)` / `Err(IoError)`. |
| `sys.read_text(path)` | `sys.TextResult` | Läser en vanlig fil och validerar UTF-8. |
| `sys.write_text(path, text)` | `sys.Status` | Skapar filen eller ersätter hela dess innehåll. |
| `sys.write_stderr(text)` | `sys.Status` | Skriver textens exakta UTF-8-byte till stderr, utan radbrytning. |
| `sys.eprint(text)` | `sys.Status` | Skriver text till stderr följd av en radbrytning. |
| `sys.append_text(path, text)` | `sys.Status` | Skapar filen eller lägger till text sist. |
| `sys.metadata(path)` | `sys.Metadata` | Storlek och fil-/katalogtyp; följer symboliska länkar. |
| `sys.mkdir(path)` | `sys.Status` | Skapar en katalog, ett steg åt gången. |
| `sys.rename(from, to)` | `sys.Status` | Döper om/flyttar enligt operativsystemets regler. Kan ersätta en befintlig destination. |
| `sys.remove_file(path)` | `sys.Status` | Tar bort en fil eller själva den symboliska länken. |
| `sys.remove_dir(path)` | `sys.Status` | Tar bort en tom katalog. |
| `sys.pid()` | `i32` | Processens ID. |
| `sys.error_message(error)` | `string` | Kort meddelande för vanliga fel; annars `"OS error N"`. |
| `sys.file_kind(info)` | `sys.FileKind` | `Other`, `File` eller `Directory` från en `Metadata`. Kontrollera först `info.ok`. |
| `sys.error_from_code(code)` | `sys.IoError` | Namngivet fel eller `Other(code)` för en okänd kod. |
| `sys.error_code(error)` | `i32` | Dunders felkod, även för `Other(code)`. |
| `sys.io_error_message(error)` | `string` | Feltext för ett typat `IoError`. |
| `sys.write_bytes(path, &bytes)` | `sys.Status` | Skriver `[]i32` som råa byte; kräver värden 0–255. |
| `sys.decode_utf8(&bytes)` | `sys.TextResult` | Validerar byteintervallet och avkodar giltig UTF-8. |
| `sys.arg_count()` | `i32` | Antal programargument, inklusive programnamnet. |
| `sys.arg(index)` | `string` | Kopierar och UTF-8-validerar ett argument; `arg(0)` är programnamnet. |
| `sys.same_file(first, second)` | `bool` | Jämför device/inode, följer länkar. Falskt om stat eller sökvägskontroll misslyckas. |
| `sys.make_executable(path)` | `sys.Status` | Sätter läge 0755; används för färdigbyggda program. |
| `sys.now_ms()` | `i64` | Monoton klocka i millisekunder. |
| `sys.sleep_ms(ms)` | `()` | Pausar aktuell tråd minst `ms` millisekunder. |
| `unsafe sys.envp()` | `ptr` | Aktuell miljövektor på Linux: libc:s levande `environ` när programmet importerar libc, annars startstackens `envp`. `null` på macOS. |
| `unsafe sys.linux_syscall(nr, a0..a5)` | `i64` | Rått Linux-systemanrop via ARM64-kanoniskt anropsnummer (`nr: i64, a0..a5: i64`; översätts automatiskt på `x86_64-linux`). |
| `unsafe sys.memory_copy(destination, source, count)` | `()` | Kopierar `count: usize` byte mellan giltiga minnesområden; överlappning är tillåten. |
| `unsafe sys.memory_equal(lhs, rhs, count)` | `bool` | Jämför `count: usize` byte i giltiga minnesområden. |

Samtliga sökvägar och textargument har typen `string`. `read` använder enums:

```dunder
pub enum FileKind { Other = 0, File = 1, Directory = 2 }
pub enum ReadResult { Ok(string), Err(IoError) }
```

`IoError` har bland annat `NotFound`, `PermissionDenied`, `InvalidUtf8` och
`Other(i32)`. Den sistnämnda bevarar fel som ännu saknar ett eget namn.
Hela deklarationen finns i [sys.ddr](../stdlib/sys.ddr).

```dunder
fn load(path: string) -> string {
    match sys.read(path) {
        .Ok(text) => text,
        .Err(error) => { print(sys.io_error_message(error)); return "" },
    }
}
```

`read` är en Dunder-wrapper över `read_text` med samma läsregler och gränser.
Övriga I/O-funktioner och `read_text` använder nominella resultatstructar,
som också kan skickas och returneras mellan funktioner:

```dunder
pub struct Status { ok: bool, error: i32 }
pub struct TextResult { ok: bool, value: string, error: i32 }
pub struct Metadata {
    ok: bool,
    size: i64,
    is_file: bool,
    is_dir: bool,
    error: i32,
}
```

`.ok` är sant exakt när `.error` är noll. Vid fel är `TextResult.value`
tom och metadatafält noll/falska. Det skiljer en tom fil som lästes korrekt
från ett misslyckat anrop. `.size` räknar byte, inte Unicode-kodpunkter.
Ett normalt I/O-fel är ett resultat; det avbryter inte processen.
Koden väljer själv hur felet hanteras. För generella typade fel finns
`Result<T, E>` i det fördefinierade biblioteket tillsammans med
`?`-operatorn; se [typade fel med Result](enums.md#typade-fel-med-result).
Konkreta enums och `match` fungerar också; se [enumreglerna](enums.md).

`write_stderr` och `eprint` låter ett CLI-program skilja diagnostik från
maskinläsbar stdout. Båda returnerar `Status`; bind resultatet och kontrollera
`.ok` när programmet behöver hantera ett skrivfel. De använder processens
befintliga filhandtag 2, öppnar eller stänger det inte och bevarar inbäddad NUL.
Korta skrivningar fortsätter från rätt byte och avbrott försöks igen.
Ett stängt handtag ger fel 9; ett fullt icke-blockerande handtag ger fel 35.
Vid ett senare fel kan en del av texten redan ha skrivits. `eprint` använder
den vanliga strängarenan för att lägga till radbrytningen.

`write_bytes` följer samma fil-/skrivregler som `write_text`, men UTF-8 krävs
inte för innehållet. Den validerar byteintervallet före öppning/trunkering;
ett felaktigt element ger 22. En tillfällig packad bytebuffert använder
strängarenan och kan ge 12 om utrymmet inte räcker. `decode_utf8` har samma
intervallkontroll, ger 92 för ogiltig UTF-8 och har tom `.value` vid fel.
Sökvägar är fortsatt UTF-8 även vid binär utdata.

Argument exponeras som vanliga oföränderliga Dunder-strängar. Felaktigt
index ger status 103; ett argument med ogiltig UTF-8 ger 106. Vid `dunder run`
skickas programargument efter `--`, exempelvis `dunder run main.ddr -- hello`.

## Bytebuffertar och rått minne

`std.bytes.equal(&lhs, &rhs)` jämför packade bytebuffertar.
`equal_ranges(&lhs, lstart, lend, &rhs, rstart, rend)` jämför halvöppna
intervall och ger `false` för ogiltiga gränser eller olika längd. Tomma,
giltiga intervall är lika. Funktionerna kräver ingen `unsafe`-behörighet.
`extend`, `extend_range` och `append_utf8` reserverar utrymme en gång och
kopierar byte i bulk; `extend_range` avbryter vid ogiltiga intervall.

`unsafe sys.memory_copy`, `unsafe sys.memory_equal` och `unsafe sys.memory_zero`
tar råa pekare och ett antal byte av typen `usize`. Anroparen ansvarar för
läsbart källminne och, vid kopiering eller nollställning, skrivbart målminne
under hela anropet. Överlappande kopiering fungerar i båda riktningarna. Noll
byte läser eller skriver inget minne och tillåter därför nullpekare; jämförelsen
ger då `true`. Jämförelsen har inget kontrakt om konstant körtid. `memory_copy`
och `memory_equal` finns även som `std.bytes.copy_memory` och
`std.bytes.equal_memory`. Samtliga stöds av alla native-mål och WebAssembly utan
en C-biblioteksimport.

För generell allokering finns dessutom `unsafe sys.pool_alloc_try(size)` och
`unsafe sys.pool_free(pointer)` ovanpå Dunders inbyggda poolallokator. Till
skillnad från automatiska array- och strängtillväxter fäller `pool_alloc_try`
aldrig utan returnerar en nullpekare vid begäran om `0` byte, storlekar över
`256 MiB` eller full pool. [Generell allokering](standard.md#generell-allokering).

## Sökvägar och filer

Relativa sökvägar utgår från programmets arbetskatalog, inte källfilens
katalog. Absoluta sökvägar fungerar också. Sökvägar måste vara giltig UTF-8;
filnamn med andra byteföljder kan inte uttryckas med denna första API-version.
Inbäddad NUL avvisas före OS-anropet så att en sökväg aldrig tyst kapas.
Högst 1023 UTF-8-byte ryms i en sökväg; OS:et kontrollerar också komponenter.

`read_text`, `write_text` och `append_text` hanterar vanliga filer.
Kataloger, enheter och FIFO:er används inte som textströmmar. Öppning är
icke-blockerande, filtypen kontrolleras och filhandtaget stängs både vid
lyckat anrop och vid fel. Råa filbeskrivare stöds på låg nivå genom `fd_read`,
`fd_write` och `fd_close`, som ligger till grund för ägda strömhandtag som
`Stdin`/`Stdout`/`Stderr` i `std.io`, `FileReader`/`FileWriter`/`File` i
`std.fs` samt `ChildStdin`/`ChildStdout`/`ChildStderr` i `std.process`; se
[Strömmar och standardinmatning](standard.md#strömmar-och-standardinmatning).
Textinnehåll får innehålla NUL. Byteföljden måste vara giltig UTF-8 vid läsning.

`write_text` trunkerar en befintlig vanlig fil innan skrivningen. Den är
inte en atomisk publicering; ett senare skrivfel kan lämna delvis data.
`append_text` använder OS:ets append-läge. Skrivloopar hanterar korta
skrivningar och avbrott, men hela strängen är inte en transaktion gentemot
andra processer. Nya filer begär läge 0600 och nya kataloger 0700,
begränsat av processens umask. Befintliga filers behörigheter bevaras.

Läsning/skrivning och metadata följer symboliska länkar. `remove_file`
tar bort länken själv. Borttagning är inte rekursiv. `rename` kan misslyckas
över filsystemsgränser och följer målplattformens regler för befintliga destinationer.

## Gränser och fel

`read_text` tillåter högst **4 MiB per fil**, baserat på filens storlek efter
öppning. En större fil ger fel 27. En fil som krymper kan ge ett kortare
lyckat resultat. Växer den utöver den uppmätta storleken under läsningen
ger EOF-kontrollen fel 35 i stället för tyst trunkering. Läsningen är inte
en atomisk snapshot gentemot samtidiga skribenter.

Lästa strängar använder samma återanvändbara heap som andra strängar och arrayer.
[Allokering och ägarskap](memory.md).

`.error` använder Dunders ursprungliga Darwin-kompatibla felkoder. På Linux
översätts kända errno-värden till samma koder, så exempelvis `NotFound`,
`NotEmpty` och `InvalidUtf8` har samma betydelse på båda plattformarna.
Linuxfel utan motsvarande kod representeras som `10000 + errno` och blir
`Other(code)`; de råkar därför inte få betydelsen av ett annat Darwinfel.
Vanliga värden:

| Fel | Betydelse |
| --- | --- |
| 2 | Fil eller katalog saknas. |
| 4 | Avbruten (`Interrupted`, EINTR). |
| 12 | Minne/strängarena räcker inte. |
| 13 | Åtkomst nekad. |
| 17 | Finns redan. |
| 21 | En katalog användes som textfil. |
| 22 | Ogiltigt argument, bland annat NUL i sökvägen. |
| 27 | Filen är för stor. |
| 28 | Slut på utrymme (`NoSpace`, ENOSPC). |
| 35 | Tillfälligt otillgänglig (`WouldBlock`); kan också betyda tillväxt under läsningen. |
| 45 | Filtypen stöds inte för text-I/O. |
| 63 | Sökvägen är för lång. |
| 66 | Katalogen är inte tom. |
| 92 | Filinnehållet är inte giltig UTF-8. |
| 93 | Oväntat filslut (`UnexpectedEof`). |
| 94 | Skrivning skrev noll byte (`WriteZero`). |

`IoError::would_block()` konstruerar motsvarande fel och `.is_would_block()`
provar om felet betyder att anropet skulle ha blockerat. OS:et kan ge andra
felkoder; `error_message` är en liten Dunder-tabell, inte hela systemets
lokaliserade felkatalog.

## Importer och verifiering

Det exakta importnamnet `sys` väljer det medföljande biblioteket och kan
aliaseras med `import sys as os`. Flera importer laddar det en gång och
delar samma typidentitet, också genom `import std.sys as os`.
För en egen lokal fil med samma namn används
exempelvis `import .sys as local`. Vanliga moduler söks fortsatt relativt
den importerande filen. De privata `__sys_*`-intrinsics är endast tillgängliga
inne i det medföljande biblioteket.

```sh
dunder test tests/net_loopback.ddr
```

Testsviten `tests/net_loopback.ddr` provar klocka (`sys.now_ms`), råa
byte-läsningar (`sys.read_u8_limit`, `fs.ReadDir` + `.resolve_read`),
filflöde, felkoder och icke-blockerande TCP-sockets. [Filhandtagsprov](../tools/test_files.ddr).

Implementationens syscallnummer och stat-layout är verifierade mot
[Apples XNU-anrop](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/syscalls.master)
och [stat-definitioner](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/sys/stat.h),
samt tidigare macOS-körningar. Linux använder direkta systemanrop och respektive
arkitekturs stat-layout; `sys.linux_syscall` tar Linux ARM64-kanoniska
systemanropsnummer (som mappas om till x86-64 i x86-backend och returnerar
`-38` för okända nummer på Linux samt `-78` på macOS). På Linux följer
`sys.envp` libc:s aktuella `environ` när programmet redan importerar
libc som `c` eller `libc.so.6`. Därmed ser `std.process.env`, `spawn`, `replace` och PATH-sökning
också ändringar genom `setenv`, `unsetenv` och `clearenv`. En nullpekare
betyder tom miljö; tidigare borttagna variabler återinförs inte från
startstacken. Program som enbart använder systemanrop använder startstackens
bevarade `envp` och behåller sitt statiska ELF-format. `sys.envp` ger fortsatt null
på macOS, där `std.process` använder plattformens miljö-API. Se
[målplattformarnas verifiering](targets.md).
Systemanropen är en plattformsdetalj som anpassas per mål.
Binärläsning finns som `sys.read_u8`/`read_u8_limit`/`read_u8_into` och
katalogverktyg i `std.fs` (`list_dir`, `ReadDir::list`, `remove_all`).
Godtyckliga rättighetsändringar (`chmod`) återstår.

## Kapabilitetsöversikt

Moduler i Dunder deklarerar använda privilegier med `capability ...` högst upp
i källfilen. De grundläggande fil-, text- och klockfunktionerna i `sys` kräver
däremot ingen särskild `capability`-rad: de verkar med öppna/stängda
engangsanrop eller direkt över en filbeskrivare som anroparen redan fått, utan
delegeringsbara katalog-/filhandtag eller godtycklig process-/nätverksåtkomst.
Högre standardmoduler skyddar bredare OS-operationer med uttryckliga kapabiliteter
(`@requires(...)`) så att beroendebibliotek inte tyst kan skaffa behörighet.

| Modul / API | Krävd kapabilitet | Exempel på operationer |
| --- | --- | --- |
| `sys` (grundläggande I/O, tid, argument) | Ingen | `sys.read`, `sys.read_text`, `sys.write_text`, `sys.append_text`, `sys.metadata`, `sys.mkdir`, `sys.rename`, `sys.remove_file`, `sys.remove_dir`, `sys.same_file`, `sys.read_u8`, `sys.write_u8`, `sys.now_ms`, `sys.sleep_ms`, `sys.pid`, `sys.arg`, `sys.fd_read`/`fd_write`/`fd_close` |
| `sys` (råminne & systemanrop) | `capability unsafe` | `sys.envp`, `sys.linux_syscall`, `sys.memory_copy`, `sys.memory_equal`, `sys.memory_zero`, `sys.pool_alloc_try`, `sys.pool_free` |
| `std.fs` | `capability filesystem` | `fs.list_dir`, `fs.mkdir_all`, `fs.remove_all`, `fs.temp_dir`, `fs.cwd`, `fs.realpath`, `fs.symlink`, `fs.hard_link`, `fs.is_symlink`, samt skapande av handtagen `ReadFile::grant`, `WriteFile::grant`, `ReadDir::grant`, `WriteFile::create` och `File::open` (även `std.sqlite` använder `filesystem` och `unsafe`) |
| `std.io` (standardströmmar) | `capability stdio` | `io.stdin()`, `io.stdout()`, `io.stderr()`, `io.read_stdin`, `io.read_stdin_bytes`, `io.read_stdin_limit` |
| `std.net` | `capability network` | `TcpListener`, `TcpStream`, `UdpSocket`, `net.resolve`, `net.listen`, `net.connect` |
| `std.process` | `capability process` | `Command::spawn`/`exec`, `Executable::grant`, `process.run`, `process.replace`, `process.env`, `ChildSet` |
| `std.dl`, `std.alloc.RawAllocator`, FFI | `capability unsafe` | `dl.open`/`sym`, `RawAllocator::alloc`/`dealloc`, `extern`, `addr`, `ptr`, `bytes_view` |

Observera varför exempelvis `fs.list_dir` och `fs.realpath` kräver
`capability filesystem` medan `sys.read_text` inte gör det: `std.fs` exponerar
katalogtraversering, rekursiv radering och delegeringsbara fil-/kataloghandtag
(`ReadFile`, `WriteFile`, `ReadDir`), som är tänkta att styras i paketets
ytterkant. Själva handtagsmetoderna som `.read()` på ett redan beviljat
`ReadFile`-handtag kan däremot anropas i moduler utan egen `capability
filesystem`; bara skapandet (`grant`/`open`) kräver kapabiliteten.

För kod som ska få åtkomst till en vald fil via en parameter finns
`std.fs.ReadFile` och `std.fs.WriteFile`. Handtagen skapas i en modul
med `capability filesystem`. [Filhandtag och deras sökvägssemantik](ffi.md#filåtkomst-genom-delegerade-handtag)
