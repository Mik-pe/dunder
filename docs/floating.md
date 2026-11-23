# Flyttal i Dunder

`f32` och `f64` är IEEE 754 binary32 respektive binary64. De fungerar i
lokala bindningar, parametrar, returvärden, callbacks, arrayer, iteratorer,
structfält och moduler. Källfiler har filändelsen `.ddr`.

```dunder
@test {
    (3.0, 4.0) -> 5.0
}
fn length(x: f64, y: f64) -> f64 {
    (x * x + y * y).sqrt()
}

fn main() {
    narrow: f32 = 0.1
    wide := 6.022_140_76e23
    print(length(3.0, 4.0))
    print(narrow)
    print(wide)
}
```

## Literaler och typer

Decimaler skrivs `1.25`, exponentform `1e3`, `1.25e-3` eller `1.25E+3`.
Understreck får separera siffror. Decimalpunkten behöver siffror på båda
sidor; använd `0.5`, inte `.5`. Ett intervall skrivs fortfarande `1..3`.
Ett okontextualiserat flyttalsuttryck får typen `f64`.

En förväntad typ från deklaration, parameter, returvärde eller en annan
operand kan ge literalen typen `f32`. Heltalsliteraler kan också få en
flyttalstyp i sådan kontext: `x: f32 = 1` och `x + 1` fungerar.
`1 + 0.5` ger `f64`. Ett befintligt heltals- eller flyttalsvärde konverteras
aldrig implicit till en annan numerisk typ.

```dunder
x: f32 = 1.25
y := x + 0.5
wide := f64(y)
count := 10
scaled := f64(count) * 0.25
```

En f32-literal avrundas direkt från decimalstavningen till binary32; den
passerar inte genom binary64 först. En literal som blir oändlig i sin typ
avvisas med `E3099`. Mycket små literaler kan avrundas till noll. Använd
exponentform för mycket stora/små tal; syntaxen utan decimalpunkt eller
exponent använder fortfarande lexerns heltalsgräns.

## Aritmetik och IEEE-värden

`+`, `-`, `*`, `/`, unärt minus och samtliga jämförelser fungerar för båda
typerna. `%` är än så länge bara en heltalsoperation. Operanderna måste ha
samma typ efter att literaler fått sin kontext. Varje operation avrundas i
sin egen typ med närmaste värde och jämna sista biten vid lika avstånd.
Ingen reassociering eller automatisk sammanslagning till FMA görs.

Runtimeöverspill ger oändlighet; `1.0 / 0.0` ger positiv oändlighet och
`0.0 / 0.0` ger NaN. Underflöde stöder subnormala tal och därefter signerad
noll. Flyttal använder inte heltalens spill- eller division-med-noll-fällor.
NaN är olikt allt, även sig självt; `<`, `<=`, `>` och `>=` är falska om
någon operand är NaN. `0.0 == -0.0` är sant, men teckenbiten finns kvar.

| Metod | Resultat |
| --- | --- |
| `x.abs()` | Absolutvärde med samma typ. |
| `x.sin()`, `x.cos()`, `x.tan()` | Vinkel i radianer. Importera `std.math` för metoderna på `f32` och `f64`. Resultatet har mottagarens typ. |
| `x.sqrt()` | Kvadratrot; negativt ändligt argument ger NaN. |
| `x.floor()`, `x.ceil()` | Avrundning nedåt respektive uppåt, med samma flyttalstyp. |
| `x.trunc()` | Avrundning mot noll, med samma flyttalstyp. |
| `x.round()` | Närmaste heltalsvärde; halva steg avrundas bort från noll. |
| `x.round_ties_even()` | Närmaste heltalsvärde; halva steg avrundas till jämnt. |
| `x.is_nan()`, `x.is_finite()`, `x.is_infinite()` | Klassificering som bool. |
| `x.is_sign_negative()` | Teckenbiten, även för negativ noll. |
| `x.to_bits()` | Råbitar som signerad i32 för f32 eller i64 för f64. |
| `f32.from_bits(bits)`, `f64.from_bits(bits)` | Tolkar heltalets råbitar, utan numerisk konvertering. |
| `x.str()`, `print(x)` | Decimalutskrift enligt avsnittet nedan. |

NaN:s exakta payload efter aritmetik är inte ett portabelt språkresultat.
`from_bits`/`to_bits` bevarar råbitar när inga beräkningar görs på värdet.
För ett little-endian-fält i en bytebuffert läser
`f32.from_bits(bytes.i32_le(&data, offset))` även negativa tal och negativ noll.
För ungefärliga jämförelser behöver programmet välja tolerans uttryckligt;
`==` använder vanlig IEEE-likhet.

## Explicita konverteringar

`f32(value)` och `f64(value)` tar heltal eller flyttal. Breddning från f32
till f64 är exakt. Övriga konverteringar kan förlora precision. Smalning
från f64 kan ge oändlighet eller signerad noll enligt IEEE-reglerna.

`i32(value)` och `i64(value)` trunkerar först mot noll och kräver sedan att
resultatet ryms. NaN, oändlighet och ett trunkerat värde utanför intervallet
avslutar processen med status **107**. Exempelvis ger `i32(3.9)` värdet 3,
`i32(-3.9)` värdet -3 och `i32(-2147483648.9)` värdet -2147483648.
Bool och rune konverteras inte direkt till flyttal.

## Decimalutskrift i Dunder

[runtime/float.ddr](../runtime/float.ddr) implementerar utskriften i Dunder.
Kompilatorn bäddar in och kompilerar funktionen när ett program använder
`print` eller `.str()` på flyttal. Färdiga program behöver inte källfilen,
extern C-runtime, libm eller något kompilatorverktyg vid körning.

Formatteraren bygger exakta decimalgränser för värdets avrundningsintervall
och söker den kortaste decimalrepresentationen som avrundas tillbaka till
samma flyttal. `0.1` skrivs som `0.1` för båda typerna. Fast notation får
`.0` för heltalsvärden, exempelvis `1.0`. Tiopotenser under -4 eller från
16 använder exponentform, exempelvis `1e-5` och `1e+20`. Specialvärden
skrivs `nan`, `inf`, `-inf`, `0.0` och `-0.0`.

Kontraktsprovet läser tillbaka decimaltexten med `strtod` och jämför
bitmönstret vid den ursprungliga typens precision. [Formateringsprovet](../tools/test_runtime_contracts.ddr)

Utskriftssträngar använder den delade heapen.
Direkt utskrift frigör sin formatteringssträng efter skrivningen;
`.str()` returnerar ett referensräknat värde. Se [minnesmodellen](memory.md).

## Decimalparsning vid körning

`std.parse.Float32::parse(text)` och `std.parse.Float64::parse(text)`
läser decimaler med valfritt tecken, decimaldel och exponent. Understreck
får stå mellan siffror i samma taldel. `range(text, start, end)` läser ett
byteintervall och kräver siffror utan understreck. Resultatet har `ok`,
`value` och `error` (1 vid ogiltig stavning, 2 vid överspill).
Båda använder samma direktavrundning som källkodsliteralerna och bevarar
tecknet vid underflöde till noll.

## Körda program och gränser

```sh
# I Dunders testbädd dunder-hundmat:
dunder run dogfood/numeric/main.ddr
# 5.0
# 2.0
# true
dunder test dogfood/numeric/main.ddr
# 7 passed, 0 failed
```

Programmen provar typade vektorer, omskalad hypotenusa, toleransjämförelser,
kompenserad summering, bisektion med callback och Runge–Kutta-integration.
Hjälparna ger exempel på hur flyttalsstödet fungerar i Dunder-program.
Metoderna i `std.math.Trigonometry` använder samma beräkning som de fria
funktionerna `math.sin`, `math.cos` och `math.tan`. Med `f32` breddas
vinkeln före beräkningen och resultatet avrundas till `f32`. En modul som använder
metoderna kan skriva `import std.math`. Skriv `import std.math as _` för att
ladda metoderna utan ett lokalt modulnamn.

I Dunders interna ABI passerar skalära flyttalsargument som råbitar i
x0–x7, precis som övriga skalära argument; från det nionde argumentet
används stacken. Flyttalsresultat returneras som råbitar i x0.
Registeruppdelningen v0–v7 för flyttalsargument och s0/d0 för
flyttalsreturer enligt
[AArch64:s anropsstandard](https://github.com/ARM-software/abi-aa/blob/main/aapcs64/aapcs64.rst)
gäller C-anrop, som har ett separat [FFI-kontrakt](ffi.md).
Vanliga struct- och arrayparametrar använder Dunders interna lånekonvention;
`move`-parametrar och resursägande returer överför ägarskapet.
