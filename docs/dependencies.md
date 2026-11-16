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

## Välj en Git-version

En Git-källa får ha högst en av `branch`, `tag` och `rev`:

```toml
[dependencies]
gren = { git = "https://github.com/ditt-team/bibliotek.git", branch = "main" }
utgava = { git = "https://github.com/ditt-team/annat.git", tag = "v1.2.0" }
exakt = { git = "https://github.com/ditt-team/tredje.git", rev = "0123456789abcdef0123456789abcdef01234567" }
```

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

Git-källorna lagras under rotprojektets `build/.dunder/git`. Varje checkout
har en bestämd, frikopplad commit. Källorna ska inte redigeras där; använd
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
