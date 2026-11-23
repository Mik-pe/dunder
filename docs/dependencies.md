# Beroenden i `dunder.toml`

Ett projekt anger importerbara källbibliotek under `[dependencies]`:

```toml
name = "app"
root = "src/main.ddr"
test_root = "tests/suite.ddr"

[dependencies]
matematik = { git = "https://github.com/ditt-team/dunder-matematik.git" }
verktyg = { path = "../verktyg" }
```

Byt exempeladressen mot ett riktigt repo. Källan kan ligga på GitHub eller en
annan Git-server; HTTPS och SSH använder den installerade Git-klientens vanliga
autentisering. Git måste finnas i `PATH` när projektet har Git-beroenden.
Projekt med enbart lokala beroenden behöver inte Git.

Importnamnet är nyckeln i manifestet, exempelvis `import matematik`. Det måste
vara en Dunder-identifierare och får inte vara `std` eller `sys`. Bibliotekets
`dunder.toml` anger dess `root`, som kan vara en biblioteksmodul utan `main`:

```toml
root = "src/lib.ddr"
```

Biblioteksmanifest behöver inget `name`. `import matematik` läser rotmodulen;
`import matematik.algebra` läser `algebra.ddr` i rotmodulens katalog. Saknas
manifest kan katalogens moduler användas genom explicita submodulimporter.

## Kapabiliteter

Ett paket som deklarerar en kapabilitet i en importerad modul behöver få den
beviljad i det importerande paketets manifest. Ange den under `capabilities`
på beroendet. Exemplet ger biblioteket rätt att använda filsystemet.

```toml
[dependencies]
bildlasare = { path = "../bildlasare", capabilities = ["filesystem"] }
```

Tillåtna namn är `filesystem`, `network`, `process`, `stdio` och `unsafe`.
En utelämnad lista ger en tom uppsättning. För en transitiv import måste
kapabiliteten vara beviljad längs hela beroendekedjan.

## Välj en Git-version

En Git-källa får ha högst en av `branch`, `tag` och `rev`:

```toml
[dependencies]
gren = { git = "https://github.com/ditt-team/bibliotek.git", branch = "main" }
utgava = { git = "https://github.com/ditt-team/annat.git", tag = "v1.2.0" }
exakt = { git = "https://github.com/ditt-team/tredje.git", rev = "0123456789abcdef0123456789abcdef01234567" }
underpaket = { git = "https://github.com/ditt-team/monorepo.git", branch = "main", subdir = "packages/http" }
```

För monorepon kan `subdir` ange en relativ underkatalog inom Git-utcheckningen
där beroendets `dunder.toml` och källkod ligger (utan `..` eller ledande `/`).
Låset lagrar den lösta commiten och katalogen för samma Git-källa.
Utan väljare används repots standardgren. `rev` kan vara en Git-revision, men
en full commitidentifierare ger en bestämd källa redan i manifestet.
`path` och `git` kan inte kombineras. Lokala sökvägar är relativa till
manifestets katalog; den korta formen `verktyg = "../verktyg"` fungerar också.
Inline-tabeller skrivs på en rad. Värden kan vara vanliga dubbelt citerade
TOML-strängar eller enkelt citerade litterala strängar. Kommentarer utanför
strängarna börjar med `#`.

## Låsfil och uppdatering

Första lyckade upplösningen av hela beroendegrafen skriver `dunder.lock` i
rotprojektet. Låset innehåller Git-adress, väljare och full commitidentifierare
för varje direkt och transitiv Git-källa. Checka in filen tillsammans med
`dunder.toml`. Bibliotekens egna låsfiler används inte av konsumentprojektet.

Vanliga byggen behåller låsta commits även om grenen eller taggen flyttas:

```sh
dunder build
dunder check --locked
dunder test --offline --locked
dunder update
```

`dunder update` löser källorna på nytt och uppdaterar låsfilen utan att
kompilera programmet. En fixerad `rev` fortsätter peka på samma commit.
Ändrade eller borttagna krav återspeglas i låsfilen vid nästa upplösning.
Fel i manifest, saknade källor, beroendecykler eller namnkonflikter lämnar den
tidigare låsfilen orörd. Programutdata publiceras först efter ett lyckat bygge.

`--locked` avvisar en upplösning som behöver ändra låsfilen. En saknad låsfil
med Git-beroenden är också ett fel. Flaggan förbjuder inte nätverk; en låst
commit kan behöva hämtas om den inte redan finns lokalt.

`--offline` förbjuder hämtning och kräver att källan finns i den lokala cachen.
Flaggan kan kombineras med `--locked` och användas med `build`, `run`, `check`,
`test`, `build-tests` och `update`.

## Cache och transitiva beroenden

Git-källorna lagras i en delad cache under `$DUNDER_HOME/git` (eller
`~/.dunder/git` när `DUNDER_HOME` saknas); utan någon av miljövariablerna
används rotprojektets `build/.dunder/git`. Varje checkout har en bestämd,
frikopplad commit. Källorna ska inte redigeras där; använd
`path` för ett bibliotek som du utvecklar lokalt. En ändrad cache-checkout
avvisas. Cachen kan tas bort och återskapas från manifest och låsfil.
Samtidiga upplösningar i samma projekt samordnas innan cachen och låsfilen
publiceras.

Ett bibliotek kan ha egna `[dependencies]`, även blandningar av Git-källor
och lokala underkataloger. Lokala beroenden och manifestets `root` i en
Git-checkout måste stanna inom samma checkout, även genom symboliska länkar.
Alla importer går genom Dunders vanliga modul- och kapabilitetskontroll.
Upplösningen kör inga byggskript, Git-hooks eller submodulbyggen.

Beroenden har för närvarande en gemensam namnrymd för hela programmet. Två
krav med samma importnamn måste peka på samma källa och rotmodul. Olika
versioner behöver olika importnamn. Cykler mellan paket och motstridiga namn
ger fel vid manifestets rad innan kompileringen börjar.

## Körbara exempel

Kör `dunder --list-examples` för att visa ett namn per rad. Kommandot utgår
från närmaste `dunder.toml` och kan köras offline. Även `dunder run --example`
utan namn visar vilka exempel du kan välja.

Ett eget projekt under `examples/<namn>/dunder.toml` kan köras direkt från
huvudprojektet med `dunder run --example <namn>`. Katalognamnet är namnet
på kommandoraden. Dunder använder exempelprojektets `root`, beroenden,
kapabiliteter och låsfil. Argument efter `--` skickas till programmet.

För exempel som delar huvudprojektets manifest, ange rotfilerna under
`[examples]`. En uttrycklig post har företräde om en exempelprojektkatalog
har samma namn.

```toml
name = "app"
root = "src/main.ddr"
test_root = "tests/suite.ddr"

[dependencies]
verktyg = { path = "../verktyg" }

[examples]
atlas = "examples/atlas.ddr"
```

Välj exempel med `--example <namn>` eller `--example=<namn>` vid `build`, `run`,
`check` och `install`.

```sh
dunder run --example atlas -- arg1
dunder build --example atlas
dunder check --example atlas
```

En sökväg i `[examples]` tolkas relativt manifestets katalog och använder
det projektets beroendegraf och `dunder.lock` (inklusive `--offline` och `--locked`). Utan
`-o` skrivs utdata till `build/<mål>/examples/<namn>` (med `.wasm` för
`wasm32`/`wasm32-wasi`, `.exe` för `x86_64-windows` och `.app` med `--app`), så exempel med samma filnamn inte kolliderar
med varandra eller med huvudprogrammet. `root` och `test_root` fortsätter att
vara standardvalen när `--example` utelämnas.

Exempelnamn består av ASCII-bokstäver, siffror, `_` och `-` utan inledande
`-`. `--example` kan inte kombineras med en positionell `.ddr`-fil.

### Tab-komplettering i zsh

Lägg raden i `~/.zshrc`, eller kör den direkt i zsh för att aktivera
komplettering i det öppna skalet.

```zsh
eval "$(dunder shim zsh)"
```

Tryck Tab efter `dunder ` för att se kommandon. Efter `dunder run --example `
föreslår Tab exempel från projektet där du står. Skriv början av ett namn
och tryck Tab för att fylla i det. Samma komplettering finns för `build`,
`check` och `install`. Om `$SHELL` pekar på zsh kan du utelämna `zsh`
från `dunder shim zsh`.

## Körning, installation och tester

- **Binärnamn i `build/` från `dunder.toml`:** Utan `-o` döps projektets
  färdigbyggda binär under `build/<mål>/<namn>` efter paketets `name` i
  `dunder.toml` (med `.wasm` för `wasm32`/`wasm32-wasi`, `.exe` för
  `x86_64-windows` och `.app` med `--app`) i stället
  för källfilens stamnamn (`main`).
- **Arbetskatalog vid `dunder run`:** När `dunder run` startar den byggda
  binären körs programmet alltid i anroparens aktuella arbetskatalog (även vid
  körning av `dunder.toml`, `--example` eller en fil i en annan katalog), så
  relativa filsökvägar och argument tolkas likadant som när binären körs direkt.
- **`dunder install`:** Bygger projektets binär (eller ett valt `--example`
  respektive en positionell `.ddr`-fil) för värdplattformen och installerar den
  atomiskt som `<namn>` i målkatalogen. Utan `--dir` eller `--prefix` används
  samma katalog som `dunder`-binären (när den startats via `PATH` eller sökväg);
  med `--dir KATALOG` respektive `--prefix PREFIX` installeras den i `KATALOG`
  eller `PREFIX/bin`.
- **Testutdata i `dunder test`:** Som standard kör och sammanräknar
  `dunder test` bara testerna i det aktuella paketet/målfilerna (och hoppar över
  standardbibliotekets egna `@test`-funktioner, inklusive för inbäddad
  `selfhost/`- eller `stdlib/`-kod), såvida inte själva standardbiblioteket eller
  kompilatorn testas direkt.

[`Manifestparser`](../selfhost/driver/manifest.ddr) · [`CLI`](../selfhost/driver/cli.ddr)
