# Lokala kompilatorbyggen för GitHub Releases

Ange versionsnumret från `compiler_version()` i `selfhost/cli.ddr`. Med
följande kommando hamnar de färdiga filerna i `build/release/v0.1.0/`.

```sh
./tools/release.sh v0.1.0
```

Med en lokal versionstagg byggs den commit som taggen pekar ut. Utan tagg
används `HEAD`. Skriptet packar upp de incheckade källorna i en tillfällig
katalog och kör `tools/bootstrap.sh` där. Värdens kompilator måste nå
fixpunkt och rapportera versionen från taggen. Den bygger sedan de andra
målens kompilatorer lokalt. macOS-paketet innehåller den parallella
kompilatorn. För en annan utdatakatalog används `--outdir KATALOG`.

## Publicera

När bygget ska bli tillgängligt för `dunder upgrade`, använd Git med
skrivrättighet till repot och en autentiserad GitHub CLI.

```sh
./tools/release.sh v0.1.0 --publish
```

Om taggen saknades vid provbygget, sätt den på
commitvärdet i `release.txt` innan publiceringskommandot körs.

```sh
git tag v0.1.0 "$(sed -n 's/^commit=//p' build/release/v0.1.0/release.txt)"
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
`dunder upgrade --version v0.1.0` väljer den namngivna releasen.

`python3 tools/test-release.py` provar paketering, avbruten uppladdning och
omkörning i en lokal Git-kopia med simulerad GitHub CLI.
