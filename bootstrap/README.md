# Återbygg Dunder

Kör från reporoten på ARM64/macOS, ARM64/Linux eller x86-64/Linux.

```sh
dunder_bin="$(./tools/bootstrap.sh)"
"$dunder_bin" run examples/hello.ddr
```

Skriptet kontrollerar SHA-256 för värddatorns frö och packar upp det. Fröet
bygger `selfhost/main.ddr`. Den nya kompilatorn bygger sedan samma källa med
automatiskt antal jobb. Om binärerna skiljer sig görs ett tredje bygge.
Två på varandra följande byggen måste vara byteidentiska innan skriptet
atomiskt ersätter `build/selfhost/<värdmål>/dunder`.
[Arbetsbudget och kompilatorjobb](../docs/tasks.md#kompilatorn-som-taskkonsument)

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
