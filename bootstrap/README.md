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

De tre fröna kommer från Dunder-källrevision `f15e9f3e47805c952717d311b45a5eeefa8994a9`.
De byggdes från de frysta syntaxbryggorna från revision
`f5ad2ab52b3d58e8a8670df2635a0c80f4145296`, med värdens ingångsfil och
två byteidentiska slutbyggen. De använder ägarflyttar och lånade
standardparametrar och förstår `mut`, `move`, `@resource`, uttrycklig
`.clone()`, `array.swap` och stränginterpolering. Uppdateringen låter även
kompilatorns egen tidiga byggkedja använda klonfria arraybyten.
Komprimeringen är `gzip -9 -n`.
Tabellen anger SHA-256 för den **uppackade** kompilatorn. Syskonfilerna
`*.sha256` kontrollerar de komprimerade filerna.

| Värd | Ingångsfil | SHA-256 |
| --- | --- | --- |
| ARM64/macOS | `selfhost/main.ddr` | `8783408c28c43e79cb306de1b8834ba697da7fe1aea1fef004965b6c46663ec1` |
| ARM64/Linux | `selfhost/linux.ddr` | `ef87067be99a8691232723e1b8de23c385c2eba6f17fc5a959338cbfcb21d1fa` |
| x86-64/Linux | `selfhost/x86_linux.ddr` | `a3ebec87f920785fec0b3df9c63e2954c543779d96048ac06b1e0161bb13aab9` |

Frökällan passerade 86 verifieringssviter samt byteidentiska självbyggen
med en och fyra workers på macOS. Båda Linux-värdarna passerade seriell
fixpunkt, 143 testgrupper och de tre nya ägarskapssviterna. Linuxfröna är
även byteidentiska med macOS-kompilatorns korsproducerade bilder.
Linux/x86-64 kördes under emulering på ARM64. Varje frö kan bygga
kompilatorn och korskompilera program till alla fyra mål.

För att utveckla från en ren utcheckning behövs det incheckade fröet för
värden. Den äldre Rust-implementationen och dess ursprungliga testsvit finns
i projekthistoriken vid [deb48dd](https://github.com/Mik-pe/dunder/tree/deb48dd).
