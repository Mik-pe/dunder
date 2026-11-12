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

De tre fröna kommer från Dunder-källrevision `28fd7d768b7e8fbcfdb9746f91e8daffbe96fac9`.
De byggdes med föregående frön från revision `1c15f7f`, med värdens
ingångsfil och två byteidentiska slutbyggen. Komprimeringen är `gzip -9 -n`.
Tabellen anger SHA-256 för den **uppackade** kompilatorn. Syskonfilerna
`*.sha256` kontrollerar de komprimerade filerna.

| Värd | Ingångsfil | SHA-256 |
| --- | --- | --- |
| ARM64/macOS | `selfhost/main.ddr` | `3daff87390f0716657552a6b38d60406724ab31f0e4098c8d9af10aa2bf60314` |
| ARM64/Linux | `selfhost/linux.ddr` | `ffbed9858c2d286aff7d352c11fed1dc9b4a3433906413363e3b781b4b6d422e` |
| x86-64/Linux | `selfhost/x86_linux.ddr` | `09ae3e8a9a5b55dc60deb72f4ccccf3a43f0ceede1ea886f0af63796dee8aa1f` |

Varje värd körde 59 dogfood-grupper och Tabells 14 grupper. Linux/x86-64
kördes under emulering på ARM64. Varje frö kan bygga kompilatorn och
korskompilera program till alla fyra mål.

För att utveckla från en ren utcheckning behövs det incheckade fröet för
värden. Den äldre Rust-implementationen och dess ursprungliga testsvit finns
i projekthistoriken vid [deb48dd](https://github.com/Mik-pe/dunder/tree/deb48dd).
