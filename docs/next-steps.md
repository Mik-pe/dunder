# Fortsatt utveckling av Dunder

Kompilatorns fortsatta optimeringsarbete omfattar SIMD, autovektorisering och
målspecifik schemaläggning för ARM64 och x86-64. Byggtid och körprestanda
följs med samma kompilatorrevision. [Mätmetod och mål](prestanda.md)

För Windows återstår bredare stöd i standardbiblioteket och paketen. För
språkservern är nästa områden kodåtgärder och inkrementell analys.
[Målplattformar](targets.md) · [Språkservern](../selfhost/driver/lsp.ddr)

Kolumnlagring kan redan uttryckas med en array per fält och en gemensam
posttyp. En eventuell SoA-härledning behöver visa en vinst i verkliga
arbetslaster och bevara lånen när kolumnerna växer.

Fristående bibliotek kan utvecklas som paket när programmen i testbädden
visar ett konkret behov. [Befintliga paket](../packages/)
