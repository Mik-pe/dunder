# Distribuera Dunder

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
exempelvis köra `sh install-dunder.sh --release v0.9.0`. Skriptet kontrollerar
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
hamnar de färdiga filerna i `build/release/v0.9.0/`.

```sh
./tools/release.sh v0.9.0
```

Med en lokal versionstagg byggs den commit som taggen pekar ut. Utan tagg
används `HEAD`. Skriptet packar upp de incheckade källorna i en tillfällig
katalog och kör `tools/bootstrap.sh` där. Macens kompilator måste nå
fixpunkt och rapportera versionen från taggen. Den korskompilerar sedan
Linux- och Windowskompilatorerna från samma ingång, `selfhost/main.ddr`.
Linuxproven i Kiri kör Macens artefakter för ARM64 och x86-64.
Inför x86-64-proven kontrollerar skriptet att ett program kan välja
`argv[0]` vid processbyte. Om Kiri använder Rosetta utan detta stöd
registrerar skriptet samma Rosetta med flaggan `P` i en privat
användar- och monteringsnamnrymd. Initieringen behöver `SYS_ADMIN` för
namnrymden och `SETFCAP` för rotanvändarens id-mappning. Alla
Linux-capabilities släpps före proven och containerns seccomp-filter är aktivt.
Med `--outdir KATALOG` kan paketen skrivas till en annan katalog.

## Publicera

När bygget ska bli tillgängligt för `dunder upgrade`, använd Git med
skrivrättighet till repot och en autentiserad GitHub CLI.

```sh
./tools/release.sh v0.9.0 --publish
```

Om taggen saknades vid provbygget, sätt den på
commitvärdet i `release.txt` innan publiceringskommandot körs.

```sh
git tag v0.9.0 "$(sed -n 's/^commit=//p' build/release/v0.9.0/release.txt)"
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
läggas i `build/release/v0.9.0/notes.md` och uppdateras separat:

```sh
gh release edit v0.9.0 --repo Mik-pe/dunder --notes-file build/release/v0.9.0/notes.md
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
`dunder upgrade --version v0.9.0` väljer den namngivna releasen.

`dunder run tools/test_release.ddr` (eller `./tools/verify-selfhost.sh --release`) provar paketering, avbruten uppladdning och
omkörning i en lokal Git-kopia med simulerad GitHub CLI.

`dunder run tools/test_upgrade.ddr -- KOMPILATOR` provar den nativa uppgraderaren
mot lokala releasefiler. Både namngivna och senaste releaser måste ersätta
filen atomiskt, bevara symlänken och lämna den tidigare filens innehåll
orört för en redan öppen läsare. Även fixturens återställning använder en
ny tempfil och namnbyte: skrivning över en tidigare körd Mach-O-fil kan
lämna macOS signaturcache inkonsekvent. Se Apples
[Updating Mac Software](https://developer.apple.com/documentation/security/updating-mac-software).
