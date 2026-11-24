# Distribuera Dunder

## Versionsserien 0.11

Från `0.11.0` ska program som följer den dokumenterade syntaxen,
typ- och ägarskapsreglerna och standardbibliotekets publika kontrakt fortsätta
att bygga och ha samma dokumenterade beteende i senare 0.11-versioner.
Löftet gäller ARM64/macOS och Linux på ARM64 och x86-64. Varje kandidat provas
med den självbyggda kompilatorn på alla tre värdarna. Underhållsversioner får lägga till API:er
och rätta fel där kompilatorn tidigare accepterade ogiltiga program. En ändring
som kräver att giltiga program skrivs om hör till en ny versionsserie med
migrationsanvisningar.

Binärkompatibilitet mellan kompilatorversioner, intern representation och exakt
diagnostiktext kan ändras. Windows är en förhandsversion. WebAssembly och WASI är
korskompileringsmål med egna begränsningar för bland annat systemanrop och tasks.
Stabila installationer följer fortsatt den stabila releasekanalen.
[Målplattformar](targets.md)

### Uppgradera från 0.10

Kör `dunder upgrade` och sedan `dunder test` i projektet. Två kontroller kan
ge nya felmeddelanden i befintlig kod.

- Med `@derive(Eq)` måste varje structfält och varje enumvariants nyttolast
  uppfylla traiten `Eq`. Om en fälttyp har en egen `eq`-metod, lägg den i
  `impl Eq for Typ` eller använd `@derive(Eq)` även på fälttypen.
- En closure som fångar en ägd array eller resurs genom värde tar över den
  även när samma closure också innehåller lån. Behöver du fortsätta använda
  ägaren efteråt kan du låna värdet i fångstlistan, till exempel
  `fn[&values, &offset]() -> i32 { values.len + offset }`.

Bygg om FFI-kod som använder arrayer av `@c_layout`-structar. Avståndet i
byte mellan två element är nu `size_of(Typ)`, som i en C-array. Övervakade
HTTP-anrop behöver libcurl 7.66 eller senare för att kunna avbrytas även
under väntan på TLS eller svarshuvuden. [C-layout](ffi.md) · [HTTP-klienten](http.md)

## Installera en kompilator

För ARM64/macOS och för ARM64 och x86-64 med glibc på Linux behöver
installationen ett POSIX-skal, curl, gzip, awk och antingen
`sha256sum` eller `shasum`, utöver vanliga filverktyg.

```sh
curl -fsSLo install-dunder.sh https://raw.githubusercontent.com/Mik-pe/dunder/main/tools/install.sh && sh install-dunder.sh --release
export PATH="${DUNDER_HOME:-$HOME/.dunder}/bin:$PATH"
dunder --version
```

`--release` väljer senaste stabila version. För en bestämd tagg kan du
exempelvis köra `sh install-dunder.sh --release v0.11.3`. Skriptet kontrollerar
SHA-256 för arkivet och den uppackade binären samt att kompilatorn rapporterar
rätt version och värd innan den ersätter `dunder` i installationskatalogen.

### Windows x86-64

Ladda ner [Windowskompilatorn](https://github.com/Mik-pe/dunder/releases/latest/download/x86_64-windows.exe)
och [dess SHA-256](https://github.com/Mik-pe/dunder/releases/latest/download/x86_64-windows.sha256) från den senaste stabila releasen.
Kör följande i PowerShell i nedladdningskatalogen.

```powershell
$expected = ((Get-Content -LiteralPath .\x86_64-windows.sha256 -Raw).Trim() -split '\s+')[0]
if ((Get-FileHash -LiteralPath .\x86_64-windows.exe -Algorithm SHA256).Hash -ne $expected) { throw 'SHA-256 mismatch' }
.\x86_64-windows.exe --version
```

Versionsutskriften ska ange den valda versionen och `x86_64-windows`.
Lägg exe-filen som `dunder.exe` i en katalog på `PATH`. Med Git Bash går
det också att köra samma `install.sh --release` som på de andra värdarna.

`dunder upgrade` på Windows använder `curl.exe`, kontrollerar SHA-256 och
kör den hämtade filens `--version`. Om Windows håller den installerade filen
öppen och hindrar bytet får kommandot slutstatus 3. Det skriver ut sökvägar
och SHA-256 för den redan verifierade ersättningen. Stäng processerna som
använder den installerade filen. Ange sedan sökvägarna och hashen från
utskriften i PowerShell.

```powershell
$candidate = Read-Host 'Sökväg till verifierad ersättning'
$installed = Read-Host 'Sökväg till installerad kompilator'
$expected = Read-Host 'SHA-256'
if ((Get-FileHash -LiteralPath $candidate -Algorithm SHA256).Hash -ne $expected) { throw 'SHA-256 mismatch' }
$backup = Join-Path ([IO.Path]::GetDirectoryName($candidate)) 'previous.exe'
[IO.File]::Replace($candidate, $installed, $backup)
& $installed --version
```

Den tidigare kompilatorn ligger nu i `previous.exe` bredvid den angivna
ersättningsfilens ursprungliga plats.

## Bygg releasepaket

Bygg projektets release på ARM64/macOS. Ange versionsnumret från
`compiler_version()` i `selfhost/driver/cli.ddr`. Med följande kommando
hamnar de färdiga filerna i `build/release/v0.11.3/`.

```sh
./tools/release.sh v0.11.3
```

Med en lokal versionstagg byggs den commit som taggen pekar ut. Utan tagg
används `HEAD`. Skriptet packar upp de incheckade källorna i en tillfällig
katalog och kör `tools/bootstrap.sh` där. Macens kompilator måste nå
fixpunkt och rapportera versionen från taggen. Den korskompilerar sedan
Linux- och Windowskompilatorerna från samma ingång, `selfhost/main.ddr`.
Kör hela testsviten inför publicering från samma incheckade revision.
Det första kommandot provar även releasepaketeringen på macOS. De andra
två kör de korsbyggda kompilatorerna i Kiri.

```sh
./tools/verify-selfhost.sh --all
./tools/verify-selfhost.sh --linux arm64 --all
./tools/verify-selfhost.sh --linux x86_64 --all
```

Inför x86-64-proven kontrollerar verifieraren att ett program kan välja
`argv[0]` vid processbyte. Om Kiri använder Rosetta utan detta stöd
registrerar skriptet samma Rosetta med flaggan `P` i en privat
användar- och monteringsnamnrymd. Initieringen behöver `SYS_ADMIN` för
namnrymden och `SETFCAP` för rotanvändarens id-mappning. Alla
Linux-capabilities släpps före proven och containerns seccomp-filter är aktivt.
Med `release.sh`-flaggan `--outdir KATALOG` kan paketen skrivas till en annan katalog.

## Publicera

När bygget ska bli tillgängligt för `dunder upgrade`, använd Git med
skrivrättighet till repot och en autentiserad GitHub CLI.

```sh
./tools/release.sh v0.11.3 --publish
```

Om taggen saknades vid provbygget, sätt den på
commitvärdet i `release.txt` innan publiceringskommandot körs.

```sh
git tag v0.11.3 "$(sed -n 's/^commit=//p' build/release/v0.11.3/release.txt)"
```

En befintlig tagg på GitHub styr källrevisionen. Om taggen saknas skapar
skriptet den i `Mik-pe/dunder` på den byggda revisionen. Lokala annoterade
taggar behåller sin annotation. Därefter laddas filerna upp till ett
releaseutkast och hämtas tillbaka för bytejämförelse innan utkastet
publiceras. En version med förhandssuffix, exempelvis `v1.0.0-rc.1`,
publiceras som en förhandsrelease.

Efter ett avbrott kan samma kommando köras igen. För ett utkast fortsätter
uppladdningen med de filer som saknas eller skiljer sig. För en redan
publicerad release jämför skriptet de publicerade filerna med det lokala
bygget.

Skriptet skriver en kort standardbeskrivning. En förberedd versionstext kan
läggas i `build/release/v0.11.3/notes.md` och uppdateras separat:

```sh
gh release edit v0.11.3 --repo Mik-pe/dunder --notes-file build/release/v0.11.3/notes.md
```

## Filer som uppgraderaren läser

`release.txt` anger version och källcommit. Mål med gzip-arkiv har två SHA-256-värden i manifestet. Nyckeln `<mål>.gz.sha256` gäller gzip-arkivet och
`<mål>.sha256` gäller kompilatorns körbara fil, även för den direkt hämtbara Windowsfilen.

| Mål | Arkiv | Kontrollfil för arkivet |
| --- | --- | --- |
| ARM64 macOS | `arm64-macos.gz` | `arm64-macos.sha256` |
| ARM64 Linux | `arm64-linux.gz` | `arm64-linux.sha256` |
| x86-64 Linux | `x86_64-linux.gz` | `x86_64-linux.sha256` |
| x86-64 Windows | `x86_64-windows.exe` | `x86_64-windows.sha256` |

`dunder upgrade` läser manifestet från GitHubs senaste stabila release.
`dunder upgrade --version v0.11.3` väljer den namngivna releasen.

`dunder run tools/test_release.ddr` (eller `./tools/verify-selfhost.sh --release`) provar paketering, avbruten uppladdning och
omkörning i en lokal Git-kopia med simulerad GitHub CLI.

`dunder run tools/test_upgrade.ddr -- KOMPILATOR` provar den nativa uppgraderaren
mot lokala releasefiler. Både namngivna och senaste releaser måste ersätta
filen atomiskt, bevara symlänken och lämna den tidigare filens innehåll
orört för en redan öppen läsare. Även fixturens återställning använder en
ny tempfil och namnbyte: skrivning över en tidigare körd Mach-O-fil kan
lämna macOS signaturcache inkonsekvent. Se Apples
[Updating Mac Software](https://developer.apple.com/documentation/security/updating-mac-software).
