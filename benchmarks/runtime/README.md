# Körtidsmätningar

Sviten jämför två kompilatorer med samma fyra program. `numeric` använder
kontrollerad heltalsaritmetik, `scan` läser och ändrar en array, `text`
går igenom UTF-8 och `sort` sorterar heltal. Varje programs resultat
kontrolleras före tidsjämförelsen. `--c-compiler` lägger till
[C-referensen](reference.c).

```sh
dunder run tools/performance.ddr -- /sökväg/till/baseline /sökväg/till/kandidat \
  build/runtime-report --suite runtime --rounds 30 --c-compiler /usr/bin/clang
```

[Mätmetod och mål](../../docs/prestanda.md) · [Rapporten från 10 november med rådata och ursprungliga verktyg](https://github.com/Mik-pe/dunder/blob/e67fa3d887be84a99707c20d5266105c43edb93a/benchmarks/runtime/README.md)
