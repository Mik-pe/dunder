# Rustfri återbyggnad

```sh
./tools/bootstrap.sh
build/selfhost/arm64-macos/dunder build examples/hello.ddr -o build/hello
./build/hello
```

Byt `arm64-macos` mot värdmålet som skriptet skriver ut. Skriptet behöver
POSIX-shell, gzip, en SHA-256-kontroll och vanliga filverktyg. Hela
återbyggnaden kan köras offline. Varje fryst kompilator har en kontrollsumma
för de komprimerade byten i samma katalog.

Skriptet bygger den aktuella Dunder-källan från den frysta binären och låter
resultatet bygga samma källor igen. När en ändring påverkar kompilatorns egen
kodgenerering kan det krävas ytterligare ett steg innan två efterföljande
byggen blir byteidentiska. Först därefter publiceras kompilatorn med atomiskt
namnbyte i `build/selfhost/<värdmål>/dunder`.

Fröna för `arm64-macos`, `arm64-linux` och `x86_64-linux` kommer från
källrevision `3077660`. Rust-bootstrapen byggde först respektive
`selfhost/main.ddr`, `selfhost/linux.ddr` och `selfhost/x86_linux.ddr`.
Den kompilatorn byggde stage 2, som byggde stage 3 från samma källor.
Stage 2 och stage 3 var byteidentiska per mål. Stage 3 byggde och körde
alla 59 grupperna i `dogfood/tests.ddr` på varje värdmål. Linux/x86-64
kördes med QEMU i Docker på ARM64. Filerna är komprimerade med `gzip -9 -n`.

Återbyggnaden har dessutom körts med dessa frön från rena källkopior,
med `PATH` utan Rust. Linux-fröna använder systemets glibc för
processdrivrutinen. Program som enbart använder Dunders native-runtime
behåller sin fristående ELF-körväg.

För återställning från enbart källkod finns Rust-kompilatorn i `src/`.
Se [selfhost](../selfhost/README.md) och [byggmätningarna](../benchmarks/README.md)
för kompilatorernas täckning och prestanda.
