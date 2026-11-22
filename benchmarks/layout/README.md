# Rörelse i rader och kolumner

Två Dunderprogram flyttar 4 099 punkter i två dimensioner. I `rows.ddr`
ligger position och hastighet i samma post. I `columns.ddr` har varje fält
en egen växande array. En uppdatering läser två hastigheter, multiplicerar
med samma tidssteg och adderar till två positioner för varje punkt.
Punkterna och koordinaterna är oberoende inom ett tidssteg. Antalet punkter
ger även en svans för vektorer med två, fyra eller åtta element.

Första argumentet väljer antal tidssteg och det andra väljer startvärden.
Efter varje steg vägs en punkts position in i kontrollsumman. Efter sista
steget vägs samtliga punkter in. Med argumenten `101 1729` är summan
`16774246`. Mätverktyget räknar själv ut svaret med heltal. Tidsstegen
upprepas som 1, −2, 1, så den sammanlagda förflyttningen går att räkna ut
direkt från startvärdet även mitt i en period. Alla flyttalsvärden i
denna arbetslast är exakt representerbara som `f32`.

## Reproducera

Kör från repots rot med sökvägen till den kompilator som ska mätas.

```sh
d="$PWD/build/selfhost/arm64-macos/dunder"
"$d" run tools/performance_layout.ddr -o build/performance-layout -- "$d" 30 > layout.csv
"$d" test benchmarks/layout/columns.ddr
```

Verktyget bygger och kör varje variant i egna processer, med samma
kompilator och en byggarbetare. Den första omgången värmer filcachen och
kontrollerar också noll till tre steg med ett annat startvärde. Därefter
växlar körordningen mellan omgångarna. Varje redovisad byggtid omfattar
ett nytt bygge och varje körtid omfattar processstart, skapande av arrayer,
alla tidssteg och kontrollsumman. Utdata är CSV med en rad per prov.

För att jämföra den genererade koden kan man bygga båda varianterna med
`-g` och inspektera funktionen `advance` på respektive mål. C-referensen
har separata buffertar och samma uppdatering med `restrict`. Den kan också
kontrollera om en annan kompilator vektoriserar just punktloopen.

```sh
clang -O3 -ffp-contract=off -Rpass=loop-vectorize benchmarks/layout/reference.c -o build/layout-ref
build/layout-ref 101 1729
```

Kolumnprogrammet returnerar en lånad vy över x-positionerna. Testet i
`columns.ddr` lägger till en punkt efter vyns sista läsning.
`tools/test_local_borrows.ddr` kontrollerar att tillväxt före sista
läsningen ger kompileringsfel. Samma två fall gäller om radtypens
kolumnrepresentation senare härleds automatiskt.
