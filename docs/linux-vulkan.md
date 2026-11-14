# Vulkan på Linux

Programmen i `dunder-hundmat` (`dogfood/vulkan/`) importerar grafikbiblioteket med
`import ..graphics.vulkan as vulkan`. Ingången är
`dogfood/graphics/vulkan.ddr` i syskonrepot `dunder-hundmat`. Ett eget
projekt kan kopiera `vulkan.ddr` och dess syskonkatalog `vulkan/` till
`graphics/` bredvid sin `main.ddr` och använda `import graphics.vulkan as vulkan`.

Biblioteket laddar systemets Vulkan-bibliotek genom `std.dl`.
Implementationen i `dogfood/graphics/vulkan/` beskriver
driverstrukturerna med `@c_layout` och anropar
Vulkan och SDL genom typade `extern fn`-fält. Modulen äger allokering,
livscykel, rasteriseringens konfiguration och händelsehantering.
SPIR-V-orden finns i `shader.ddr` tillsammans med GLSL-källorna i samma
katalog. En shaderkompilator behövs endast när shaderkoden ändras.

Vulkan renderar till en bild som kan läsas tillbaka som RGBA. Fönsterläget
visar samma bild via SDL2:s mjukvaruyta. SDL hanterar fönster och händelser;
det rasteriserar inte geometrin. Den första versionen använder alltså
readback och en yt-blit, inte en Vulkan-swapchain. Headless-läget behöver
varken SDL, en skärm eller en window surface.

## Kör med lavapipe

Installera Vulkan-loader, Mesa-drivrutiner och SDL2 på Linux. På Debian/Ubuntu:

```sh
sudo apt install libvulkan1 mesa-vulkan-drivers libsdl2-2.0-0 vulkan-tools
dunder="$(tools/bootstrap.sh)"
```

Välj lavapipe uttryckligen för reproducerbar mjukvarurendering. ICD-filens
namn varierar mellan distributioner; välj den installerade `lvp`-filen:

```sh
ls /usr/share/vulkan/icd.d/*lvp*.json
export VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json
vulkaninfo --summary

"$dunder" run dogfood/vulkan/triangle.ddr -- --headless --frames 3 --capture build/triangle.ppm
"$dunder" run dogfood/vulkan/main.ddr -- --headless --frames 3 --capture build/atlas.ppm
"$dunder" run dogfood/vulkan/lifecycle.ddr
```

Programmen skriver namnet på den faktiskt valda enheten. Mesa brukar
rapportera lavapipe-enheten som `llvmpipe`. Utan ICD-variabeln väljer
Vulkan-loadern bland installerade drivrutiner, inklusive fysisk GPU.
Kompilatorn väljer inte lavapipe åt slutanvändaren.

För interaktiv körning i en grafisk Linux-session:

```sh
"$dunder" run dogfood/vulkan/main.ddr
"$dunder" run dogfood/vulkan/triangle.ddr -- --frames 120
```

Dra skulpturen med vänster musknapp. Mellanslag pausar, C byter palett,
R återställer och Escape stänger. Fönstret kan storleksändras.
Renderingsupplösningen är tills vidare den storlek som angavs vid `open`;
SDL skalar bilden till fönstrets aktuella storlek. `Frame.width/height`
följer fönstret och `pixel_width/pixel_height` anger Vulkan-bildens storlek.
`--headless` och `--capture` väljer tre bildrutor om `--frames` saknas.
Capture använder en bestämd animationstid och skriver en binär PPM-bild
(P6); den fungerar utan externa bildverktyg.

## Exempelmodulens API

`Window`, `OpenResult`, `Frame` och `DrawResult` följer Metal-API:ts form.
Vulkan-modulens typer är egna nominella typer.

```dunder
pub fn open(title: string, width: i32, height: i32) -> OpenResult
pub fn open_headless(width: i32, height: i32) -> OpenResult
pub fn poll(window: Window) -> Frame
pub fn draw(window: Window, vertices: &[]f32, red: f64, green: f64, blue: f64) -> DrawResult
pub fn device_name(window: Window) -> string
pub fn pixels(window: Window) -> []i32
pub fn close(window: Window)
```

Kontrollera `OpenResult.ok` och `DrawResult.ok`; `.error` beskriver fel.
Handtaget är ett generations-ID. Ett gammalt eller påhittat handtag ger
inte tillgång till nativepekare. `close` får upprepas; ett nytt fönster
kan öppnas efter stängning. Ett aktivt fönster eller headless-mål stöds
per process.

Vertexformatet är åtta `f32`: `[x, y, 0, 1, red, green, blue, alpha]`.
`Point`, `Color`, `rgb`, `rgba`, `vertex`, `triangle`, `gradient_triangle`
och `quad` bygger detta format med vanliga Dunder-funktioner.
Clip-koordinater har positiv Y uppåt, samma som Metal-exemplen.
Pekarkoordinater har origo uppe till vänster och positiv Y nedåt.
Färger använder rak alpha. En tom triangelista rensar bakgrunden.

`pixels` ger en oberoende kopia av senast renderade RGBA-byte, översta
raden först, fyra byte per pixel. Ogiltiga handtag ger en tom array;
`device_name` ger då en tom sträng. `DrawResult.frames` räknar lyckade
Vulkan-inlämningar. Rendering väntar på GPU-kön innan anropet återvänder,
så vertexarrayen kan återanvändas direkt.

`Frame.key` använder ASCII för kontrollerna: Escape 27, mellanslag 32,
C 99 och R 114; -1 betyder ingen tangent. Det är ett kontroll-API,
inte ett API för Unicode-textinmatning.

## Verifiering och gränser

Från repositoryts rot bygger följande kontroll Metal-exemplet som en
macOS-app och Vulkan-exemplet för båda Linux-målen. Den verifierar ELF-filernas
arkitektur, dynamiska laddare, segmenträttigheter och reproducerbarhet.

```sh
python3 tools/test-native.py "$dunder"
```

För körprov med den valda Vulkan-drivrutinen och Khronos valideringslager:

```sh
sudo apt install vulkan-validationlayers
VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation python3 tools/test-native.py "$dunder" --graphics
```

Med `--graphics` renderar både triangeln och Atlas tre bildrutor. Separata
pixelprov kontrollerar bakgrund, Y-riktning, färginterpolering, alpha och
oberoende RGBA-kopior. Livscykelprogrammet provar gamla handtag och felaktig
geometri. SDL-provet läser en pixel från fönstrets egen yta och skickar
tangent- och stängningshändelser genom SDL-kön med dummy-videodrivrutinen.
En saknad loader eller grafikdrivrutin gör körprovet rött.

Detta är en första synkron renderingsväg. Vulkan-swapchain, flera samtidiga
fönster, texturer, egen shaderkod och textlayout återstår. Readback och
mjukvarupresentation gör den olämplig som prestandareferens för en framtida
GPU-driven swapchain.
