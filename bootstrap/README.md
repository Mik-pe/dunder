# Återbygg Dunder

Kör från reporoten på ARM64/macOS, ARM64/Linux eller x86-64/Linux.

```sh
dunder_bin="$(./tools/bootstrap.sh)"
"$dunder_bin" run examples/hello.ddr
```

Skriptet väljer ett fryst Dunder-frö för värddatorn, kontrollerar den
komprimerade filens SHA-256 och bygger den seriella ingången i `selfhost/`.
Den seriella kompilatorn bygger både sin egen ingång och
`selfhost/main_parallel.ddr`. Den parallella kompilatorn bygger sedan om sig
själv med `--jobs 4`. Resultatet måste vara byteidentiskt med den seriella
kompilatorns bygge av samma ingång. Om de första två seriella binärerna
skiljer sig bygger den parallella kompilatorn även den seriella ingången
och jämför resultatet med det senaste seriella bygget. Efter godkända
jämförelser publiceras `build/selfhost/<värdmål>/dunder` atomiskt.
[Arbetsbudget och kompilatorjobb](../docs/tasks.md#kompilatorn-som-taskkonsument)

`--serial` kontrollerar fixpunkten med enbart seriella självbyggen och publicerar `build/selfhost/<värdmål>/dunder-serial`.
Båda kompilatoringångarna kan korskompilera till alla sex mål.

```sh
serial_bin="$(./tools/bootstrap.sh --serial)"
"$serial_bin" run examples/hello.ddr
```

Återbyggnaden kan köras offline med POSIX-shell, gzip, en SHA-256-kontroll
och vanliga filverktyg. Linuxkompilatorerna använder systemets glibc.
Program som enbart använder Dunders native-runtime kan fortfarande byggas
som fristående ELF-binärer.

## Frönas ursprung

De tre fröna kommer från Dunder-källrevision `030ff7d3f704e99fc1acccc22347364b1083ba1b`
(Git-träd `6c1e6b46e5c73db9367da620fa64be49b202a4d4`). Kör
`tools/bootstrap.sh --serial` på den revisionen för att reproducera
den uppackade kompilatorn från föregående frö.
Komprimeringen är `gzip -9 -n`.
Tabellen anger SHA-256 för den **uppackade** kompilatorn. Syskonfilerna
`*.sha256` kontrollerar de komprimerade filerna.

| Värd | Ingångsfil | SHA-256 |
| --- | --- | --- |
| ARM64/macOS | `selfhost/main.ddr` | `cd3c550f8f4d2ea7da29d5e1aa23f584eae055dba2d8718ff72a1aa681809efe` |
| ARM64/Linux | `selfhost/linux.ddr` | `0bda3418d84cd78e402cbf06d077570ca1af15936d10b98c0477e9bb5ed72ce2` |
| x86-64/Linux | `selfhost/x86_linux.ddr` | `e4d39815d6343252cf0f9c7f7677b58bf821c51292258941edbe70bf0fe9bc6f` |

Varje frö verifierades med byteidentiska självbyggen. Linuxfröna är
dessutom byteidentiska med macOS-kompilatorns korsbyggen. Linuxmålen
kördes i lokala Docker-containrar med x86-64 emulerat på ARM64.
