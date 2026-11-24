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

De tre fröna kommer från Dunder-källrevision `07bfe9ebdd418d0c95dfb0d3402ff2c2aec21c5b`
(Git-träd `9d2298c38459679b57e2e9fbc97c68fc5c2b0eaa`). Kör
`./tools/bootstrap.sh` på den revisionen för att reproducera
den uppackade kompilatorn från föregående frö.
Komprimeringen är `gzip -9 -n`.
Tabellen anger SHA-256 för den **uppackade** kompilatorn. Syskonfilerna
`*.sha256` kontrollerar de komprimerade filerna.

| Värd | Ingångsfil | SHA-256 |
| --- | --- | --- |
| ARM64/macOS | `selfhost/main.ddr` | `ee06c063814ac1f3eeb22060592b97ddb9324baca8b3a0ca5ffc2c387a9d9964` |
| ARM64/Linux | `selfhost/main.ddr` | `c1b4acaab5aed7ccacdb5906070c3650ea2d5125a120d16435bb486934d115ca` |
| x86-64/Linux | `selfhost/main.ddr` | `e7646cc0da9086645fb307911b4e6998f31ac9ce5ef0a76ba4d2bebbf5968c60` |

Varje frö verifierades med byteidentiska självbyggen på ARM64/macOS samt för
ARM64/Linux och x86-64/Linux i Kiri. Linuxfröna är dessutom byteidentiska med
macOS-kompilatorns korsbyggen.
