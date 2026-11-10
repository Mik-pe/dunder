# Dunder-status

Status: **RUNNING**
Uppdrag: docs/uppdrag.md (2026-11-10). Gren: `dogfood/arm64`.
Referenser: README.md, docs/arkitektur.md, docs/prestanda.md.

## Kontrollpunkt 1 — SLUTFÖRD 2026-11-10 (commit kommer)

Fyra filer från förra sessionen granskade och fixade; alla fel verifierade mot
oberoende källor (SDK `mach/machine.h`, /bin/ls arm64-slice, codesign-signerad
referens) med ett reproducerbart prov per fel. Byggvägen använder endast
python3 (ursprungsfrö, dev); verifiering använde clang/codesign/otool (ej i
byggvägen).

### Verifierade fel och fixar i tools/dev/seedasm.py

1. **cputype** `0x1000000C` → `0x0100000C`. Bevis: SDK `CPU_TYPE_ARM64 =
   (CPU_TYPE_ARM|CPU_ARCH_ABI64)` = 16777228 = /bin/ls arm64-slice. Ofixad
   bild: körning → "bad CPU type" (127). Fixad: felet borta (processen kom
   vidare till signaturkontroll).
2. **sizeofcmds** `32 + cmds_size` → `cmds_size`. Bevis: /bin/ls
   sizeofcmds(1712) == summa(cmdsize), fältet exkluderar mach_header_64.
   Vårt fält blev 288 == summa efter fix.
3. **CodeDirectory-magi** `0xFade0C17` → `0xFADE0C02`. Bevis: signaturblob
   typ 0 i /bin/ls och codesign-referens har 0xFADE0C02 (vår = 0xFADE0C17
   i baseline-inspektion).
4. **CD-sidstorlek** 12 → 14 (16 KiB). Bevis: lokal codesign-referens
   (clang-byggd `tiny.ref`, ad hoc) använder pageSizeLog2=14 på denna
   arm64-maskin.
5. **execSegLimit** len(text) → __TEXT-vmsize. Bevis: samma referens har
   execSegLimit = 0x4000 = __TEXT vmsize.
6. **dyld-koppling saknades** (nytt krav på macOS 27 arm64): lade till
   LC_LOAD_DYLINKER(/usr/lib/dyld) + MH_DYLDLINK (flags 0x00200005).
   Bevis: clang-länkad STATISK binär utan dyld (giltig ad hoc-signatur,
   codesign verify OK) dödas ändå med SIGKILL av kärnan; med dylinker kör
   vår bild. => kärnan kräver dyld-kopplade executables på denna plattform.
7. **calltest-prologen**: stp/ldp med writeback (pre/post-index) saknades i
   kodaren och etiketten '!' slogs bort av tokenizern. La till former +
   10 nya differentiellfall. Under arbetet fångade diffenc två egna fel
   (inverterad 32/64-bit-tabell; fel LDP pre-opcode 0xA840→0xA9C0) —
   differentiell verifiering bevisat värdefull.

### KP1-bevis (kommandon i TMPDIR/kp1, python3 tools/dev/seedasm.py)

- `seedasm.py origin/etapp0.s` → körbar; `./etapp0.bin; echo $?` → **42**.
- Samma källa med `mov x0, 7` → **7**.
- `seedasm.py origin/calltest.s` → `./calltest.bin` → **42** (riktigt
  funktionsanrop: main → bl add_tva → add x0,x0,2).
- Determinism: två byggen `cmp` identiska;
  sha256(etapp0) = dde39802635e8eb969e12adb1cb7d3fff9e92704f5f0e99c154c4bc9fbdd367a.
- Oberoende inspektion: `file` = Mach-O 64-bit executable arm64;
  `otool -h` cputype 16777228, ncmds 7, sizeofcmds 392;
  `otool -l` PAGEZERO/TEXT/LINKEDIT/BUILD_VERSION/LOAD_DYLINKER/MAIN
  (entryoff 432 = textstart)/CODE_SIGNATURE; `codesign -dv` adhoc v20400,
  `codesign -v` OK.
- `diffenc.py`: **100 instruktionsord jämförda, 0 skillnader** (var 95).

## Aktuell kontrollpunkt: 2 — Fröet bygger sig självt

Nästa verifierbara handling: skriv fröet (assembler + Mach-O-skrivare +
SHA-256-signerare + fil-I/O via svc 0x80-syscalls) i det egna fröspråket,
så att `./seed origin/seed.s ut.bin` ger byteidentisk bild med
ursprungsfröets bild av samma källa, och seed bygger calltest som kör (42).

### Checklista KP2

- [ ] Utöka fröspråket där så behövs (t.ex. adc/sbc/eor-rotation för
      SHA-256; b.ne etc. finns) — varje ny kodning differentiellt verifierad.
- [ ] Syscallanrop: open/read/write/close/exit via svc #0x80, x16=0x2000000|nr.
- [ ] seed.s: läs källfil (argv), tvåpass-assemblera, skriv signerad Mach-O.
- [ ] Bevis 1: seed bygger sin egen källa → byteidentisk med sin egen binär
      (självbygge D1 == origin).
- [ ] Bevis 2: seed bygger calltest.s → kör → 42.
- [ ] Checka in: ursprung (seedasm.py-vägen), seed.s, binär, kontrollsummor,
      exakta kommandon i origin/README.md.
- [ ] Commit.

## Verifierat / misslyckat / oprövat

- Verifierat: allt under KP1 ovan (maskin: Apple Silicon, macOS 27.2,
  26B123; verifieringsverktyg: clang/codesign/otool ur Xcode — ej byggväg).
- Misslyckat: static.exec utan dyld dödas (ovan, bevarat som kunskap).
- Oprövat: KP2–KP6.

## Beslut

- macOS arm64 (27.x) kräver dyld-kopplade executables: alla Dunder-bilder
  får LC_LOAD_DYLINKER + MH_DYLDLINK. dyld ingår i plattformen enligt
  arkitekturdokumentet ("operativsystemets laddare").
- Temporära filer i TMPDIR.
