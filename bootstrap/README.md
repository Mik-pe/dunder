# Återbygg Dunder

Kör från reporoten på ARM64/macOS, ARM64/Linux eller x86-64/Linux.

```sh
dunder_bin="$(./tools/bootstrap.sh)"
"$dunder_bin" run examples/hello.ddr
```

Skriptet väljer ett fryst Dunder-frö för värddatorn, kontrollerar den
komprimerade filens SHA-256 och bygger den seriella ingången i `selfhost/`.
Den nya kompilatorn bygger samma källor igen. När kodgeneratorn har ändrats
kan ytterligare ett bygge behövas; två efterföljande binärer måste vara
byteidentiska innan den seriella fixpunkten godtas.

På ARM64/macOS bygger denna verifierade kompilator sedan
`selfhost/main_parallel.ddr`. Den taskbaserade kompilatorn bygger sig själv
med en respektive fyra workers. Slutbyggena måste vara byteidentiska både
mellan workerantal och mellan efterföljande självbyggen. Först då publiceras
standardbinären med atomiskt namnbyte i `build/selfhost/arm64-macos/dunder`.
Dess standard är fyra workers; `--jobs=1` väljer den delade seriella
Session-vägen. Fröet behöver aldrig förstå tasksyntax: den ingår bara i det
senare byggstegets beroendeträd.

Den verifierade seriella macOS-kompilatorn behålls som `dunder-serial`.
För att enbart bygga den seriella återställningsvägen på någon av de tre
värdplattformarna:

```sh
serial_bin="$(./tools/bootstrap.sh --serial)"
"$serial_bin" run examples/hello.ddr
```

`--serial` publicerar och returnerar
`build/selfhost/<värdmål>/dunder-serial` och ersätter aldrig standardbinären
`dunder`. Utan flaggan fortsätter Linux att publicera den seriella
fixpunkten som `dunder`; taskruntimen stöder tills vidare ARM64/macOS.
Båda kompilatoringångarna kan korskompilera till alla fyra mål.

Återbyggnaden kan köras offline med POSIX-shell, gzip, en SHA-256-kontroll
och vanliga filverktyg. Linuxkompilatorerna använder systemets glibc.
Program som enbart använder Dunders native-runtime kan fortfarande byggas
som fristående ELF-binärer.

## Frönas ursprung

De tre fröna kommer från Dunder-källrevision `a1e4478f8906c1d70f159051fd603f64b59fb05e`.
De byggdes från föregående frön från revision `28fd7d7`, med värdens
ingångsfil och två byteidentiska slutbyggen. Fröna förstår explicit `.clone()`
och stränginterpolering, så att kompilatorns källkod kan migreras till
uttryckliga kopior utan att förlora återbyggnadskedjan.
Komprimeringen är `gzip -9 -n`.
Tabellen anger SHA-256 för den **uppackade** kompilatorn. Syskonfilerna
`*.sha256` kontrollerar de komprimerade filerna.

| Värd | Ingångsfil | SHA-256 |
| --- | --- | --- |
| ARM64/macOS | `selfhost/main.ddr` | `6b5a0782b4b7058084e5d026a7aa890b87d1af4b7360ddb00afc26fc7577b4fa` |
| ARM64/Linux | `selfhost/linux.ddr` | `9fd6183f8c7e874b7bb90b31732212199352f6584847c1fd626e6d4f42b1770c` |
| x86-64/Linux | `selfhost/x86_linux.ddr` | `57e2e4e21e24c1f2f4bdc66a14ef23ed23b21802ecb7d5e4763a96fe2159cc54` |

macOS passerade den fullständiga verifieringens 81 sviter och byteidentiska
självbyggen med en och fyra workers. Båda Linux-värdarna passerade seriell
fixpunkt, fem interpoleringsgrupper och elva process-/biblioteksgrupper.
Linux/x86-64 kördes under emulering på ARM64. Varje frö kan bygga
kompilatorn och korskompilera program till alla fyra mål.

För att utveckla från en ren utcheckning behövs det incheckade fröet för
värden. Den äldre Rust-implementationen och dess ursprungliga testsvit finns
i projekthistoriken vid [deb48dd](https://github.com/Mik-pe/dunder/tree/deb48dd).
