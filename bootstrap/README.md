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

De tre fröna kommer från Dunder-källrevision `119b5b09e4dfdf2f1353fc833ee54acaec1f06af`
(Git-träd `ac8b3db892858caed5d6f0f7143b70ceaedda772`). Kör
`tools/bootstrap.sh --serial` på den revisionen för att reproducera
den uppackade kompilatorn från föregående frö.
Komprimeringen är `gzip -9 -n`.
Tabellen anger SHA-256 för den **uppackade** kompilatorn. Syskonfilerna
`*.sha256` kontrollerar de komprimerade filerna.

| Värd | Ingångsfil | SHA-256 |
| --- | --- | --- |
| ARM64/macOS | `selfhost/main.ddr` | `b7922eee39025679a8285d2554c92d62700416bcde34ed5cc3fd52cbfdc47c61` |
| ARM64/Linux | `selfhost/linux.ddr` | `8626b0268dee2ea52591770904887efa9fb79f95aeb178a24401ef14a37f35a5` |
| x86-64/Linux | `selfhost/x86_linux.ddr` | `01922963236fcea0bf4b82ba7fb39fc8305e179892eedd5146eb5c284647a8ee` |

Varje frö verifierades med byteidentiska självbyggen. Linuxfröna är
dessutom byteidentiska med macOS-kompilatorns korsbyggen. Linuxmålen
kördes i lokala Docker-containrar med x86-64 emulerat på ARM64.
