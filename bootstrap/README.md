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

De tre fröna kommer från Dunder-källrevision `7748bcaaf47a7cb2b88526307ababaee7b30d39f`
(Git-träd `7d00db25b37b5c9ece839132020d413ed2b9bc72`). Kör
`./tools/bootstrap.sh` på den revisionen för att reproducera
den uppackade kompilatorn från föregående frö.
Komprimeringen är `gzip -9 -n`.
Tabellen anger SHA-256 för den **uppackade** kompilatorn. Syskonfilerna
`*.sha256` kontrollerar de komprimerade filerna.

| Värd | Ingångsfil | SHA-256 |
| --- | --- | --- |
| ARM64/macOS | `selfhost/main.ddr` | `051d0aeadbada6dabe737f5bfeb64fc7fdcd4047f7fd615749938145a896d54a` |
| ARM64/Linux | `selfhost/main.ddr` | `04394f916a92a889f3438847eadcbf90e25ed0c99bde55da5f9c9aa0a8909fd8` |
| x86-64/Linux | `selfhost/main.ddr` | `f492e85d6dea253e85c850d021c231f8dbc0c3f4059e5de815c4537c41a5f722` |

Varje frö verifierades med byteidentiska självbyggen på ARM64/macOS samt för
ARM64/Linux och x86-64/Linux i Kiri. Linuxfröna är dessutom byteidentiska med
macOS-kompilatorns korsbyggen.
