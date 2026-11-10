# Återbygg Dunder

Kör från reporoten på ARM64/macOS, ARM64/Linux eller x86-64/Linux.

```sh
dunder_bin="$(./tools/bootstrap.sh)"
"$dunder_bin" run examples/hello.ddr
```

Skriptet väljer ett fryst Dunder-frö för värddatorn, kontrollerar den
komprimerade filens SHA-256 och bygger källorna i `selfhost/`. Den nya
kompilatorn bygger samma källor igen. När kodgeneratorn har ändrats kan
det behövas ytterligare ett bygge för att nå fixpunkten. Två efterföljande
binärer måste vara byteidentiska innan resultatet publiceras med atomiskt
namnbyte i `build/selfhost/<värdmål>/dunder`.

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
