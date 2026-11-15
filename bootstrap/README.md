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

De tre fröna kommer från Dunder-källrevision `f5ad2ab52b3d58e8a8670df2635a0c80f4145296`.
De byggdes från föregående frön från revision `a1e4478`, med värdens
ingångsfil och två byteidentiska slutbyggen. Fröna förstår `mut`, `move`-uttryck, ägande parametrar och funktionstyper,
explicit `.clone()` och stränginterpolering. Bryggan behåller den tidigare
semantiken för att kunna bygga kompilatorn som inför de nya ägarskapsreglerna.
Komprimeringen är `gzip -9 -n`.
Tabellen anger SHA-256 för den **uppackade** kompilatorn. Syskonfilerna
`*.sha256` kontrollerar de komprimerade filerna.

| Värd | Ingångsfil | SHA-256 |
| --- | --- | --- |
| ARM64/macOS | `selfhost/main.ddr` | `12232c0b73e66a136c100621f3cc56e2abf72563670314c6763c96d37e6c12be` |
| ARM64/Linux | `selfhost/linux.ddr` | `90d821a633512e93f0b04866b83de9c8c2c5663ceb113f77f55ddd24fb43ee9e` |
| x86-64/Linux | `selfhost/x86_linux.ddr` | `8abfdc9016cb1772aff4b8473b8f52b8e604258b9116be46834f3a6bdb9eb104` |

Bryggkällan passerade 55 kompilatorgrupper, 15 lexergrupper, 17 nativa
målbilder och 12 syntaxfel samt seriell fixpunkt och byteidentiska
självbyggen med en och fyra workers på macOS. Båda Linux-värdarna
passerade seriell fixpunkt, syntaxfixturen och fem interpoleringsgrupper.
Linux/x86-64 kördes under emulering på ARM64. Varje frö kan bygga
kompilatorn och korskompilera program till alla fyra mål.

För att utveckla från en ren utcheckning behövs det incheckade fröet för
värden. Den äldre Rust-implementationen och dess ursprungliga testsvit finns
i projekthistoriken vid [deb48dd](https://github.com/Mik-pe/dunder/tree/deb48dd).
