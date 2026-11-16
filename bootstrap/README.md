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

De tre fröna kommer från Dunder-källrevision `0d40b9c045b4249d84235f95abbd97665433fa2c`
(Git-träd `226b6c270b092238b72a45a4a5806481d56b37fa`). macOS-kompilatorn
återbyggdes med föregående frysta macOS-frö från `7347cd6` och producerade
Linux-korsbilderna. Linuxmålen återbyggdes också direkt från sina föregående
fröarkiv. Varje mål verifierades med sin ingångsfil och byteidentiska
självbyggen. Fröna förstår lokala lexikala läslån, lånade array-/textintervall
och temporära lån vid anrop, utöver partiella structfältflyttar,
`replace(&mut place, replacement)`, `mut`, `move`, `@resource`, uttrycklig
`.clone()`, `array.swap` och stränginterpolering. Därmed kan självbyggkedjan
både överföra färdiga kod- och metadataarenor utan kloning och läsa
fragmentcachens tokenstavningar och byteintervall genom lån. Den opaque
`Session` behöver inte öppnas för andra moduler.
Komprimeringen är `gzip -9 -n`.
Tabellen anger SHA-256 för den **uppackade** kompilatorn. Syskonfilerna
`*.sha256` kontrollerar de komprimerade filerna.

| Värd | Ingångsfil | SHA-256 |
| --- | --- | --- |
| ARM64/macOS | `selfhost/main.ddr` | `148950f5ea666eacc3eff94c966a36a346fb53e1877d83517edb15fc1831214a` |
| ARM64/Linux | `selfhost/linux.ddr` | `ef5005604098e21c6277ba7759dd8f8bf42b00220d7274b5e324f46ef3c2cc7f` |
| x86-64/Linux | `selfhost/x86_linux.ddr` | `0598b64da6bb2bd1d4927bc958c8cdcd7b0a9b6e23c09ae492fab6d973d526c0` |

Varje frö passerade 14 ägartillståndsgrupper, 18 lånegrupper och 14 grupper
för escapegrafen. Den riktade ägarskapssviten gav per frö 30 målbilder,
10 egna nativekörningar och 90 avvisade kontrakt. Lånesviten gav 129 bilder,
45 körningar och 105 avvisningar på macOS; respektive Linuxfrö gav
126 bilder, 42 körningar och 102 avvisningar. Proven täcker fältflyttar,
`replace`, cleanup, inline C-/POD-lagring, originaladresser utan extra
allokering eller retain, intervall/UTF-8 och index-/utvärderingsordning
med optimeringar på och av.
Linuxfröna är byteidentiska med macOS-kompilatorns korsproducerade bilder
och två efterföljande självbyggen på respektive Linux-värd. Linuxmålen
kördes i lokala Docker-containrar; x86-64 emulerades på ARM64. macOS-fröets
fixpunkt verifierades även med ett separat självbygge. Varje frö kan bygga
kompilatorn och korskompilera program till alla fyra mål.

För att utveckla från en ren utcheckning behövs det incheckade fröet för
värden. Den äldre Rust-implementationen och dess ursprungliga testsvit finns
i projekthistoriken vid [deb48dd](https://github.com/Mik-pe/dunder/tree/deb48dd).
