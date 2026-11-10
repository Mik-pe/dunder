# macOS och Metal från Dunder

`import std.macos as macos` öppnar ett vanligt AppKit-fönster och ritar
en triangelista med Metal. Programmet kompileras direkt till en ARM64 Mach-O.
Fönsterloopen, tillståndet, inmatningslogiken, geometrin och animationsmatematiken
i exemplen är Dunder-kod.

Den lilla nativegränsen anropar AppKit, QuartzCore och Metal genom systemets
Objective-C-runtime. Ingen C-, Objective-C- eller Swift-källkod kompileras,
och ingen extern länkare eller signeringsprocess krävs. Den fasta MSL-shadern
kompileras av Apples Metal-runtime när fönstret öppnas. Det är en GPU-pipeline,
inte en alternativ kompilator för Dunders CPU-kod. API:t tar inte emot egen
shaderkod.

Kör från en inloggad grafisk macOS-session på Apple Silicon:

Fönsteranrop kräver målet `arm64-macos`. Geometrihjälparna kan användas även
vid kompilering för Linux; ett nåbart fönsteranrop ger då ett målrelaterat fel.

```sh
cargo build --release
./target/release/dunder run dogfood/metal/triangle.ddr
./target/release/dunder run dogfood/metal/main.ddr
```

Atlas-exemplet har följande kontroller:

| Inmatning | Beteende |
| --- | --- |
| Dra i skulpturen | Flyttar kompositionens mittpunkt. |
| Mellanslag | Pausar eller återupptar animationen. |
| C | Växlar accentpalett. |
| R | Återställer mittpunkten. |
| Escape eller fönstrets stängknapp | Stänger och avslutar loopen. |

Fönstret kan storleksändras. Layouten beräknas i logiska punkter och Metal-ytan
följer skärmens backing scale. Den ritade ordmärkesgrafiken är geometri i scenen;
det här API:t innehåller ännu ingen textlayout eller uppsättning UI-kontroller.

För ett begränsat röktest skickas programmets argument efter `--`:

```sh
./target/release/dunder run dogfood/metal/triangle.ddr -- --frames 3
./target/release/dunder run dogfood/metal/main.ddr -- --frames 120
./target/release/dunder test dogfood/metal/main.ddr
```

`--frames N` är Dunder-kod i `dogfood/metal/options.ddr`. Det avslutar efter N
lyckade `draw`-anrop och skriver räknaren till stdout. Fönstret öppnas även i
detta läge; det är inte ett headless-läge. Testkommandot kör bara taggade
geometri-, matematik- och tillståndstester och öppnar inget fönster.

Det manuella livscykeltestet öppnar och stänger sex riktiga fönster och provar
Retina-mått, gamla handtag, felaktig geometri och GPU-räknare. Låt fönstren
arbeta ostört; framgång skriver `lifecycle cycles=3`:

```sh
./target/release/dunder run dogfood/metal/lifecycle.ddr
```

En vanlig `.app` kan byggas utan projektgenerator:

```sh
./target/release/dunder build dogfood/metal/main.ddr --app -o build/DunderMetal.app
open build/DunderMetal.app
```

## Fönster och resultat

```dunder
pub struct Window { id: i64 }
pub struct OpenResult { ok: bool, window: Window, error: string }
pub struct DrawResult { ok: bool, error: string, frames: i64 }

pub fn open(title: string, width: i32, height: i32) -> OpenResult
pub fn poll(window: Window) -> Frame
pub fn draw(window: Window, vertices: &[]f32, red: f64, green: f64, blue: f64) -> DrawResult
pub fn close(window: Window)
```

`open` får innehållets önskade storlek i logiska punkter. Kontrollera `.ok`
innan handtaget används; `.error` innehåller en felbeskrivning vid misslyckande.
Första versionen stöder ett aktivt fönster per process. Ett nytt fönster kan
öppnas efter att det tidigare har stängts.

`Window.id` är ett validerat generationshandtag, inte en minnesadress.
En värdekopia av `Window` refererar till samma nativefönster. `close` stänger
resursen för alla kopior, och upprepade stängningar är tillåtna. Gamla eller
påhittade ID:n ger aldrig tillgång till nativepekare.

`draw` rensar bakgrunden med de tre färgkomponenterna och skickar geometri
till den fasta pipelinen. `.frames` räknar fönstrets lyckade bildinlämningar;
det är ingen mätning av skärmens faktiska bildfrekvens. Vertexarrayen lånas
under anropet och kan återanvändas när det återvänder. Ett misslyckat anrop
ger `.ok == false` och en felbeskrivning.

En lista får innehålla högst 100 000 vertices och måste bestå av hela
trianglar. En tom lista rensar bara bakgrunden. Första implementationen
väntar på GPU-kommandots slutförande innan `draw` återvänder och återanvänder
ingen GPU-buffer mellan uppladdningar. Dunders vertexarray kan återanvändas.
Om fönstret är dolt, minimerat eller tillfälligt saknar drawable lyckas
anropet utan att öka `.frames`.

## Händelser och Retina

```dunder
pub struct Frame {
    closed: bool,
    width: i32,
    height: i32,
    pixel_width: i32,
    pixel_height: i32,
    scale: f64,
    time: f64,
    pointer_x: f64,
    pointer_y: f64,
    pointer_down: bool,
    key: i32,
    resized: bool,
}
```

`poll` behandlar väntande AppKit-händelser och returnerar det aktuella
tillståndet. `closed` betyder att fönstret har stängts; minimering stänger
det inte. `width` och `height` är innehållets storlek i logiska punkter.
`pixel_width`, `pixel_height` och `scale` beskriver backing-ytan. Använd de
logiska måtten för layout och pekarinmatning.

Pekarens origo är innehållets övre vänstra hörn, med positiv Y nedåt.
`pointer_down` beskriver vänster musknapp. `key` är en fysisk macOS-keyCode,
eller `-1` om ingen tangent rapporteras. Exemplet använder Escape `53`,
mellanslag `49`, C `8` och R `15`. Detta är ingen textinmatnings- eller
tangentbordslayout-API. `time` är monotona sekunder sedan fönstret öppnades;
`resized` signalerar ändrad yta.

Varje tangenttryck och varje press-/drag-/släpphändelse avslutar den aktuella
pollningen. Därmed kan även en redan köad kort dragning observeras över flera
`Frame`-värden. Vanliga musrörelser kan slås ihop till senaste positionen.

AppKit kan ta över händelseslingan under en interaktiv storleksändring.
Native-callbacken anpassar Metal-ytan och ritar senast uppladdad geometri;
Dunders animation fortsätter när `poll` återvänder.

## Geometri i Dunder

Varje vertex består av åtta `f32`: `[x, y, 0, 1, red, green, blue, alpha]`.
Positionerna är clip coordinates: `(-1,-1)` nere till vänster och `(1,1)`
uppe till höger. Tre efterföljande vertices bildar en triangel. Färger
interpoleras över triangeln. Alpha är rak alpha och blandas över det som
redan har ritats. Ritordningen bestämmer överlappningen.

Biblioteket bygger denna representation med vanliga Dunder-funktioner:

| Funktion eller typ | Användning |
| --- | --- |
| `Point { x: f32, y: f32 }` | En position i clip coordinates. |
| `Color { red, green, blue, alpha }` | Fyra `f32`-komponenter. |
| `rgb(r,g,b)`, `rgba(r,g,b,a)` | Skapar en färg; `rgb` väljer alpha 1. |
| `vertex(&mut vertices, point, color)` | Lägger till en vertex. |
| `triangle(&mut vertices, a,b,c, color)` | Lägger till en enfärgad triangel. |
| `gradient_triangle(&mut vertices, a,b,c, ca,cb,cc)` | Lägger till en triangel med tre vertexfärger. |
| `quad(&mut vertices, x,y,width,height, color)` | Lägger till två trianglar; x/y är nedre vänstra hörnet. |

En triangel behöver bara detta:

```dunder
var vertices: []f32 = []
macos.gradient_triangle(&mut vertices,
    macos.Point { x: 0.0, y: 0.7 },
    macos.Point { x: -0.7, y: -0.6 },
    macos.Point { x: 0.7, y: -0.6 },
    macos.rgb(1.0, 0.7, 0.3),
    macos.rgb(0.2, 0.9, 0.7),
    macos.rgb(0.4, 0.5, 1.0))
```

Atlas visar moduler, structar, värde-enums, muterbara borrows, dynamiska
arrayer, flyttal, loopar och inmatningsstyrt tillstånd i ett verkligt program.
Dess ringar, cirklar, roterade kort, fontgrafik och sinusapproximationer är
implementerade under `dogfood/metal/`. Vertexbufferten reserveras en gång
och töms med `clear()` inför varje bild. Det återanvänder lagringen i Dunders
nuvarande processarena i stället för att skapa en ny array varje bild.
