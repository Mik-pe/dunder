# Verktyg för lokala prestandamätningar

Den aktuella självhostade kompilatorn mäts med
[`tools/performance.ddr`](../tools/performance.ddr).
Den jämför två Dunder-kompilatorer med fyra runtimeprogram, ett genererat
byggkorpus, kompilatorns egen källkod och, med `--tabell`, även Tabell. Varje binärs resultat verifieras.
Verktyget sparar råtider, median, p95, binärstorlek och SHA-256.

```sh
dunder run tools/performance.ddr -- /path/to/baseline /path/to/candidate \
  /path/to/empty-report-directory --rounds 30 --c-compiler /usr/bin/clang
```

Med `--tabell /sökväg/till/dunder-hundmat/tabell` använder runtimesviten
CSV-programmet från Dunders testbädd som arbetslast. Indatan har 32 768 rader,
402 grupper, UTF-8 och citerade fält. Varje körnings utdata jämförs med en
separat summering och sortering.

`--suite runtime` och `--suite compiler` väljer en delmängd. `--include-check`
mäter även `check` för det genererade korpuset och kompilatorkällan. C-referensen
är valfri och byggs med `-O3 -ftrapv`. Kompilatorerna och runtimeprogrammen
körs med tom miljö och en PATH utan externa verktyg. Kör mätningarna utan
andra avsiktligt tunga arbeten på värddatorn.

`--baseline-flag=--scalar-opt=off` med samma nya binär som baseline och
kandidat jämför optimeringslägen utan att byta kompilatorbinär.

Kompilatormatrisen jämför två revisioner eller kör samma kompilator med
en, två och fyra workers. På ARM64/macOS kontrollerar `--validate-only`
binärer och körresultat utan att ta tids- eller minnesprover.

```sh
dunder run tools/performance_compiler_matrix.ddr -- /path/to/baseline \
  /path/to/candidate build/compiler-matrix --mode revision --include-check --validate-only
```

`--mode workers` kräver identiska kompilatorbinärer och ett rundantal
delbart med sex. Mätlägena sparar råprover, per-process-RSS och parade kvoter.

[Mätmetod och mål](../docs/prestanda.md) · [Daterade resultat och rådata](results.md)
