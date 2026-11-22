# Distribuera Dunder

## Installera en kompilator

Färdiga kompilatorer finns för ARM64/macOS och för ARM64 och x86-64 med glibc
på Linux. Installationen behöver ett POSIX-skal, curl, gzip, awk och antingen
`sha256sum` eller `shasum`, utöver vanliga filverktyg.

```sh
curl -fsSLo install-dunder.sh https://raw.githubusercontent.com/Mik-pe/dunder/main/tools/install.sh && sh install-dunder.sh --release
export PATH="${DUNDER_HOME:-$HOME/.dunder}/bin:$PATH"
dunder --version
```

`--release` väljer senaste stabila version. För en bestämd tagg kan du
exempelvis köra `sh install-dunder.sh --release v0.8.0`. Skriptet kontrollerar
SHA-256 för arkivet och den uppackade binären samt att kompilatorn rapporterar
rätt version och värd innan den ersätter `dunder` i installationskatalogen.

## Bygg releasepaket

Ange versionsnumret från `compiler_version()` i `selfhost/driver/cli.ddr`. Med
följande kommando hamnar de färdiga filerna i `build/release/v0.8.0/`.

```sh
./tools/release.sh v0.8.0
```

Med en lokal versionstagg byggs den commit som taggen pekar ut. Utan tagg
används `HEAD`. Skriptet packar upp de incheckade källorna i en tillfällig
katalog och kör `tools/bootstrap.sh` där. Värdens kompilator måste nå
fixpunkt och rapportera versionen från taggen. Den bygger sedan de andra
målens kompilatorer lokalt. Alla tre paketen innehåller den parallella
kompilatorn (`selfhost/main.ddr`). För en annan utdatakatalog används `--outdir KATALOG`.

## Publicera

När bygget ska bli tillgängligt för `dunder upgrade`, använd Git med
skrivrättighet till repot och en autentiserad GitHub CLI.

```sh
./tools/release.sh v0.8.0 --publish
```

Om taggen saknades vid provbygget, sätt den på
commitvärdet i `release.txt` innan publiceringskommandot körs.

```sh
git tag v0.8.0 "$(sed -n 's/^commit=//p' build/release/v0.8.0/release.txt)"
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
läggas i `build/release/v0.8.0/notes.md` och uppdateras separat:

```sh
gh release edit v0.8.0 --repo Mik-pe/dunder --notes-file build/release/v0.8.0/notes.md
```

## Filer som uppgraderaren läser

`release.txt` anger version och källcommit. Varje mål har två SHA-256-värden
i manifestet. Nyckeln `<mål>.gz.sha256` gäller gzip-arkivet och
`<mål>.sha256` gäller den uppackade kompilatorn.

| Mål | Arkiv | Kontrollfil för arkivet |
| --- | --- | --- |
| ARM64 macOS | `arm64-macos.gz` | `arm64-macos.sha256` |
| ARM64 Linux | `arm64-linux.gz` | `arm64-linux.sha256` |
| x86-64 Linux | `x86_64-linux.gz` | `x86_64-linux.sha256` |

`dunder upgrade` läser manifestet från GitHubs senaste stabila release.
`dunder upgrade --version v0.8.0` väljer den namngivna releasen.

`dunder run tools/test_release.ddr` (eller `./tools/verify-selfhost.sh --release`) provar paketering, avbruten uppladdning och
omkörning i en lokal Git-kopia med simulerad GitHub CLI.

`dunder run tools/test_upgrade.ddr -- KOMPILATOR` provar den nativa uppgraderaren
mot lokala releasefiler. Både namngivna och senaste releaser måste ersätta
filen atomiskt, bevara symlänken och lämna den tidigare filens innehåll
orört för en redan öppen läsare. Även fixturens återställning använder en
ny tempfil och namnbyte: skrivning över en tidigare körd Mach-O-fil kan
lämna macOS signaturcache inkonsekvent. Se Apples
[Updating Mac Software](https://developer.apple.com/documentation/security/updating-mac-software).
