# Fortsatt utveckling av Dunder

Kompilatorns fortsatta optimeringsarbete omfattar SIMD, autovektorisering och
målspecifik schemaläggning för ARM64 och x86-64. Byggtid och körprestanda
följs med samma kompilatorrevision. [Mätmetod och mål](prestanda.md)

Kör först korskompilerade `.exe`-filer på Windows x86-64 och verifiera
Win64-FFI inklusive callbacks. Porta sedan OS-lagret för två byteidentiska
seriella självbyggen och fungerande `check`, `build`, `run` och `test` med
mellanslag och svenska tecken i sökvägarna. Därefter följer parallell
kompilering, språkserver, paket och distribution. [Målplattformar](targets.md)

För språkservern är nästa områden kodåtgärder och inkrementell analys.
[Språkservern](../selfhost/driver/lsp.ddr)

Kolumnlagring kan redan uttryckas med en array per fält och en gemensam
posttyp. En eventuell SoA-härledning behöver visa en vinst i verkliga
arbetslaster och bevara lånen när kolumnerna växer.

Fristående bibliotek kan utvecklas som paket när programmen i testbädden
visar ett konkret behov. [Befintliga paket](../packages/)
