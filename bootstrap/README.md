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

De tre fröna kommer från Dunder-källrevision `7347cd62d081139b7cd0c47a21b86b27e61803fe`
(Git-träd `55a8361918dd5eff994162c14da0c9a96f70aa18`). macOS-kompilatorn
återbyggdes med föregående frysta macOS-frö från `f15e9f3` och producerade
Linux-korsbilderna. Varje mål verifierades sedan med sin ingångsfil och
byteidentiska självbyggen. Fröna använder ägarflyttar och lånade
standardparametrar och förstår partiella structfältflyttar och
`replace(&mut place, replacement)`, utöver `mut`, `move`, `@resource`,
uttrycklig `.clone()`, `array.swap` och stränginterpolering. Därmed kan den
tidiga självbyggkedjan överföra färdiga kod- och metadataarenor utan kloning
och utan att öppna kompilatorns opaque `Session` för andra moduler.
Komprimeringen är `gzip -9 -n`.
Tabellen anger SHA-256 för den **uppackade** kompilatorn. Syskonfilerna
`*.sha256` kontrollerar de komprimerade filerna.

| Värd | Ingångsfil | SHA-256 |
| --- | --- | --- |
| ARM64/macOS | `selfhost/main.ddr` | `72a911c59318821c7d9b301c5ab429609282e4096356dad9698d6828e87443e3` |
| ARM64/Linux | `selfhost/linux.ddr` | `bfa1fe3d8cb4eb050974d247fee0bf084adf7cd93609a8449d9671da5ec961e8` |
| x86-64/Linux | `selfhost/x86_linux.ddr` | `8d2d72139b69879a7248c5aeff0c1bdc77aabdd43b282b2db65c1fdcd289b92f` |

Varje frö passerade 13 ägartillståndsgrupper samt 30 målbilder, 10 egna
nativekörningar och 90 avvisade kontrakt i den riktade ägarskapssviten.
Den täcker fältflyttar, `replace`, begränsad cleanup, inline C-/POD-lagring
och index-/utvärderingsordning med optimeringar på och av.
Linuxfröna är byteidentiska med macOS-kompilatorns korsproducerade bilder
och två efterföljande självbyggen på respektive Linux-värd. Linuxmålen
kördes i lokala Docker-containrar; x86-64 emulerades på ARM64. macOS-fröets
fixpunkt verifierades även med ett separat självbygge. Varje frö kan bygga
kompilatorn och korskompilera program till alla fyra mål.

För att utveckla från en ren utcheckning behövs det incheckade fröet för
värden. Den äldre Rust-implementationen och dess ursprungliga testsvit finns
i projekthistoriken vid [deb48dd](https://github.com/Mik-pe/dunder/tree/deb48dd).
