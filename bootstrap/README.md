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

Den verifierade kompilatorn bygger sedan `selfhost/main_parallel.ddr`.
Skriptet jämför den seriella kompilatorns resultat med den taskbaserade
kompilatorns självbyggen vid `--jobs 1` och `--jobs 4`. När alla tre binärer
är byteidentiska publiceras `build/selfhost/<värdmål>/dunder` atomiskt.
[Arbetsbudget och kompilatorjobb](../docs/tasks.md#kompilatorn-som-taskkonsument)

`--serial` publicerar enbart `build/selfhost/<värdmål>/dunder-serial`.
Båda kompilatoringångarna kan korskompilera till alla fyra mål.

```sh
serial_bin="$(./tools/bootstrap.sh --serial)"
"$serial_bin" run examples/hello.ddr
```

Återbyggnaden kan köras offline med POSIX-shell, gzip, en SHA-256-kontroll
och vanliga filverktyg. Linuxkompilatorerna använder systemets glibc.
Program som enbart använder Dunders native-runtime kan fortfarande byggas
som fristående ELF-binärer.

## Frönas ursprung

De tre fröna kommer från Dunder-källrevision `03b7462703454c399985af0c679325489d22e217`
(Git-träd `923291e37c2f11a5a7993a7440c812f4bbd25509`). Kör
`tools/bootstrap.sh --serial` på den revisionen för att reproducera
den uppackade kompilatorn från föregående frö. Fröna kan specialisera
generiska deklarationer, inklusive `Map<K, V>` som kompilatorn använder.
Komprimeringen är `gzip -9 -n`.
Tabellen anger SHA-256 för den **uppackade** kompilatorn. Syskonfilerna
`*.sha256` kontrollerar de komprimerade filerna.

| Värd | Ingångsfil | SHA-256 |
| --- | --- | --- |
| ARM64/macOS | `selfhost/main.ddr` | `a199afff3feca85f9b714a797bc7ffa4e8048f515fee326195dbe347ebc90399` |
| ARM64/Linux | `selfhost/linux.ddr` | `ea459b3fea6bdfa54fe3aba2730fdd3e4665f3260bda42d9f7dc4d67945b0b71` |
| x86-64/Linux | `selfhost/x86_linux.ddr` | `8cf7bc97f1d920778cbc5cd2f1c1fa41726a4c2a141099423e9add0bc99eda2c` |

Varje frö verifierades med byteidentiska självbyggen. Linuxfröna är
dessutom byteidentiska med macOS-kompilatorns korsbyggen. Linuxmålen
kördes i lokala Docker-containrar med x86-64 emulerat på ARM64.

För att utveckla från en ren utcheckning behövs det incheckade fröet för
värden. Den äldre Rust-implementationen och dess ursprungliga testsvit finns
i projekthistoriken vid [deb48dd](https://github.com/Mik-pe/dunder/tree/deb48dd).
