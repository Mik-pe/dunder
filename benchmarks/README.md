# Lokala byggmätningar

Mätverktyget är [tools/compile-bench.rs](../tools/compile-bench.rs), ett separat
Rust/std-program för mätning utanför kompilatorprocessen. Det ingår inte i
Dunders byggväg. Enligt [agentregeln](../AGENTS.md) körs ingen GitHub-CI.

```sh
cargo build --release
rustc --edition=2021 -O tools/compile-bench.rs -o build/compile-bench
./build/compile-bench /path/to/dunder compiler.csv 30
```

Verktyget skapar ett genererat stresskorpus med 21 moduler, 1 001 funktioner,
101 024 fysiska rader med språktoken och 2 216 254 källbyte. Varje hjälpfunktion
anropas av main och bidrar till resultatet. Blankrader och kommentarer ingår
inte. Korpuset har många upprepade heltalsoperationer och representerar inte
hela språket eller kompilatorns egen källkod.

Varje försök startar en ny `dunder build`-process, läser alla moduler, gör
typkontroll, kodgenerering och signering samt publicerar en ny utdatafil.
Klockan stannar när processen har avslutats. Binären körs därefter med tom
miljö och måste skriva `595500`; felaktigt resultat underkänner försöket.
Körningen ingår inte i byggtiden. Kompilatorns PATH saknar externa verktyg.
Det finns ingen Dunder-cache, OS-filcachen är varm och en arbetare används.

Rapportera råvärden, median, nearest-rank p95, compiler/source revision,
binärchecksumma, maskin, OS och byggflaggor tillsammans. Även första försöket
ingår i fördelningen. Detta prov visar genomströmning för genererad stresskod.
Det uppfyller inte ensamt [hela prestandakontraktet](../docs/prestanda.md):
representativa moduler med samlingar/lån, C/C++-jämförelse, kall OS-cache,
inkrementella arbetsfall och ett verkligt självhostat kompilatorbygge återstår.
