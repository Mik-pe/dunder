# ============================================================================
# Dunder-frö 1: självassemblerande fröverktyg i eget fröspråk.
#
# Bygger en källfil (.s) till en signerad körbar ARM64/Mach-O-bild, byte för
# byte identiskt med ursprungsfröet tools/dev/seedasm.py (som enbart får
# materialisera det dokumenterade ursprunget; se origin/README.md).
#
# Användning: ./seed in.s ut.bin
# All I/O via macOS-syscall: x16 = 0x2000000+nr, svc 0x80.
# ============================================================================

.ident "dunder-seed"
.entry _start

.text

# ----------------------------------------------------------------------------
# Systemanrop
# ----------------------------------------------------------------------------

io_exit:
    mov x16, 0x2000001
    svc 0x80

# io_read1(fd, buf, len) -> n
io_read1:
    mov x16, 0x2000003
    svc 0x80
    ret

# io_write1(fd, buf, len) -> n
io_write1:
    mov x16, 0x2000004
    svc 0x80
    ret

# io_open_ro(path) -> fd
io_open_ro:
    mov x1, 0
    mov x2, 0
    mov x16, 0x2000005
    svc 0x80
    ret

# io_open_out(path) -> fd   (O_WRONLY|O_CREAT|O_TRUNC = 0x601, mode 0755)
io_open_out:
    mov x1, 0x601
    mov x2, 493
    mov x16, 0x2000005
    svc 0x80
    ret

# io_close(fd)
io_close:
    mov x16, 0x2000006
    svc 0x80
    ret

# io_strlen(ptr) -> len
io_strlen:
    mov x1, 0
io_strlen_l:
    ldrb w2, [x0, x1]
    cbz w2, io_strlen_e
    add x1, x1, 1
    b io_strlen_l
io_strlen_e:
    mov x0, x1
    ret

# io_writen(fd, buf, len): skriver allt (loop över partial writes)
io_writen:
    stp x29, x30, [sp, #-48]!
    str x0, [sp, #16]
    str x1, [sp, #24]
    str x2, [sp, #32]
io_writen_l:
    ldr x0, [sp, #16]
    ldr x1, [sp, #24]
    ldr x2, [sp, #32]
    cmp x2, 0
    b.le io_writen_e
    bl io_write1
    cmp x0, 1
    b.lt io_writen_e
    ldr x1, [sp, #24]
    add x1, x1, x0
    str x1, [sp, #24]
    ldr x1, [sp, #32]
    sub x1, x1, x0
    str x1, [sp, #32]
    b io_writen_l
io_writen_e:
    ldp x29, x30, [sp], #48
    ret

# io_wdec(fd, val): skriver val decimalt
io_wdec:
    stp x29, x30, [sp, #-16]!
    mov x8, x0
    mov x3, 0
    lea x4, g_scratch
    add x4, x4, 24
    mov x5, 10
io_wdec_l:
    udiv x6, x1, x5
    mul x7, x6, x5
    sub x7, x1, x7
    add x7, x7, 48
    sub x4, x4, 1
    strb w7, [x4, #0]
    add x3, x3, 1
    mov x1, x6
    cmp x1, 0
    b.ne io_wdec_l
    mov x0, x8
    mov x1, x4
    mov x2, x3
    bl io_writen
    ldp x29, x30, [sp], #16
    ret

# ----------------------------------------------------------------------------
# Felrapportering: avslutar med status 1 efter meddelande på fd 2
# ----------------------------------------------------------------------------

# fl_fail(cstr): "seedasm: " + cstr + "\n"
fl_fail:
    stp x29, x30, [sp, #-16]!
    mov x19, x0
    mov x0, 2
    lea x1, m_pre
    mov x2, 9
    bl io_writen
    mov x0, x19
    bl io_strlen
    mov x2, x0
    mov x0, 2
    mov x1, x19
    bl io_writen
    mov x0, 2
    lea x1, m_nl
    mov x2, 1
    bl io_writen
    mov x0, 1
    bl io_exit

# fl_fail_ln(ln, cstr): "seedasm: rad N: cstr\n"
fl_fail_ln:
    stp x29, x30, [sp, #-32]!
    str x0, [sp, #16]
    str x1, [sp, #24]
    mov x0, 2
    lea x1, m_pre
    mov x2, 9
    bl io_writen
    mov x0, 2
    lea x1, m_rad
    mov x2, 4
    bl io_writen
    ldr x1, [sp, #16]
    mov x0, 2
    bl io_wdec
    mov x0, 2
    lea x1, m_kol
    mov x2, 2
    bl io_writen
    ldr x0, [sp, #24]
    bl io_strlen
    mov x2, x0
    mov x0, 2
    ldr x1, [sp, #24]
    bl io_writen
    mov x0, 2
    lea x1, m_nl
    mov x2, 1
    bl io_writen
    mov x0, 1
    bl io_exit

# fl_fail_ln_ln(ln, cstr, ptr, len): "seedasm: rad N: cstr + 'tok'\n"
fl_fail_ln_tok:
    stp x29, x30, [sp, #-32]!
    str x0, [sp, #16]
    str x1, [sp, #24]
    str x2, [sp, #0]
    str x3, [sp, #8]
    mov x0, 2
    lea x1, m_pre
    mov x2, 9
    bl io_writen
    mov x0, 2
    lea x1, m_rad
    mov x2, 4
    bl io_writen
    ldr x1, [sp, #16]
    mov x0, 2
    bl io_wdec
    mov x0, 2
    lea x1, m_kol
    mov x2, 2
    bl io_writen
    ldr x0, [sp, #24]
    bl io_strlen
    mov x2, x0
    mov x0, 2
    ldr x1, [sp, #24]
    bl io_writen
    mov x0, 2
    lea x1, m_cit
    mov x2, 1
    bl io_writen
    ldr x0, [sp, #0]
    ldr x2, [sp, #8]
    bl io_writen
    mov x0, 2
    lea x1, m_cit
    mov x2, 1
    bl io_writen
    mov x0, 2
    lea x1, m_nl
    mov x2, 1
    bl io_writen
    mov x0, 1
    bl io_exit

# ----------------------------------------------------------------------------
# Tokenbuffer: g_toks, 64 slotar a 16 byte (ptr, len)
# len-varden: 0xFFFF... (dvs -1) = '[', -2 = ']', -3 = '!'
# ----------------------------------------------------------------------------

# ps_tok_at(i) -> x0=ptr x1=len
ps_tok_at:
    lea x2, g_toks
    lsl x3, x0, 4
    add x2, x2, x3
    ldr x0, [x2, #0]
    ldr x1, [x2, #8]
    ret

# ps_push_tok(ptr, len)
ps_push_tok:
    lea x2, g_ntok
    ldr x3, [x2, #0]
    cmp x3, 64
    b.ge ps_push_err
    lsl x4, x3, 4
    lea x5, g_toks
    add x5, x5, x4
    str x0, [x5, #0]
    str x1, [x5, #8]
    add x3, x3, 1
    str x3, [x2, #0]
    ret
ps_push_err:
    mov x0, 1
    lea x1, m_ntok
    b fl_fail_ln

# ps_push_mark(kod)  (kod i x0: -1,-2,-3)
ps_push_mark:
    lea x2, g_ntok
    ldr x3, [x2, #0]
    cmp x3, 64
    b.ge ps_push_err
    lsl x4, x3, 4
    lea x5, g_toks
    add x5, x5, x4
    mov x6, 0
    str x6, [x5, #0]
    str x0, [x5, #8]
    add x3, x3, 1
    str x3, [x2, #0]
    ret

# ----------------------------------------------------------------------------
# Symboltabell (linjar sokning), etiketter
# ----------------------------------------------------------------------------

# ps_lab_find(ptr, len) -> x0=idx eller -1
ps_lab_find:
    stp x29, x30, [sp, #-16]!
    lea x4, g_srcbuf
    lea x5, g_nlab
    ldr x6, [x5, #0]
    mov x7, 0
ps_lab_find_l:
    cmp x7, x6
    b.ge ps_lab_find_n
    lsl x8, x7, 4
    lsl x9, x7, 3
    add x8, x8, x9
    lea x9, g_labs
    add x9, x9, x8
    ldr x10, [x9, #0]
    add x10, x4, x10
    ldr x11, [x9, #8]
    cmp x11, x1
    b.ne ps_lab_find_c
    mov x12, 0
ps_lab_find_b:
    cmp x12, x1
    b.ge ps_lab_find_h
    add x13, x0, x12
    add x14, x10, x12
    ldrb w15, [x13, #0]
    ldrb w16, [x14, #0]
    cmp w15, w16
    b.ne ps_lab_find_c
    add x12, x12, 1
    b ps_lab_find_b
ps_lab_find_h:
    mov x0, x7
    b ps_lab_find_e
ps_lab_find_c:
    add x7, x7, 1
    b ps_lab_find_l
ps_lab_find_n:
    mov x0, -1
ps_lab_find_e:
    ldp x29, x30, [sp], #16
    ret

# ps_lab_def(ptr, len, ln): definierar etikett pa aktuell sektionsoffset
ps_lab_def:
    stp x29, x30, [sp, #-32]!
    str x0, [sp, #16]
    str x1, [sp, #24]
    str x2, [sp, #0]
    bl ps_lab_find
    cmp x0, 0
    b.lt ps_lab_def_ok
    ldr x0, [sp, #0]
    lea x1, m_duplab
    b fl_fail_ln
ps_lab_def_ok:
    lea x4, g_nlab
    ldr x5, [x4, #0]
    lsl x6, x5, 4
    lsl x9, x5, 3
    add x6, x6, x9
    lea x7, g_labs
    add x7, x7, x6
    lea x8, g_srcbuf
    ldr x9, [sp, #16]
    sub x9, x9, x8
    str x9, [x7, #0]
    ldr x9, [sp, #24]
    str x9, [x7, #8]
    lea x9, g_cursec
    ldr x10, [x9, #0]
    str x10, [x7, #16]
    cmp x10, 0
    b.ne ps_lab_def_c
    lea x9, g_tbufp
    ldr x11, [x9, #0]
    lea x12, g_tbuf
    sub x11, x11, x12
    b ps_lab_def_s
ps_lab_def_c:
    cmp x10, 1
    b.ne ps_lab_def_d
    lea x9, g_cbufp
    ldr x11, [x9, #0]
    lea x12, g_cbuf
    sub x11, x11, x12
    b ps_lab_def_s
ps_lab_def_d:
    cmp x10, 2
    b.ne ps_lab_def_b
    lea x9, g_dbufp
    ldr x11, [x9, #0]
    lea x12, g_dbuf
    sub x11, x11, x12
    b ps_lab_def_s
ps_lab_def_b:
    lea x9, g_bsssz
    ldr x11, [x9, #0]
ps_lab_def_s:
    str x11, [x7, #24]
    add x5, x5, 1
    str x5, [x4, #0]
    ldp x29, x30, [sp], #32
    ret

# ----------------------------------------------------------------------------
# Register- och immediate-parsning (port av seedasm.parse_reg/parse_imm/is_reg)
# ----------------------------------------------------------------------------

# ps_streq4(ptr, len, c1, c2, c3): jamfor 3 tecken + langd 3
# (anvands internt av ps_reg)
#
# ps_reg(ptr, len, ln) -> x0=sf(0/1), x1=num. Fel -> exit.
ps_reg:
    stp x29, x30, [sp, #-16]!
    str x2, [sp, #0]
    # xzr
    lea x3, s_xzr
    cmp x1, 3
    b.ne ps_reg_wzr
    ldrb w4, [x0, #0]
    ldrb w5, [x3, #0]
    cmp w4, w5
    b.ne ps_reg_wzr
    ldrb w4, [x0, #1]
    ldrb w5, [x3, #1]
    cmp w4, w5
    b.ne ps_reg_wzr
    ldrb w4, [x0, #2]
    ldrb w5, [x3, #2]
    cmp w4, w5
    b.ne ps_reg_wzr
    mov x0, 1
    mov x1, 31
    b ps_reg_e
ps_reg_wzr:
    lea x3, s_wzr
    cmp x1, 3
    b.ne ps_reg_sp
    ldrb w4, [x0, #0]
    ldrb w5, [x3, #0]
    cmp w4, w5
    b.ne ps_reg_sp
    ldrb w4, [x0, #1]
    ldrb w5, [x3, #1]
    cmp w4, w5
    b.ne ps_reg_sp
    ldrb w4, [x0, #2]
    ldrb w5, [x3, #2]
    cmp w4, w5
    b.ne ps_reg_sp
    mov x0, 0
    mov x1, 31
    b ps_reg_e
ps_reg_sp:
    lea x3, s_sp
    cmp x1, 2
    b.ne ps_reg_num
    ldrb w4, [x0, #0]
    ldrb w5, [x3, #0]
    cmp w4, w5
    b.ne ps_reg_num
    ldrb w4, [x0, #1]
    ldrb w5, [x3, #1]
    cmp w4, w5
    b.ne ps_reg_num
    mov x0, 1
    mov x1, 31
    b ps_reg_e
ps_reg_num:
    cmp x1, 2
    b.lt ps_reg_err
    ldrb w4, [x0, #0]
    cmp w4, 120
    b.eq ps_reg_isx
    cmp w4, 119
    b.ne ps_reg_err
    mov x5, 0
    b ps_reg_d
ps_reg_isx:
    mov x5, 1
ps_reg_d:
    mov x6, 1
    mov x7, 0
ps_reg_dl:
    cmp x6, x1
    b.ge ps_reg_de
    add x8, x0, x6
    ldrb w9, [x8, #0]
    sub w9, w9, 48
    cmp w9, 9
    b.gt ps_reg_err
    mov w10, 10
    mul w7, w7, w10
    add w7, w7, w9
    add x6, x6, 1
    b ps_reg_dl
ps_reg_de:
    cmp x7, 30
    b.gt ps_reg_err
    mov x0, x5
    mov x1, x7
    b ps_reg_e
ps_reg_err:
    ldr x0, [sp, #0]
    lea x1, m_badreg
    b ps_reg_flt
ps_reg_flt:
    b fl_fail_ln
ps_reg_e:
    ldp x29, x30, [sp], #16
    ret

# ps_isreg(ptr, len) -> x0=1 om registerlik (xN/wN/xzr/wzr, N<=31)
ps_isreg:
    stp x29, x30, [sp, #-16]!
    cmp x1, 2
    b.lt ps_isreg_n
    ldrb w3, [x0, #0]
    cmp w3, 120
    b.eq ps_isreg_x
    cmp w3, 119
    b.eq ps_isreg_x
    b ps_isreg_n
ps_isreg_x:
    # xzr/wzr?
    cmp x1, 3
    b.ne ps_isreg_d
    lea x4, s_xzr
    ldrb w5, [x0, #1]
    ldrb w6, [x4, #1]
    cmp w5, w6
    b.ne ps_isreg_zw
    ldrb w5, [x0, #2]
    ldrb w6, [x4, #2]
    cmp w5, w6
    b.eq ps_isreg_y
ps_isreg_zw:
    lea x4, s_wzr
    ldrb w5, [x0, #1]
    ldrb w6, [x4, #1]
    cmp w5, w6
    b.ne ps_isreg_d
    ldrb w5, [x0, #2]
    ldrb w6, [x4, #2]
    cmp w5, w6
    b.eq ps_isreg_y
ps_isreg_d:
    mov x4, 1
ps_isreg_dl:
    cmp x4, x1
    b.ge ps_isreg_v
    add x5, x0, x4
    ldrb w6, [x5, #0]
    sub w6, w6, 48
    cmp w6, 9
    b.gt ps_isreg_n
    add x4, x4, 1
    b ps_isreg_dl
ps_isreg_v:
    mov x5, 0
    mov x6, 1
ps_isreg_vl:
    cmp x6, x1
    b.ge ps_isreg_vd
    add x7, x0, x6
    ldrb w8, [x7, #0]
    sub w8, w8, 48
    mov w9, 10
    mul w5, w5, w9
    add w5, w5, w8
    add x6, x6, 1
    b ps_isreg_vl
ps_isreg_vd:
    cmp x5, 31
    b.gt ps_isreg_n
ps_isreg_y:
    mov x0, 1
    b ps_isreg_e
ps_isreg_n:
    mov x0, 0
ps_isreg_e:
    ldp x29, x30, [sp], #16
    ret

# ps_imm(ptr, len, ln) -> x0=varde (signed 64). Fel -> exit.
# Stoder: # valfritt, - valfritt, 0x hex, decimal utan ledande nollor.
ps_imm:
    stp x29, x30, [sp, #-16]!
    str x2, [sp, #0]
    mov x3, x0
    mov x4, x1
    cmp x4, 1
    b.lt ps_imm_err
    ldrb w5, [x3, #0]
    cmp w5, 35
    b.ne ps_imm_nh
    add x3, x3, 1
    sub x4, x4, 1
    cmp x4, 1
    b.lt ps_imm_err
    ldrb w5, [x3, #0]
ps_imm_nh:
    mov x6, 0
    cmp w5, 45
    b.ne ps_imm_go
    mov x6, 1
    add x3, x3, 1
    sub x4, x4, 1
    cmp x4, 1
    b.lt ps_imm_err
ps_imm_go:
    cmp x4, 2
    b.lt ps_imm_dec
    ldrb w5, [x3, #0]
    cmp w5, 48
    b.ne ps_imm_dec
    add x7, x3, 1
    ldrb w5, [x7, #0]
    cmp w5, 120
    b.eq ps_imm_hex
    cmp w5, 88
    b.eq ps_imm_hex
    cmp x4, 1
    b.ne ps_imm_dec
    mov x0, 0
    b ps_imm_e
ps_imm_hex:
    add x3, x3, 2
    sub x4, x4, 2
    mov x7, 0
ps_imm_hl:
    cmp x4, 0
    b.le ps_imm_hv
    ldrb w5, [x3, #0]
    sub w8, w5, 48
    cmp w8, 9
    b.le ps_imm_ha
    sub w8, w5, 65
    cmp w8, 5
    b.le ps_imm_hb
    sub w8, w5, 97
    cmp w8, 5
    b.gt ps_imm_err
    add w8, w8, 10
    b ps_imm_ha
ps_imm_hb:
    add w8, w8, 10
ps_imm_ha:
    lsl x7, x7, 4
    add x7, x7, x8
    add x3, x3, 1
    sub x4, x4, 1
    b ps_imm_hl
ps_imm_hv:
    mov x0, x7
    b ps_imm_fin
ps_imm_dec:
    mov x7, 0
ps_imm_dl:
    cmp x4, 0
    b.le ps_imm_dv
    ldrb w5, [x3, #0]
    sub w8, w5, 48
    cmp w8, 9
    b.gt ps_imm_err
    mov w9, 10
    mul x7, x7, x9
    add x7, x7, x8
    add x3, x3, 1
    sub x4, x4, 1
    b ps_imm_dl
ps_imm_dv:
    mov x0, x7
ps_imm_fin:
    cmp x6, 1
    b.ne ps_imm_e
    sub x0, xzr, x0
ps_imm_e:
    ldp x29, x30, [sp], #16
    ret
ps_imm_err:
    ldr x0, [sp, #0]
    lea x1, m_badimm
    b fl_fail_ln

# ps_emitw(word): appendera 4 byte LE till textbufferten
ps_emitw:
    lea x1, g_tbufp
    ldr x2, [x1, #0]
    str w0, [x2, #0]
    add x2, x2, 4
    str x2, [x1, #0]
    ret

# ps_emitb(ptr, len): appendera bytes till aktuell sektion (bss: enbart storlek)
ps_emitb:
    stp x29, x30, [sp, #-16]!
    lea x4, g_cursec
    ldr x5, [x4, #0]
    cmp x5, 3
    b.ne ps_emitb_c
    lea x4, g_bsssz
    ldr x6, [x4, #0]
    add x6, x6, x1
    str x6, [x4, #0]
    b ps_emitb_e
ps_emitb_c:
    cmp x5, 1
    b.ne ps_emitb_t
    lea x4, g_cbufp
    b ps_emitb_go
ps_emitb_t:
    cmp x5, 2
    b.ne ps_emitb_d
    lea x4, g_dbufp
    b ps_emitb_go
ps_emitb_d:
    lea x4, g_tbufp
ps_emitb_go:
    ldr x6, [x4, #0]
    mov x7, 0
ps_emitb_l:
    cmp x7, x1
    b.ge ps_emitb_e
    add x8, x0, x7
    ldrb w9, [x8, #0]
    strb w9, [x6, #0]
    add x6, x6, 1
    add x7, x7, 1
    b ps_emitb_l
ps_emitb_e:
    str x6, [x4, #0]
    ldp x29, x30, [sp], #16
    ret

# ps_emit0(n): appendera n nollbyte (bss: enbart storlek)
ps_emit0:
    stp x29, x30, [sp, #-16]!
    lea x1, g_zbuf
    mov x2, x0
ps_emit0_l:
    cmp x2, 0
    b.le ps_emit0_e
    mov x0, x1
    mov x3, 1
    cmp x2, 64
    b.le ps_emit0_s
    mov x3, 64
ps_emit0_s:
    bl ps_emitb
    sub x2, x2, x3
    b ps_emit0_l
ps_emit0_e:
    ldp x29, x30, [sp], #16
    ret

# ----------------------------------------------------------------------------
# h_tokeq(tokindex, cptr, clen) -> x0=1/0
# ----------------------------------------------------------------------------

h_tokeq:
    stp x29, x30, [sp, #-32]!
    str x1, [sp, #16]
    str x2, [sp, #24]
    bl ps_tok_at
    ldr x3, [sp, #16]
    ldr x4, [sp, #24]
    cmp x1, x4
    b.ne h_tokeq_n
    mov x5, 0
h_tokeq_l:
    cmp x5, x4
    b.ge h_tokeq_y
    add x6, x0, x5
    ldrb w7, [x6, #0]
    add x6, x3, x5
    ldrb w8, [x6, #0]
    cmp w7, w8
    b.ne h_tokeq_n
    add x5, x5, 1
    b h_tokeq_l
h_tokeq_y:
    mov x0, 1
    b h_tokeq_e
h_tokeq_n:
    mov x0, 0
h_tokeq_e:
    ldp x29, x30, [sp], #32
    ret

# ----------------------------------------------------------------------------
# ps_tokenize(start, end): fyller g_toks
# ----------------------------------------------------------------------------

ps_tokenize:
    stp x29, x30, [sp, #-64]!
    stp x19, x20, [sp, #16]
    stp x21, x22, [sp, #32]
    stp x23, x24, [sp, #48]
    mov x19, x0
    mov x20, x1
    mov x23, x19
    lea x21, g_ntok
    str xzr, [x21, #0]
    b ps_tz_ws
ps_tz_ws:
    cmp x23, x20
    b.ge ps_tz_done
    ldrb w9, [x23, #0]
    cmp w9, 32
    b.eq ps_tz_ws1
    cmp w9, 9
    b.eq ps_tz_ws1
    cmp w9, 13
    b.eq ps_tz_ws1
    b ps_tz_c
ps_tz_ws1:
    add x23, x23, 1
    b ps_tz_ws
ps_tz_c:
    cmp w9, 44
    b.eq ps_tz_comma
    cmp w9, 91
    b.eq ps_tz_ob
    cmp w9, 93
    b.eq ps_tz_cb
    mov x24, x23
ps_tz_scan:
    cmp x23, x20
    b.ge ps_tz_push
    ldrb w9, [x23, #0]
    cmp w9, 32
    b.eq ps_tz_push
    cmp w9, 9
    b.eq ps_tz_push
    cmp w9, 13
    b.eq ps_tz_push
    cmp w9, 44
    b.eq ps_tz_push
    cmp w9, 91
    b.eq ps_tz_push
    cmp w9, 93
    b.eq ps_tz_push
    add x23, x23, 1
    b ps_tz_scan
ps_tz_push:
    mov x0, x24
    sub x1, x23, x24
    bl ps_push_tok
    b ps_tz_ws
ps_tz_comma:
    add x23, x23, 1
    b ps_tz_ws
ps_tz_ob:
    mov x0, -1
    bl ps_push_mark
    add x23, x23, 1
    b ps_tz_ws
ps_tz_cb:
    mov x0, -2
    bl ps_push_mark
    add x23, x23, 1
    cmp x23, x20
    b.ge ps_tz_ws
    ldrb w9, [x23, #0]
    cmp w9, 33
    b.ne ps_tz_ws
    mov x0, -3
    bl ps_push_mark
    add x23, x23, 1
    b ps_tz_ws
ps_tz_done:
    ldp x19, x20, [sp, #16]
    ldp x21, x22, [sp, #32]
    ldp x23, x24, [sp, #48]
    ldp x29, x30, [sp], #64
    ret

# ----------------------------------------------------------------------------
# ps_parse_line(start, end, ln)
# ----------------------------------------------------------------------------

ps_parse_line:
    stp x29, x30, [sp, #-80]!
    stp x19, x20, [sp, #16]
    stp x21, x22, [sp, #32]
    stp x23, x24, [sp, #48]
    stp x25, x26, [sp, #64]
    mov x19, x0
    mov x20, x1
    mov x21, x2
    mov x22, x19
ps_pl_ws:
    cmp x22, x20
    b.ge ps_pl_ret
    ldrb w9, [x22, #0]
    cmp w9, 32
    b.eq ps_pl_ws1
    cmp w9, 9
    b.eq ps_pl_ws1
    cmp w9, 13
    b.eq ps_pl_ws1
    b ps_pl_go
ps_pl_ws1:
    add x22, x22, 1
    b ps_pl_ws
ps_pl_go:
    ldrb w9, [x22, #0]
    cmp w9, 35
    b.eq ps_pl_ret
    # kommentar: '#' med ws fore och (slut eller ws efter)
    mov x23, x22
ps_pl_cs:
    cmp x23, x20
    b.ge ps_pl_rst
    ldrb w9, [x23, #0]
    cmp w9, 35
    b.ne ps_pl_cs1
    cmp x23, x22
    b.eq ps_pl_cs1
    sub x24, x23, 1
    ldrb w9, [x24, #0]
    cmp w9, 32
    b.eq ps_pl_csok
    cmp w9, 9
    b.eq ps_pl_csok
    cmp w9, 13
    b.eq ps_pl_cs1
    b ps_pl_cs1
ps_pl_csok:
    add x24, x23, 1
    cmp x24, x20
    b.ge ps_pl_csc
    ldrb w9, [x24, #0]
    cmp w9, 32
    b.eq ps_pl_csc
    cmp w9, 9
    b.eq ps_pl_csc
    cmp w9, 13
    b.eq ps_pl_csc
    b ps_pl_cs1
ps_pl_csc:
    mov x20, x23
    b ps_pl_rst
ps_pl_cs1:
    add x23, x23, 1
    b ps_pl_cs
ps_pl_rst:
    # radera slutf Spaces
    cmp x20, x22
    b.le ps_pl_ret
    sub x24, x20, 1
    ldrb w9, [x24, #0]
    cmp w9, 32
    b.eq ps_pl_rst1
    cmp w9, 9
    b.eq ps_pl_rst1
    cmp w9, 13
    b.eq ps_pl_rst1
    b ps_pl_l1
ps_pl_rst1:
    mov x20, x24
    b ps_pl_rst
ps_pl_l1:
    # etikettform 1: inga ws/komma och slutar pa ':'
    mov x23, x22
    mov x25, 1
ps_pl_l1s:
    cmp x23, x20
    b.ge ps_pl_l1d
    ldrb w9, [x23, #0]
    cmp w9, 32
    b.eq ps_pl_l1n
    cmp w9, 9
    b.eq ps_pl_l1n
    cmp w9, 13
    b.eq ps_pl_l1n
    cmp w9, 44
    b.eq ps_pl_l1n
    add x23, x23, 1
    b ps_pl_l1s
ps_pl_l1n:
    mov x25, 0
ps_pl_l1d:
    cmp x25, 0
    b.eq ps_pl_l2
    sub x23, x20, 1
    ldrb w9, [x23, #0]
    cmp w9, 58
    b.ne ps_pl_l2
    mov x0, x22
    sub x1, x23, x22
    mov x2, x21
    bl ps_lab_def
    b ps_pl_ret
ps_pl_l2:
    # etikettform 2: namn, ws*, ':', ws+, rest
    ldrb w9, [x22, #0]
    # namnstart: A-Z a-z _ . $
    bl ps_nstart
    cbz x0, ps_pl_ins
    mov x23, x22
ps_pl_l2n:
    cmp x23, x20
    b.ge ps_pl_ins
    ldrb w0, [x23, #0]
    bl ps_nchar
    cbz x0, ps_pl_l2e
    add x23, x23, 1
    b ps_pl_l2n
ps_pl_l2e:
    mov x24, x23
ps_pl_l2w:
    cmp x24, x20
    b.ge ps_pl_ins
    ldrb w9, [x24, #0]
    cmp w9, 32
    b.eq ps_pl_l2w1
    cmp w9, 9
    b.eq ps_pl_l2w1
    cmp w9, 13
    b.eq ps_pl_l2w1
    b ps_pl_l2c
ps_pl_l2w1:
    add x24, x24, 1
    b ps_pl_l2w
ps_pl_l2c:
    ldrb w9, [x24, #0]
    cmp w9, 58
    b.ne ps_pl_ins
    add x25, x24, 1
    cmp x25, x20
    b.ge ps_pl_ins
    ldrb w9, [x25, #0]
    cmp w9, 32
    b.eq ps_pl_l2ok
    cmp w9, 9
    b.eq ps_pl_l2ok
    cmp w9, 13
    b.eq ps_pl_l2ok
    b ps_pl_ins
ps_pl_l2ok:
    mov x0, x22
    sub x1, x23, x22
    mov x2, x21
    bl ps_lab_def
    add x22, x25, 0
ps_pl_l2r:
    cmp x22, x20
    b.ge ps_pl_ret
    ldrb w9, [x22, #0]
    cmp w9, 32
    b.eq ps_pl_l2r1
    cmp w9, 9
    b.eq ps_pl_l2r1
    cmp w9, 13
    b.eq ps_pl_l2r1
    b ps_pl_ins
ps_pl_l2r1:
    add x22, x22, 1
    b ps_pl_l2r
ps_pl_ins:
    lea x9, g_ln
    str x21, [x9, #0]
    mov x0, x22
    mov x1, x20
    bl ps_tokenize
    lea x9, g_ntok
    ldr x9, [x9, #0]
    cmp x9, 0
    b.eq ps_pl_ret
    lea x9, g_toks
    ldr x10, [x9, #0]
    ldrb w10, [x10, #0]
    cmp w10, 46
    b.eq ps_pl_dir
    mov x0, x21
    bl ps_instr
    b ps_pl_ret
ps_pl_dir:
    mov x0, x21
    bl ps_directive
ps_pl_ret:
    ldp x19, x20, [sp, #16]
    ldp x21, x22, [sp, #32]
    ldp x23, x24, [sp, #48]
    ldp x25, x26, [sp, #64]
    ldp x29, x30, [sp], #80
    ret

# ps_nstart(c i x0) -> x0=1 om A-Z a-z _ . $
ps_nstart:
    cmp x0, 65
    b.lt ps_nstart_1
    cmp x0, 90
    b.le ps_nstart_y
ps_nstart_1:
    cmp x0, 97
    b.lt ps_nstart_2
    cmp x0, 122
    b.le ps_nstart_y
ps_nstart_2:
    cmp x0, 95
    b.eq ps_nstart_y
    cmp x0, 46
    b.eq ps_nstart_y
    cmp x0, 36
    b.eq ps_nstart_y
    mov x0, 0
    ret
ps_nstart_y:
    mov x0, 1
    ret

# ps_nchar(c i x0) -> x0=1 om namntecken (nstart + 0-9)
ps_nchar:
    cmp x0, 48
    b.lt ps_nstart
    cmp x0, 57
    b.gt ps_nstart
    mov x0, 1
    ret

# ----------------------------------------------------------------------------
# Instruktion: gren/lea-detektion + fixup, annars en_encode
# ----------------------------------------------------------------------------

# en_reg(tokidx) -> x0=sf x1=num
en_reg:
    stp x29, x30, [sp, #-16]!
    str x0, [sp, #0]
    bl ps_tok_at
    ldr x2, [sp, #0]
    lea x3, g_ln
    ldr x2, [x3, #0]
    bl ps_reg
    ldp x29, x30, [sp], #16
    ret

# en_imm(tokidx) -> x0=varde
en_imm:
    stp x29, x30, [sp, #-16]!
    str x0, [sp, #0]
    bl ps_tok_at
    ldr x2, [sp, #0]
    lea x3, g_ln
    ldr x2, [x3, #0]
    bl ps_imm
    ldp x29, x30, [sp], #16
    ret

# en_isreg(tokidx) -> x0=0/1
en_isreg:
    stp x29, x30, [sp, #-16]!
    str x0, [sp, #0]
    bl ps_tok_at
    bl ps_isreg
    ldp x29, x30, [sp], #16
    ret

# en_condid(ptr) -> x0=cond-id 0..15
en_condid:
    stp x29, x30, [sp, #-16]!
    str x0, [sp, #0]
    ldrb w8, [x0, #0]
    ldrb w9, [x0, #1]
    lsl w8, w8, 24
    lsl w9, w9, 16
    orr w8, w8, w9
    lea x7, c_conds
    mov x10, 0
en_condid_l:
    cmp x10, 18
    b.ge en_condid_e
    ldr w9, [x7, x10]
    cmp w9, w8
    b.eq en_condid_f
    add x10, x10, 1
    b en_condid_l
en_condid_f:
    cmp x10, 16
    b.lt en_condid_f2
    sub x10, x10, 16
en_condid_f2:
    mov x0, x10
    b en_condid_r
en_condid_e:
    lea x3, g_ln
    ldr x0, [x3, #0]
    lea x1, m_badcond
    b fl_fail_ln
en_condid_r:
    ldp x29, x30, [sp], #16
    ret

# fx_add(kind, oparg, labptr, lablen): sparar fixup vid aktuell textoffset
fx_add:
    stp x29, x30, [sp, #-16]!
    lea x4, g_nfix
    ldr x5, [x4, #0]
    lsl x6, x5, 6
    lsl x9, x5, 3
    sub x6, x6, x9
    lea x7, g_fixs
    add x7, x7, x6
    str x0, [x7, #0]
    lea x8, g_ln
    ldr x8, [x8, #0]
    str x8, [x7, #8]
    str x1, [x7, #16]
    str x2, [x7, #24]
    str x3, [x7, #32]
    lea x8, g_tbufp
    ldr x8, [x8, #0]
    lea x9, g_tbuf
    sub x8, x8, x9
    str x8, [x7, #40]
    add x5, x5, 1
    str x5, [x4, #0]
    ldp x29, x30, [sp], #16
    ret

# ps_instr(ln): x20=kind, x21=oparg, x22=labidx, x23=size, gemensam svans
ps_instr:
    stp x29, x30, [sp, #-48]!
    str x19, [sp, #16]
    mov x19, x0
    mov x0, 0
    lea x1, s_b
    mov x2, 1
    bl h_tokeq
    cbnz x0, ps_i_b
    mov x0, 0
    lea x1, s_bl
    mov x2, 2
    bl h_tokeq
    cbnz x0, ps_i_bl
    # b.cc: langd 4 och andra tecknet '.'
    lea x3, g_toks
    ldr x4, [x3, #0]
    ldr x5, [x3, #8]
    cmp x5, 4
    b.ne ps_i_notbcc
    ldrb w6, [x4, #1]
    cmp w6, 46
    b.eq ps_i_bcc
ps_i_notbcc:
    mov x0, 0
    lea x1, s_cbz
    mov x2, 3
    bl h_tokeq
    cbnz x0, ps_i_cbz
    mov x0, 0
    lea x1, s_cbnz
    mov x2, 4
    bl h_tokeq
    cbnz x0, ps_i_cbnz
    mov x0, 0
    lea x1, s_tbz
    mov x2, 3
    bl h_tokeq
    cbnz x0, ps_i_tbz
    mov x0, 0
    lea x1, s_tbnz
    mov x2, 4
    bl h_tokeq
    cbnz x0, ps_i_tbnz
    mov x0, 0
    lea x1, s_lea
    mov x2, 3
    bl h_tokeq
    cbnz x0, ps_i_lea
    mov x0, x19
    bl en_encode
    b ps_i_e
ps_i_b:
    mov x20, 0
    mov x21, 0
    mov x22, 1
    mov x23, 4
    b ps_i_tail
ps_i_bl:
    mov x20, 1
    mov x21, 0
    mov x22, 1
    mov x23, 4
    b ps_i_tail
ps_i_bcc:
    add x0, x4, 2
    bl en_condid
    mov x20, 2
    mov x21, x0
    mov x22, 1
    mov x23, 4
    b ps_i_tail
ps_i_cbz:
    mov x20, 3
    b ps_i_cbnz_common
ps_i_cbnz:
    mov x20, 4
ps_i_cbnz_common:
    mov x0, 1
    bl en_reg
    lsl x0, x0, 8
    orr x21, x0, x1
    mov x22, 2
    mov x23, 4
    b ps_i_tail
ps_i_tbz:
    mov x20, 5
    b ps_i_tbz_common
ps_i_tbnz:
    mov x20, 6
ps_i_tbz_common:
    mov x0, 1
    bl en_reg
    lsl x4, x1, 8
    mov x0, 2
    bl en_imm
    orr x21, x4, x0
    mov x22, 3
    mov x23, 4
    b ps_i_tail
ps_i_lea:
    mov x0, 1
    bl en_reg
    cmp x0, 1
    b.eq ps_i_lea_x
    lea x3, g_ln
    ldr x0, [x3, #0]
    lea x1, m_leax
    b fl_fail_ln
ps_i_lea_x:
    mov x20, 7
    mov x21, x1
    mov x22, 2
    mov x23, 8
ps_i_tail:
    mov x0, x22
    bl ps_tok_at
    mov x2, x0
    mov x3, x1
    mov x0, x20
    mov x1, x21
    bl fx_add
    mov x0, x23
    bl ps_emit0
ps_i_e:
    ldr x19, [sp, #16]
    ldp x29, x30, [sp], #48
    ret

# ----------------------------------------------------------------------------
# Direktiv
# ----------------------------------------------------------------------------

# ps_secsz() -> x0 = aktuell sektionstorlek
ps_secsz:
    lea x4, g_cursec
    ldr x5, [x4, #0]
    cmp x5, 0
    b.ne ps_ss_c
    lea x4, g_tbufp
    ldr x6, [x4, #0]
    lea x7, g_tbuf
    sub x0, x6, x7
    ret
ps_ss_c:
    cmp x5, 1
    b.ne ps_ss_d
    lea x4, g_cbufp
    ldr x6, [x4, #0]
    lea x7, g_cbuf
    sub x0, x6, x7
    ret
ps_ss_d:
    cmp x5, 2
    b.ne ps_ss_b
    lea x4, g_dbufp
    ldr x6, [x4, #0]
    lea x7, g_dbuf
    sub x0, x6, x7
    ret
ps_ss_b:
    lea x4, g_bsssz
    ldr x0, [x4, #0]
    ret

# ps_emit_val(v, szb): skriver low szb byte LE i g_scratch och emitterar
ps_emit_val:
    stp x29, x30, [sp, #-16]!
    lea x2, g_scratch
    mov x3, 0
ps_ev_l:
    cmp x3, x1
    b.ge ps_ev_d
    strb w0, [x2, #0]
    add x2, x2, 1
    lsr x0, x0, 8
    add x3, x3, 1
    b ps_ev_l
ps_ev_d:
    lea x0, g_scratch
    mov x1, x3
    bl ps_emitb
    ldp x29, x30, [sp], #16
    ret

# ps_unesc(src, len) -> emitterar unescape:ade byte i aktuell sektion
ps_unesc:
    stp x29, x30, [sp, #-64]!
    stp x19, x20, [sp, #16]
    stp x21, x22, [sp, #32]
    stp x23, x24, [sp, #48]
    mov x19, x0
    mov x20, x1
    lea x21, g_scratch2
    mov x22, 0
    mov x23, 0
ps_un_l:
    cmp x22, x20
    b.ge ps_un_d
    add x4, x19, x22
    ldrb w5, [x4, #0]
    cmp w5, 92
    b.ne ps_un_pl
    add x22, x22, 1
    cmp x22, x20
    b.ge ps_un_err_end
    add x4, x19, x22
    ldrb w5, [x4, #0]
    cmp w5, 110
    b.eq ps_un_n
    cmp w5, 116
    b.eq ps_un_t
    cmp w5, 114
    b.eq ps_un_r
    cmp w5, 48
    b.eq ps_un_z
    cmp w5, 92
    b.eq ps_un_bs
    cmp w5, 34
    b.eq ps_un_q
    cmp w5, 120
    b.eq ps_un_hex
    b ps_un_err_esc
ps_un_n:
    mov w5, 10
    b ps_un_st
ps_un_t:
    mov w5, 9
    b ps_un_st
ps_un_r:
    mov w5, 13
    b ps_un_st
ps_un_z:
    mov w5, 0
    b ps_un_st
ps_un_bs:
    mov w5, 92
    b ps_un_st
ps_un_q:
    mov w5, 34
    b ps_un_st
ps_un_hex:
    add x22, x22, 1
    add x4, x19, x22
    ldrb w6, [x4, #0]
    sub w7, w6, 48
    cmp w7, 9
    b.le ps_un_h1
    sub w7, w6, 65
    cmp w7, 5
    b.le ps_un_ha
    sub w7, w6, 97
    cmp w7, 5
    b.gt ps_un_err_esc
    add w7, w7, 10
    b ps_un_h1
ps_un_ha:
    add w7, w7, 10
ps_un_h1:
    lsl w5, w7, 4
    add x22, x22, 1
    add x4, x19, x22
    ldrb w6, [x4, #0]
    sub w7, w6, 48
    cmp w7, 9
    b.le ps_un_h2
    sub w7, w6, 65
    cmp w7, 5
    b.le ps_un_hb
    sub w7, w6, 97
    cmp w7, 5
    b.gt ps_un_err_esc
    add w7, w7, 10
    b ps_un_h2
ps_un_hb:
    add w7, w7, 10
ps_un_h2:
    orr w5, w5, w7
    b ps_un_st2
ps_un_st:
    add x22, x22, 1
ps_un_st2:
    add x6, x21, x23
    strb w5, [x6, #0]
    add x23, x23, 1
    b ps_un_l
ps_un_pl:
    add x6, x21, x23
    strb w5, [x6, #0]
    add x23, x23, 1
    add x22, x22, 1
    b ps_un_l
ps_un_d:
    mov x0, x21
    mov x1, x23
    bl ps_emitb
    ldp x19, x20, [sp, #16]
    ldp x21, x22, [sp, #24]
    ldp x23, x24, [sp, #32]
    ldp x29, x30, [sp], #48
    ret
ps_un_err_end:
    lea x3, g_ln
    ldr x0, [x3, #0]
    lea x1, m_escend
    b fl_fail_ln
ps_un_err_esc:
    lea x3, g_ln
    ldr x0, [x3, #0]
    lea x1, m_escbad
    b fl_fail_ln

# ps_directive(ln)
ps_directive:
    stp x29, x30, [sp, #-48]!
    str x19, [sp, #16]
    str x20, [sp, #24]
    str x21, [sp, #32]
    mov x19, x0
    lea x3, g_ln
    str x19, [x3, #0]
    mov x0, 0
    lea x1, c_dtext
    mov x2, 5
    bl h_tokeq
    cbz x0, ps_d_1
    lea x3, g_cursec
    str xzr, [x3, #0]
    b ps_d_e
ps_d_1:
    mov x0, 0
    lea x1, c_dconst
    mov x2, 6
    bl h_tokeq
    cbz x0, ps_d_2
    lea x3, g_cursec
    mov x4, 1
    str x4, [x3, #0]
    b ps_d_e
ps_d_2:
    mov x0, 0
    lea x1, c_ddata
    mov x2, 5
    bl h_tokeq
    cbz x0, ps_d_3
    lea x3, g_cursec
    mov x4, 2
    str x4, [x3, #0]
    b ps_d_e
ps_d_3:
    mov x0, 0
    lea x1, c_dbss
    mov x2, 4
    bl h_tokeq
    cbz x0, ps_d_4
    lea x3, g_cursec
    mov x4, 3
    str x4, [x3, #0]
    b ps_d_e
ps_d_4:
    mov x0, 0
    lea x1, c_dentry
    mov x2, 6
    bl h_tokeq
    cbz x0, ps_d_5
    mov x0, 1
    bl ps_tok_at
    lea x3, g_entry_p
    str x0, [x3, #0]
    str x1, [x3, #8]
    b ps_d_e
ps_d_5:
    mov x0, 0
    lea x1, c_dident
    mov x2, 6
    bl h_tokeq
    cbz x0, ps_d_6
    mov x0, 1
    bl ps_tok_at
    # strippa " i borjan och slut (alla)
ps_d_id_l:
    cmp x1, 0
    b.le ps_d_id_t
    cmp x0, 0
    sub x4, x1, 1
    add x5, x0, x4
    ldrb w6, [x5, #0]
    cmp w6, 34
    b.ne ps_d_id_t
    sub x1, x1, 1
    b ps_d_id_l
ps_d_id_t:
    ldrb w6, [x0, #0]
    cmp w6, 34
    b.ne ps_d_id_w
    add x0, x0, 1
    sub x1, x1, 1
    b ps_d_id_t
ps_d_id_w:
    lea x3, g_ident
    mov x4, 0
ps_d_id_c:
    cmp x4, x1
    b.ge ps_d_id_d
    cmp x4, 255
    b.ge ps_d_id_d
    add x5, x0, x4
    ldrb w6, [x5, #0]
    add x7, x3, x4
    strb w6, [x7, #0]
    add x4, x4, 1
    b ps_d_id_c
ps_d_id_d:
    lea x3, g_ident_n
    str x4, [x3, #0]
    b ps_d_e
ps_d_6:
    mov x0, 0
    lea x1, c_dspace
    mov x2, 6
    bl h_tokeq
    cbz x0, ps_d_7
    mov x0, 1
    bl en_imm
    bl ps_emit0
    b ps_d_e
ps_d_7:
    mov x0, 0
    lea x1, c_dalign
    mov x2, 6
    bl h_tokeq
    cbz x0, ps_d_8
    mov x0, 1
    bl en_imm
    mov x20, x0
    bl ps_secsz
    # pad = a - cur % a  (a>0)
    udiv x4, x0, x20
    mul x5, x4, x20
    sub x5, x0, x5
    cmp x5, 0
    b.eq ps_d_7z
    sub x0, x20, x5
    b ps_d_7p
ps_d_7z:
    mov x0, 0
ps_d_7p:
    bl ps_emit0
    b ps_d_e
ps_d_8:
    mov x0, 0
    lea x1, c_du8
    mov x2, 3
    bl h_tokeq
    cbz x0, ps_d_9
    mov x20, 1
    b ps_d_u
ps_d_9:
    mov x0, 0
    lea x1, c_du16
    mov x2, 4
    bl h_tokeq
    cbz x0, ps_d_10
    mov x20, 2
    b ps_d_u
ps_d_10:
    mov x0, 0
    lea x1, c_du32
    mov x2, 4
    bl h_tokeq
    cbz x0, ps_d_11
    mov x20, 4
    b ps_d_u
ps_d_11:
    mov x0, 0
    lea x1, c_du64
    mov x2, 4
    bl h_tokeq
    cbz x0, ps_d_12
    mov x20, 8
    b ps_d_u
ps_d_12:
    mov x0, 0
    lea x1, c_dstr
    mov x2, 4
    bl h_tokeq
    cbz x0, ps_d_13
    mov x20, 0
    b ps_d_str
ps_d_13:
    mov x0, 0
    lea x1, c_dstrz
    mov x2, 5
    bl h_tokeq
    cbz x0, ps_d_bad
    mov x20, 1
ps_d_str:
    # sammanfoga token 1.. med enkel space i g_scratch
    lea x21, g_scratch
    mov x22, 0
    mov x23, 1
ps_d_str_j:
    lea x3, g_ntok
    ldr x3, [x3, #0]
    cmp x23, x3
    b.ge ps_d_str_q
    mov x0, x23
    bl ps_tok_at
    mov x24, 0
ps_d_str_c:
    cmp x24, x1
    b.ge ps_d_str_s
    add x5, x0, x24
    ldrb w6, [x5, #0]
    add x5, x21, x22
    strb w6, [x5, #0]
    add x22, x22, 1
    add x24, x24, 1
    b ps_d_str_c
ps_d_str_s:
    add x23, x23, 1
    lea x3, g_ntok
    ldr x3, [x3, #0]
    cmp x23, x3
    b.ge ps_d_str_q
    mov w6, 32
    add x5, x21, x22
    strb w6, [x5, #0]
    add x22, x22, 1
    b ps_d_str_j
ps_d_str_q:
    cmp x22, 0
    b.eq ps_d_str_bad
    ldrb w6, [x21, #0]
    cmp w6, 34
    b.ne ps_d_str_bad
    sub x5, x22, 1
    add x7, x21, x5
    ldrb w6, [x7, #0]
    cmp w6, 34
    b.ne ps_d_str_bad
    add x0, x21, 1
    sub x1, x22, 2
    bl ps_unesc
    cmp x20, 1
    b.ne ps_d_e
    lea x0, g_nul1
    mov x1, 1
    bl ps_emitb
    b ps_d_e
ps_d_str_bad:
    lea x3, g_ln
    ldr x0, [x3, #0]
    lea x1, m_strq
    b fl_fail_ln
ps_d_u:
    # .uN: en vardeserie fran token 1..
    mov x23, 1
ps_d_u_l:
    lea x3, g_ntok
    ldr x3, [x3, #0]
    cmp x23, x3
    b.ge ps_d_e
    mov x0, x23
    bl en_imm
    mov x1, x20
    bl ps_emit_val
    add x23, x23, 1
    b ps_d_u_l
ps_d_bad:
    lea x3, g_ln
    ldr x0, [x3, #0]
    lea x1, m_baddir
    b fl_fail_ln
ps_d_e:
    ldr x19, [sp, #16]
    ldr x20, [sp, #24]
    ldr x21, [sp, #32]
    ldp x29, x30, [sp], #48
    ret

# ----------------------------------------------------------------------------
# en_mem(bracket_idx) -> x0=base x1=kind(0/1/2) x2=imm x3=idx_sf x4=idx_num
#                        x5=shift x6=close_idx
# ----------------------------------------------------------------------------

en_mem:
    stp x29, x30, [sp, #-112]!
    stp x19, x20, [sp, #16]
    stp x21, x22, [sp, #32]
    stp x23, x24, [sp, #48]
    stp x25, x26, [sp, #64]
    stp x27, x28, [sp, #80]
    mov x19, x0
    mov x27, x19
    add x27, x27, 1
en_mem_cl:
    mov x0, x27
    bl ps_tok_at
    cmp x1, -2
    b.eq en_mem_cok
    add x27, x27, 1
    b en_mem_cl
en_mem_cok:
    mov x26, x27
    mov x0, x19
    add x0, x0, 1
    bl en_reg
    mov x20, x1
    mov x21, 0
    mov x22, 0
    mov x23, 0
    mov x24, 0
    mov x25, 0
    add x27, x19, 2
    cmp x27, x26
    b.ge en_mem_out
    mov x0, x27
    bl ps_tok_at
    cmp x1, 0
    b.lt en_mem_reg
    ldrb w8, [x0, #0]
    cmp w8, 35
    b.eq en_mem_imm
    b en_mem_reg
en_mem_imm:
    mov x0, x27
    bl en_imm
    mov x22, x0
    mov x21, 1
    b en_mem_out
en_mem_reg:
    add x28, x27, 1
    cmp x28, x26
    b.ne en_mem_reg3
    mov x0, x27
    bl en_reg
    mov x23, x0
    mov x24, x1
    mov x21, 2
    b en_mem_out
en_mem_reg3:
    add x28, x27, 3
    cmp x28, x26
    b.ne en_mem_bad
    mov x0, x27
    add x0, x0, 1
    bl h_tokeq3
    cbz x0, en_mem_bad
    mov x0, x27
    bl en_reg
    mov x23, x0
    mov x24, x1
    mov x0, x27
    add x0, x0, 2
    bl en_imm
    mov x25, x0
    mov x21, 2
    b en_mem_out
en_mem_bad:
    lea x3, g_ln
    ldr x0, [x3, #0]
    lea x1, m_badmem
    b fl_fail_ln
en_mem_out:
    mov x0, x20
    mov x1, x21
    mov x2, x22
    mov x3, x23
    mov x4, x24
    mov x5, x25
    mov x6, x26
    ldp x19, x20, [sp, #16]
    ldp x21, x22, [sp, #32]
    ldp x23, x24, [sp, #48]
    ldp x25, x26, [sp, #64]
    ldp x27, x28, [sp, #80]
    ldp x29, x30, [sp], #112
    ret

# h_tokeq3(tokidx): ar token 'lsl'? -> x0=1/0
h_tokeq3:
    stp x29, x30, [sp, #-16]!
    lea x1, s_lsl
    mov x2, 3
    bl h_tokeq
    ldp x29, x30, [sp], #16
    ret

# ----------------------------------------------------------------------------
# en_movimm(sf, v, rd): movz/movn/movk-sekvens
# ----------------------------------------------------------------------------

en_movimm:
    stp x29, x30, [sp, #-48]!
    stp x20, x21, [sp, #16]
    stp x22, x23, [sp, #32]
    mov x20, x0
    mov x21, x1
    mov x22, x2
    # 32-bitars intervallkontroll: -2^31 <= v < 2^32
    cmp x20, 0
    b.ne en_mi_nchk
    mov x9, 0x80000000
    sub x9, xzr, x9
    cmp x21, x9
    b.lt en_mi_err
    mov x9, 0xFFFFFFFF
    cmp x21, x9
    b.hi en_mi_err
en_mi_nchk:
    mov x9, -1
    cmp x21, x9
    b.gt en_mi_pos
    mov x9, -65536
    cmp x21, x9
    b.lt en_mi_gen
    # movn med inverterad low16
    mov x10, -1
    eor x9, x21, x10
    mov x10, 0xFFFF
    and x9, x9, x10
    lsl x9, x9, 5
    mov x10, 0x12800000
    orr x9, x10, x9
    b en_mi_mkn
en_mi_pos:
    mov x9, 0xFFFF
    cmp x21, x9
    b.hi en_mi_gen
    lsl x9, x21, 5
    mov x10, 0x52800000
    orr x9, x10, x9
    b en_mi_mkn
en_mi_gen:
    # u = v & mask (32-bit: trunkera)
    cmp x20, 0
    b.ne en_mi_g64
    lsr x21, x21, 0
    mov x10, 0xFFFFFFFF
    and x21, x21, x10
en_mi_g64:
    mov x9, 0xFFFF
    and x9, x21, x9
    lsl x9, x9, 5
    mov x10, 0x52800000
    orr x9, x10, x9
    orr x9, x9, x22
    mov x0, x9
    bl ps_emitw
    lsr x21, x21, 16
    mov x23, 1
en_mi_gl:
    cmp x21, 0
    b.eq en_mi_e
    mov x9, 0xFFFF
    and x9, x21, x9
    lsl x9, x9, 5
    mov x10, 0x72800000
    orr x9, x10, x9
    lsl x10, x23, 21
    orr x9, x9, x10
    orr x9, x9, x22
    mov x0, x9
    bl ps_emitw
    lsr x21, x21, 16
    add x23, x23, 1
    b en_mi_gl
en_mi_mkn:
    orr x9, x9, x22
    mov x0, x9
    bl ps_emitw
    b en_mi_e
en_mi_err:
    lea x3, g_ln
    ldr x0, [x3, #0]
    lea x1, m_immrange
    b fl_fail_ln
en_mi_e:
    ldp x20, x21, [sp, #16]
    ldp x22, x23, [sp, #32]
    ldp x29, x30, [sp], #48
    ret

# ----------------------------------------------------------------------------
# en_addfam(imm_base, reg_base, flip_imm, flip_reg, form)
# ----------------------------------------------------------------------------

en_addfam:
    stp x29, x30, [sp, #-96]!
    stp x19, x20, [sp, #16]
    stp x21, x22, [sp, #32]
    stp x23, x24, [sp, #48]
    stp x25, x26, [sp, #64]
    str x27, [sp, #80]
    mov x19, x0
    mov x20, x1
    mov x21, x2
    mov x22, x3
    mov x23, x4
    mov x24, 0
    cmp x23, 0
    b.ne en_af_cmp
    mov x0, 1
    bl en_reg
    mov x25, x0
    mov x26, x1
    mov x0, 2
    bl en_reg
    cmp x0, x25
    b.ne en_af_mixed
    mov x24, x1
    mov x23, 3
    b en_af_src
en_af_cmp:
    mov x0, 1
    bl en_reg
    mov x25, x0
    mov x26, x1
    mov x24, 31
    mov x23, 2
en_af_src:
    mov x0, x23
    bl en_isreg
    cbz x0, en_af_imm
    mov x0, x23
    bl en_reg
    cmp x0, x25
    b.eq en_af_regok
    cmp x1, 31
    b.eq en_af_regok
    b en_af_mixed
en_af_regok:
    lsl x0, x1, 16
    orr x0, x20, x0
    lsl x1, x26, 5
    orr x0, x0, x1
    orr x0, x0, x24
    mov x1, x25
    lsl x1, x1, 31
    orr x0, x0, x1
    mov x0, x0
    bl ps_emitw
    b en_af_e
en_af_imm:
    mov x0, x23
    bl en_imm
    mov x27, x0
    mov x9, -4095
    cmp x27, x9
    b.lt en_af_err12
    cmp x27, 0
    b.lt en_af_flip
    cmp x27, 4095
    b.gt en_af_err12
    b en_af_imm_n
en_af_flip:
    sub x27, xzr, x27
    lsl x27, x27, 10
    orr x0, x21, x27
    b en_af_fin
en_af_imm_n:
    lsl x27, x27, 10
    orr x0, x19, x27
en_af_fin:
    lsl x1, x26, 5
    orr x0, x0, x1
    orr x0, x0, x24
    mov x1, x25
    lsl x1, x1, 31
    orr x0, x0, x1
    bl ps_emitw
    b en_af_e
en_af_mixed:
    lea x3, g_ln
    ldr x0, [x3, #0]
    lea x1, m_mixed
    b fl_fail_ln
en_af_err12:
    lea x3, g_ln
    ldr x0, [x3, #0]
    lea x1, m_imm12
    b fl_fail_ln
en_af_e:
    ldp x19, x20, [sp, #16]
    ldp x21, x22, [sp, #32]
    ldp x23, x24, [sp, #48]
    ldp x25, x26, [sp, #64]
    ldr x27, [sp, #80]
    ldp x29, x30, [sp], #96
    ret

# ----------------------------------------------------------------------------
# en_r3m(base, rm, rn, rd): emit base|rm<<16|rn<<5|rd
# ----------------------------------------------------------------------------

en_r3m:
    lsl x1, x1, 16
    orr x0, x0, x1
    lsl x2, x2, 5
    orr x0, x0, x2
    orr x0, x0, x3
    b ps_emitw

# ----------------------------------------------------------------------------
# en_encode(ln): huvuddispatch for vanliga instruktioner
# ----------------------------------------------------------------------------

en_encode:
    stp x29, x30, [sp, #-96]!
    stp x19, x20, [sp, #16]
    stp x21, x22, [sp, #32]
    stp x23, x24, [sp, #48]
    stp x25, x26, [sp, #64]
    stp x27, x28, [sp, #80]
    mov x19, x0
    lea x3, g_ln
    str x19, [x3, #0]
    # ---- mov ----
    mov x0, 0
    lea x1, s_mov
    mov x2, 3
    bl h_tokeq
    cbz x0, en_c_movz
    mov x0, 1
    bl en_reg
    mov x20, x0
    mov x21, x1
    mov x0, 2
    bl en_isreg
    cbz x0, en_mov_i
    mov x0, 2
    bl en_reg
    mov x22, x1
    mov x0, 0xAA000000
    cmp x20, 1
    b.eq en_mov_rr
    mov x0, 0x2A000000
en_mov_rr:
    mov x1, x22
    mov x2, 31
    mov x3, x21
    bl en_r3m
    b en_e
en_mov_i:
    mov x0, 2
    bl en_imm
    mov x1, x0
    mov x0, x20
    mov x2, x21
    bl en_movimm
    b en_e
    # ---- movz/movn/movk ----
en_c_movz:
    mov x0, 0
    lea x1, s_movz
    mov x2, 4
    bl h_tokeq
    cbz x0, en_c_movn
    mov x24, 0x52800000
    b en_movzk
en_c_movn:
    mov x0, 0
    lea x1, s_movn
    mov x2, 4
    bl h_tokeq
    cbz x0, en_c_movk
    mov x24, 0x12800000
    b en_movzk
en_c_movk:
    mov x0, 0
    lea x1, s_movk
    mov x2, 4
    bl h_tokeq
    cbz x0, en_c_add
    mov x24, 0x72800000
en_movzk:
    mov x0, 1
    bl en_reg
    mov x20, x0
    mov x21, x1
    mov x0, 2
    bl en_imm
    mov x22, x0
    mov x23, 0
    lea x3, g_ntok
    ldr x3, [x3, #0]
    cmp x3, 5
    b.lt en_movzk_e
    mov x0, 3
    bl h_tokeq3
    cbz x0, en_movzk_e
    mov x0, 4
    bl en_imm
    lsr x23, x0, 4
en_movzk_e:
    mov x9, 0xFFFF
    and x9, x22, x9
    lsl x9, x9, 5
    orr x9, x24, x9
    lsl x10, x23, 21
    orr x9, x9, x10
    orr x9, x9, x21
    mov x0, x9
    bl ps_emitw
    b en_e
    # ---- add-familjen ----
en_c_add:
    mov x0, 0
    lea x1, s_add
    mov x2, 3
    bl h_tokeq
    cbz x0, en_c_sub
    mov x0, 0x11000000
    mov x1, 0x0B000000
    mov x2, 0x51000000
    mov x3, 0x4B000000
    mov x4, 0
    bl en_addfam
    b en_e
en_c_sub:
    mov x0, 0
    lea x1, s_sub
    mov x2, 3
    bl h_tokeq
    cbz x0, en_c_adds
    mov x0, 0x51000000
    mov x1, 0x4B000000
    mov x2, 0x11000000
    mov x3, 0x0B000000
    mov x4, 0
    bl en_addfam
    b en_e
en_c_adds:
    mov x0, 0
    lea x1, s_adds
    mov x2, 4
    bl h_tokeq
    cbz x0, en_c_subs
    mov x0, 0x31000000
    mov x1, 0x2B000000
    mov x2, 0x71000000
    mov x3, 0x6B000000
    mov x4, 0
    bl en_addfam
    b en_e
en_c_subs:
    mov x0, 0
    lea x1, s_subs
    mov x2, 4
    bl h_tokeq
    cbz x0, en_c_cmp
    mov x0, 0x71000000
    mov x1, 0x6B000000
    mov x2, 0x31000000
    mov x3, 0x2B000000
    mov x4, 0
    bl en_addfam
    b en_e
en_c_cmp:
    mov x0, 0
    lea x1, s_cmp
    mov x2, 3
    bl h_tokeq
    cbz x0, en_c_cmn
    mov x0, 0x71000000
    mov x1, 0x6B000000
    mov x2, 0x31000000
    mov x3, 0x2B000000
    mov x4, 1
    bl en_addfam
    b en_e
en_c_cmn:
    mov x0, 0
    lea x1, s_cmn
    mov x2, 3
    bl h_tokeq
    cbz x0, en_c_mul
    mov x0, 0x31000000
    mov x1, 0x2B000000
    mov x2, 0x71000000
    mov x3, 0x6B000000
    mov x4, 1
    bl en_addfam
    b en_e
    # ---- mul/smulh/umulh/udiv/sdiv ----
en_c_mul:
    mov x0, 0
    lea x1, s_mul
    mov x2, 3
    bl h_tokeq
    cbz x0, en_c_smulh
    mov x0, 1
    bl en_reg
    mov x20, x0
    mov x23, x1
    mov x0, 2
    bl en_reg
    mov x22, x1
    mov x0, 3
    bl en_reg
    mov x21, x1
    mov x0, 0x9B007C00
    cmp x20, 1
    b.eq en_c_mul2
    mov x0, 0x1B007C00
en_c_mul2:
    b en_r3all
en_c_smulh:
    mov x0, 0
    lea x1, s_smulh
    mov x2, 5
    bl h_tokeq
    cbz x0, en_c_umulh
    mov x24, 0x9B407C00
    b en_c_mul3
en_c_umulh:
    mov x0, 0
    lea x1, s_umulh
    mov x2, 5
    bl h_tokeq
    cbz x0, en_c_udiv
    mov x24, 0x9BC07C00
    b en_c_mul3
en_c_udiv:
    mov x0, 0
    lea x1, s_udiv
    mov x2, 4
    bl h_tokeq
    cbz x0, en_c_sdiv
    mov x24, 0x9AC00800
    b en_c_mul3
en_c_sdiv:
    mov x0, 0
    lea x1, s_sdiv
    mov x2, 4
    bl h_tokeq
    cbz x0, en_c_and
    mov x24, 0x9AC00C00
en_c_mul3:
    mov x0, 1
    bl en_reg
    mov x23, x1
    mov x0, 2
    bl en_reg
    mov x22, x1
    mov x0, 3
    bl en_reg
    mov x21, x1
    mov x0, x24
en_r3all:
    mov x1, x21
    mov x2, x22
    mov x3, x23
    bl en_r3m
    b en_e
    # ---- and/orr/eor ----
en_c_and:
    mov x0, 0
    lea x1, s_and
    mov x2, 3
    bl h_tokeq
    cbz x0, en_c_orr
    mov x24, 0x0A000000
    mov x25, 0x8A000000
    b en_c_logic
en_c_orr:
    mov x0, 0
    lea x1, s_orr
    mov x2, 3
    bl h_tokeq
    cbz x0, en_c_eor
    mov x24, 0x2A000000
    mov x25, 0x8A000000
    b en_c_logic
en_c_eor:
    mov x0, 0
    lea x1, s_eor
    mov x2, 3
    bl h_tokeq
    cbz x0, en_c_lsl
    mov x24, 0x4A000000
    mov x25, 0xCA000000
en_c_logic:
    mov x0, 1
    bl en_reg
    mov x20, x0
    mov x23, x1
    mov x0, 2
    bl en_reg
    mov x22, x1
    mov x0, 3
    bl en_reg
    mov x21, x1
    mov x0, x24
    cmp x20, 1
    b.eq en_c_logic2
    mov x0, x25
en_c_logic2:
    b en_r3all
    # ---- lsl/lsr/asr (register eller immediate) ----
en_c_lsl:
    mov x0, 0
    lea x1, s_lsl
    mov x2, 3
    bl h_tokeq
    cbz x0, en_c_lsr
    mov x24, 0
    b en_shift
en_c_lsr:
    mov x0, 0
    lea x1, s_lsr
    mov x2, 3
    bl h_tokeq
    cbz x0, en_c_asr
    mov x24, 1
    b en_shift
en_c_asr:
    mov x0, 0
    lea x1, s_asr
    mov x2, 3
    bl h_tokeq
    cbz x0, en_c_ror
    mov x24, 2
en_shift:
    mov x0, 1
    bl en_reg
    mov x20, x0
    mov x23, x1
    mov x0, 2
    bl en_reg
    mov x22, x1
    mov x0, 3
    bl en_isreg
    cbz x0, en_shift_imm
    mov x0, 3
    bl en_reg
    mov x21, x1
en_shift_regtab:
    # bas = c_shbase[(mn*2)+sf]
    lsl x7, x24, 1
    orr x7, x7, x20
    lsl x7, x7, 2
    lea x8, c_shbase
    ldr w9, [x8, x7]
    mov x0, x9
    mov x1, x21
    mov x2, x22
    mov x3, x23
    bl en_r3m
    b en_e
en_shift_imm:
    mov x0, 3
    bl en_imm
    mov x25, x0
    # bits = 64 om sf annars 32
    mov x26, 64
    cmp x20, 0
    b.ne en_sh_i_ok
    mov x26, 32
en_sh_i_ok:
    cmp x25, 0
    b.lt en_sh_err
    cmp x25, x26
    b.lt en_sh_i_v
en_sh_err:
    lea x3, g_ln
    ldr x0, [x3, #0]
    lea x1, m_shrange
    b fl_fail_ln
en_sh_i_v:
    # per mn: lsl immr/imms; lsr/asr immr=v imms=bits-1
    cmp x24, 0
    b.ne en_sh_i_lsr
    # lsl
    mov x27, 0
    sub x28, x26, 1
    cmp x25, 0
    b.eq en_sh_i_fin
    sub x27, x26, x25
    sub x28, x26, 1
    sub x28, x28, x25
    b en_sh_i_fin
en_sh_i_lsr:
    cmp x24, 1
    b.ne en_sh_i_asr
    mov x27, x25
    sub x28, x26, 1
    b en_sh_i_b
en_sh_i_asr:
    mov x27, x25
    sub x28, x26, 1
    b en_sh_i_a
en_sh_i_b:
    # lsl/lsr: ubfm-bas
    mov x9, 0xD3400000
    cmp x20, 0
    b.ne en_sh_i_go
    mov x9, 0x53000000
    b en_sh_i_go
en_sh_i_a:
    mov x9, 0x93400000
    cmp x20, 0
    b.ne en_sh_i_go
    mov x9, 0x13000000
en_sh_i_go:
    lsl x10, x27, 16
    orr x9, x9, x10
    lsl x10, x28, 10
    orr x9, x9, x10
    lsl x10, x22, 5
    orr x9, x9, x10
    orr x9, x9, x21
    mov x0, x9
    bl ps_emitw
    b en_e
en_sh_i_fin:
    b en_sh_i_b
    # ---- ror rd, rn, #imm ----
en_c_ror:
    mov x0, 0
    lea x1, s_ror
    mov x2, 3
    bl h_tokeq
    cbz x0, en_c_cset
    mov x0, 1
    bl en_reg
    mov x21, x1
    mov x0, 2
    bl en_reg
    mov x22, x1
    mov x0, 3
    bl en_imm
    mov x23, x0
    mov x20, 64
    mov x0, 1
    bl en_reg
    cmp x0, 1
    b.eq en_c_ror_b
    mov x20, 32
en_c_ror_b:
    udiv x25, x23, x20
    mul x26, x25, x20
    sub x25, x23, x26
    mov x9, 0x93C00000
    mov x0, 1
    bl en_reg
    cmp x0, 1
    b.eq en_c_ror_c
    mov x9, 0x13C00000
en_c_ror_c:
    lsl x10, x25, 10
    orr x9, x9, x10
    lsl x10, x22, 16
    orr x9, x9, x10
    lsl x10, x22, 5
    orr x9, x9, x10
    orr x9, x9, x21
    mov x0, x9
    bl ps_emitw
    b en_e
    # ---- cset ----
en_c_cset:
    mov x0, 0
    lea x1, s_cset
    mov x2, 4
    bl h_tokeq
    cbz x0, en_c_csel
    mov x0, 1
    bl en_reg
    mov x20, x0
    mov x21, x1
    mov x0, 2
    bl ps_tok_at
    bl en_condid
    mov x22, x0
    mov x9, 0x9A800400
    cmp x20, 1
    b.eq en_cset_b
    mov x9, 0x1A800400
en_cset_b:
    mov x10, 31
    lsl x10, x10, 16
    orr x9, x9, x10
    mov x10, 1
eor x22, x22, x10
    lsl x10, x22, 12
    orr x9, x9, x10
    mov x10, 31
    lsl x10, x10, 5
    orr x9, x9, x10
    orr x9, x9, x21
    mov x0, x9
    bl ps_emitw
    b en_e
    # ---- csel ----
en_c_csel:
    mov x0, 0
    lea x1, s_csel
    mov x2, 4
    bl h_tokeq
    cbz x0, en_c_sxtw
    mov x0, 1
    bl en_reg
    mov x20, x0
    mov x21, x1
    mov x0, 2
    bl en_reg
    mov x22, x1
    mov x0, 3
    bl en_reg
    mov x23, x1
    mov x0, 4
    bl ps_tok_at
    bl en_condid
    mov x24, x0
    mov x9, 0x9A800000
    cmp x20, 1
    b.eq en_csel_b
    mov x9, 0x1A800000
en_csel_b:
    lsl x10, x23, 16
    orr x9, x9, x10
    lsl x10, x24, 12
    orr x9, x9, x10
    lsl x10, x22, 5
    orr x9, x9, x10
    orr x9, x9, x21
    mov x0, x9
    bl ps_emitw
    b en_e
    # ---- sxtw/uxtw/rev ----
en_c_sxtw:
    mov x0, 0
    lea x1, s_sxtw
    mov x2, 4
    bl h_tokeq
    cbz x0, en_c_uxtw
    mov x24, 0x93407C00
    b en_c_ext
en_c_uxtw:
    mov x0, 0
    lea x1, s_uxtw
    mov x2, 4
    bl h_tokeq
    cbz x0, en_c_rev
    mov x24, 0xD3407C00
en_c_ext:
    mov x0, 1
    bl en_reg
    mov x21, x1
    mov x0, 2
    bl en_reg
    mov x22, x1
    mov x0, x24
    mov x1, 0
    mov x2, x22
    mov x3, x21
    bl en_r3m
    b en_e
en_c_rev:
    mov x0, 0
    lea x1, s_rev
    mov x2, 3
    bl h_tokeq
    cbz x0, en_c_stp
    mov x0, 1
    bl en_reg
    mov x20, x0
    mov x21, x1
    mov x0, 2
    bl en_reg
    mov x22, x1
    mov x9, 0xDAC00C00
    cmp x20, 1
    b.eq en_c_rev_b
    mov x9, 0x5AC00800
en_c_rev_b:
    lsl x10, x22, 5
    orr x9, x9, x10
    orr x9, x9, x21
    mov x0, x9
    bl ps_emitw
    b en_e
    # ---- stp/ldp ----
en_c_stp:
    mov x0, 0
    lea x1, s_stp
    mov x2, 3
    bl h_tokeq
    cbz x0, en_c_ldp
    mov x27, 0
    b en_pair
en_c_ldp:
    mov x0, 0
    lea x1, s_ldp
    mov x2, 3
    bl h_tokeq
    cbz x0, en_c_ldr
    mov x27, 1
en_pair:
    mov x0, 1
    bl en_reg
    mov x20, x0
    mov x21, x1
    mov x0, 2
    bl en_reg
    mov x22, x1
    mov x0, 3
    bl en_mem
    mov x23, x0
    mov x24, x1
    mov x25, x2
    mov x26, x6
    # pre: token efter close ar '!'-markor
    mov x9, 0
    add x10, x26, 1
    lea x3, g_ntok
    ldr x3, [x3, #0]
    cmp x10, x3
    b.ge en_pair_post
    lsl x4, x10, 4
    lea x5, g_toks
    add x5, x5, x4
    ldr x6, [x5, #8]
    cmp x6, -3
    b.ne en_pair_post
    mov x9, 1
    add x10, x10, 1
en_pair_post:
    # post: nästa token (efter ev. '!') ar vanlig och borjar pa '#'
    mov x11, 0
    lea x3, g_ntok
    ldr x3, [x3, #0]
    cmp x10, x3
    b.ge en_pair_pk
    mov x0, x10
    bl ps_tok_at
    cmp x1, 0
    b.lt en_pair_pk
    ldrb w4, [x0, #0]
    cmp w4, 35
    b.ne en_pair_pk
    mov x11, 1
en_pair_pk:
    cmp x11, 0
    b.eq en_pair_k
    cmp x24, 0
    b.ne en_pair_posterr
    mov x0, x10
    bl en_imm
    mov x25, x0
    mov x24, 1
en_pair_k:
    cmp x24, 1
    b.ne en_pair_errmi
    # skala och justering
    mov x8, 3
    cmp x20, 0
    b.ne en_pair_sc
    mov x8, 2
en_pair_sc:
    mov x10, 1
    lsl x10, x10, x8
    sub x10, x10, 1
    and x10, x25, x10
    cmp x10, 0
    b.ne en_pair_alignerr
    asr x25, x25, x8
    cmp x25, -64
    b.lt en_pair_rangerr
    cmp x25, 63
    b.gt en_pair_rangerr
    # bas: (x27 mn, x20 sf, pre=x9, post=x11)
    cmp x9, 0
    b.eq en_pair_nopre
    cmp x11, 0
    b.ne en_pair_comb
en_pair_nopre:
    lsl x7, x20, 3
    lsl x10, x9, 2
    orr x7, x7, x10
    lsl x10, x11, 1
    orr x7, x7, x10
    orr x7, x7, x27
    lsl x7, x7, 2
    lea x8, c_pairbase
    ldr w9, [x8, x7]
    # ord
    mov x10, 0x7F
    and x10, x25, x10
    lsl x10, x10, 15
    orr x9, x9, x10
    lsl x10, x22, 10
    orr x9, x9, x10
    lsl x10, x23, 5
    orr x9, x9, x10
    orr x9, x9, x21
    mov x0, x9
    bl ps_emitw
    b en_e
en_pair_comb:
    lea x3, g_ln
    ldr x0, [x3, #0]
    lea x1, m_pairaddr
    b fl_fail_ln
en_pair_posterr:
    lea x3, g_ln
    ldr x0, [x3, #0]
    lea x1, m_postidx
    b fl_fail_ln
en_pair_errmi:
    lea x3, g_ln
    ldr x0, [x3, #0]
    lea x1, m_pairimm
    b fl_fail_ln
en_pair_alignerr:
    lea x3, g_ln
    ldr x0, [x3, #0]
    lea x1, m_pairalign
    b fl_fail_ln
en_pair_rangerr:
    lea x3, g_ln
    ldr x0, [x3, #0]
    lea x1, m_pairrange
    b fl_fail_ln
    # ---- ldr-familjen: en_ldrstr(wform, size, mnid, immshift, rt) ----
en_c_ldr:
    mov x0, 0
    lea x1, s_ldr
    mov x2, 3
    bl h_tokeq
    cbz x0, en_c_str
    mov x24, 0
    b en_ldrmn
en_c_str:
    mov x0, 0
    lea x1, s_str
    mov x2, 3
    bl h_tokeq
    cbz x0, en_c_ldrb
    mov x24, 1
    b en_ldrmn
en_c_ldrb:
    mov x0, 0
    lea x1, s_ldrb
    mov x2, 4
    bl h_tokeq
    cbz x0, en_c_strb
    mov x24, 2
    b en_ldrmn
en_c_strb:
    mov x0, 0
    lea x1, s_strb
    mov x2, 4
    bl h_tokeq
    cbz x0, en_c_ldrh
    mov x24, 3
    b en_ldrmn
en_c_ldrh:
    mov x0, 0
    lea x1, s_ldrh
    mov x2, 4
    bl h_tokeq
    cbz x0, en_c_strh
    mov x24, 4
    b en_ldrmn
en_c_strh:
    mov x0, 0
    lea x1, s_strh
    mov x2, 4
    bl h_tokeq
    cbz x0, en_c_ldrsb
    mov x24, 5
    b en_ldrmn
en_c_ldrsb:
    mov x0, 0
    lea x1, s_ldrsb
    mov x2, 5
    bl h_tokeq
    cbz x0, en_c_ldrsw
    mov x24, 6
    b en_ldrmn
en_c_ldrsw:
    mov x0, 0
    lea x1, s_ldrsw
    mov x2, 5
    bl h_tokeq
    cbz x0, en_c_nop
    mov x24, 7
en_ldrmn:
    mov x0, 1
    bl en_reg
    mov x21, x0
    mov x25, x1
    # wform = 1 om sf==0
    mov x10, 1
eor x21, x21, x10
    # size och immshift per mn
    lea x8, c_ldsize
    ldr w9, [x8, x24]
    mov x22, x9
    lsr x22, x22, 4
    mov x10, 15
and x9, x9, x10
    mov x23, x9
    # ldr/str: size = 3 om x-form annars 2
    cmp x24, 0
    b.ne en_ldrmn_s1
    cmp x21, 0
    b.eq en_ldrmn_s1
    mov x22, 3
en_ldrmn_s1:
    cmp x24, 1
    b.ne en_ldrmn_go
    cmp x21, 0
    b.eq en_ldrmn_go
    mov x22, 3
en_ldrmn_go:
    mov x0, x21
    mov x1, x22
    mov x2, x24
    mov x3, x23
    mov x4, x25
    bl en_ldrstr
    b en_e
    # ---- nop/svc/brk/ret/br/blr ----
en_c_nop:
    mov x0, 0
    lea x1, s_nop
    mov x2, 3
    bl h_tokeq
    cbz x0, en_c_svc
    mov x0, 0xD503201F
    bl ps_emitw
    b en_e
en_c_svc:
    mov x0, 0
    lea x1, s_svc
    mov x2, 3
    bl h_tokeq
    cbz x0, en_c_brk
    mov x0, 1
    bl en_imm
    mov x9, 0xFFFF
    and x9, x0, x9
    lsl x9, x9, 5
    mov x10, 0xD4000001
    orr x9, x10, x9
    mov x0, x9
    bl ps_emitw
    b en_e
en_c_brk:
    mov x0, 0
    lea x1, s_brk
    mov x2, 3
    bl h_tokeq
    cbz x0, en_c_ret
    mov x0, 1
    bl en_imm
    mov x9, 0xFFFF
    and x9, x0, x9
    lsl x9, x9, 5
    mov x10, 0xD4200000
    orr x9, x10, x9
    mov x0, x9
    bl ps_emitw
    b en_e
en_c_ret:
    mov x0, 0
    lea x1, s_ret
    mov x2, 3
    bl h_tokeq
    cbz x0, en_c_br
    lea x3, g_ntok
    ldr x3, [x3, #0]
    cmp x3, 1
    b.ne en_ret_rn
    mov x0, 0xD65F03C0
    bl ps_emitw
    b en_e
en_ret_rn:
    mov x0, 1
    bl en_reg
    lsl x9, x1, 5
    mov x10, 0xD65F0000
    orr x9, x10, x9
    mov x0, x9
    bl ps_emitw
    b en_e
en_c_br:
    mov x0, 0
    lea x1, s_br
    mov x2, 2
    bl h_tokeq
    cbz x0, en_c_blr
    mov x0, 1
    bl en_reg
    lsl x9, x1, 5
    mov x10, 0xD61F0000
    orr x9, x10, x9
    mov x0, x9
    bl ps_emitw
    b en_e
en_c_blr:
    mov x0, 0
    lea x1, s_blr
    mov x2, 3
    bl h_tokeq
    cbz x0, en_c_bad
    mov x0, 1
    bl en_reg
    lsl x9, x1, 5
    mov x10, 0xD63F0000
    orr x9, x10, x9
    mov x0, x9
    bl ps_emitw
    b en_e
en_c_bad:
    lea x3, g_ln
    ldr x0, [x3, #0]
    lea x1, m_badins
    b fl_fail_ln
en_e:
    ldp x19, x20, [sp, #16]
    ldp x21, x22, [sp, #32]
    ldp x23, x24, [sp, #48]
    ldp x25, x26, [sp, #64]
    ldp x27, x28, [sp, #80]
    ldp x29, x30, [sp], #96
    ret

# ----------------------------------------------------------------------------
# en_ldrstr(wform, size, mnid, immshift, rt)
# ----------------------------------------------------------------------------

en_ldrstr:
    stp x29, x30, [sp, #-96]!
    stp x19, x20, [sp, #16]
    stp x21, x22, [sp, #32]
    stp x23, x24, [sp, #48]
    stp x25, x26, [sp, #64]
    mov x19, x0
    mov x20, x1
    mov x21, x2
    mov x22, x3
    mov x23, x4
    mov x0, 2
    bl en_mem
    mov x24, x0
    mov x25, x1
    mov x26, x2
    str x3, [sp, #0]
    str x4, [sp, #8]
    str x5, [sp, #80]
    cmp x25, 1
    b.ne en_ls_reg
    # immediatform
    cmp x26, 0
    b.lt en_ls_badoff
    mov x9, 1
    lsl x9, x9, x20
    sub x9, x9, 1
    and x9, x26, x9
    cmp x9, 0
    b.ne en_ls_badoff
    lsr x9, x26, 0
    add x10, x20, 12
    lsr x9, x26, x10
    cmp x9, 0
    b.ne en_ls_imm12
    lsr x26, x26, x22
    # bas fran tabell
    lsl x7, x21, 1
    orr x7, x7, x19
    lsl x7, x7, 2
    lea x8, c_ldbase
    ldr w9, [x8, x7]
    lsl x10, x26, 10
    orr x9, x9, x10
    lsl x10, x24, 5
    orr x9, x9, x10
    orr x9, x9, x23
    mov x0, x9
    bl ps_emitw
    b en_ls_e
en_ls_reg:
    cmp x25, 2
    b.ne en_ls_noidx
    ldr x3, [sp, #0]
    cmp x3, 1
    b.ne en_ls_idxw
    ldr x5, [sp, #80]
    mov x9, 0
    cmp x5, 0
    b.eq en_ls_s
    mov x9, 1
    cmp x5, x20
    b.eq en_ls_s
    b en_ls_lslerr
en_ls_s:
    lsl x9, x9, 12
    # registervariant: endast ldr/str/ldrb/strb
    cmp x21, 0
    b.eq en_ls_r_ldr
    cmp x21, 1
    b.eq en_ls_r_str
    cmp x21, 2
    b.eq en_ls_r_ldrb
    cmp x21, 3
    b.eq en_ls_r_strb
    b en_ls_noregoff
en_ls_r_ldr:
    lea x8, c_ldroff
    b en_ls_r_go
en_ls_r_str:
    lea x8, c_ldroff
    add x8, x8, 8
    b en_ls_r_go
en_ls_r_ldrb:
    lea x8, c_ldroff
    add x8, x8, 16
    b en_ls_r_go
en_ls_r_strb:
    lea x8, c_ldroff
    add x8, x8, 20
en_ls_r_go:
    cmp x21, 2
    b.ge en_ls_r_x
    lsl x7, x19, 2
    add x8, x8, x7
    b en_ls_r_l
en_ls_r_x:
    ldr w9, [x8, #0]
    b en_ls_r_fin
en_ls_r_l:
    ldr w9, [x8, #0]
en_ls_r_fin:
    ldr x4, [sp, #8]
    lsl x10, x4, 16
    orr x9, x9, x10
    lsl x10, x24, 5
    orr x9, x9, x10
    orr x9, x9, x23
    mov x0, x9
    bl ps_emitw
    b en_ls_e
en_ls_badoff:
    lea x3, g_ln
    ldr x0, [x3, #0]
    lea x1, m_badoff
    b fl_fail_ln
en_ls_imm12:
    lea x3, g_ln
    ldr x0, [x3, #0]
    lea x1, m_imm12o
    b fl_fail_ln
en_ls_idxw:
    lea x3, g_ln
    ldr x0, [x3, #0]
    lea x1, m_idxw
    b fl_fail_ln
en_ls_lslerr:
    lea x3, g_ln
    ldr x0, [x3, #0]
    lea x1, m_lslel
    b fl_fail_ln
en_ls_noregoff:
    lea x3, g_ln
    ldr x0, [x3, #0]
    lea x1, m_noregoff
    b fl_fail_ln
en_ls_noidx:
    lea x3, g_ln
    ldr x0, [x3, #0]
    lea x1, m_noidx
    b fl_fail_ln
en_ls_e:
    ldp x19, x20, [sp, #16]
    ldp x21, x22, [sp, #32]
    ldp x23, x24, [sp, #48]
    ldp x25, x26, [sp, #64]
    ldp x29, x30, [sp], #96
    ret

# ----------------------------------------------------------------------------
# Layout (port av seedasm-layouten)
# ----------------------------------------------------------------------------

# lb_layout: beraknar alla offset i globaler, validerar entry
lb_layout:
    stp x29, x30, [sp, #-96]!
    stp x19, x20, [sp, #16]
    stp x21, x22, [sp, #32]
    stp x23, x24, [sp, #48]
    stp x25, x26, [sp, #64]
    # textstorlek
    lea x3, g_tbufp
    ldr x3, [x3, #0]
    lea x4, g_tbuf
    sub x3, x3, x4
    lea x5, g_text_sz
    str x3, [x5, #0]
    lea x3, g_cbufp
    ldr x3, [x3, #0]
    lea x4, g_cbuf
    sub x3, x3, x4
    lea x5, g_csz
    str x3, [x5, #0]
    lea x3, g_dbufp
    ldr x3, [x3, #0]
    lea x4, g_dbuf
    sub x3, x3, x4
    lea x5, g_dsz
    str x3, [x5, #0]
    lea x3, g_bsssz
    ldr x3, [x3, #0]
    lea x5, g_bs_sz
    str x3, [x5, #0]
    # has-flaggor
    lea x3, g_csz
    ldr x3, [x3, #0]
    cmp x3, 0
    b.le lb_hc0
    mov x3, 1
    b lb_hc1
lb_hc0:
    mov x3, 0
lb_hc1:
    lea x5, g_has_c
    str x3, [x5, #0]
    lea x3, g_dsz
    ldr x3, [x3, #0]
    cmp x3, 0
    b.le lb_hd0
    mov x3, 1
    b lb_hd1
lb_hd0:
    mov x3, 0
lb_hd1:
    lea x5, g_has_d
    str x3, [x5, #0]
    lea x3, g_bs_sz
    ldr x3, [x3, #0]
    cmp x3, 0
    b.le lb_hb0
    mov x3, 1
    b lb_hb1
lb_hb0:
    mov x3, 0
lb_hb1:
    lea x5, g_has_b
    str x3, [x5, #0]
    lea x3, g_has_d
    ldr x3, [x3, #0]
    lea x4, g_has_b
    ldr x4, [x4, #0]
    orr x3, x3, x4
    lea x5, g_dseg
    str x3, [x5, #0]
    # nsects = 1 + has_c
    lea x3, g_has_c
    ldr x3, [x3, #0]
    add x3, x3, 1
    lea x5, g_nsects
    str x3, [x5, #0]
    # cmds = 72+72+80*n + (232 om dseg) + 72+24+32+24+16
    mov x19, 0
    lea x3, g_dseg
    ldr x3, [x3, #0]
    cmp x3, 0
    b.eq lb_cmd1
    mov x19, 232
lb_cmd1:
    lea x3, g_nsects
    ldr x3, [x3, #0]
    lsl x3, x3, 4
    lsl x4, x3, 2
    add x3, x3, x4
    # x3 = 80*nsects
    add x20, x3, x19
    add x20, x20, 72
    add x20, x20, 72
    add x20, x20, 24
    add x20, x20, 32
    add x20, x20, 24
    add x20, x20, 16
    lea x3, g_cmdsz
    str x20, [x3, #0]
    # text_off = align16(32+cmdsz)
    add x20, x20, 32
    add x20, x20, 15
    lsr x21, x20, 4
    lsl x21, x21, 4
    lea x3, g_text_off
    str x21, [x3, #0]
    # text_end
    lea x3, g_text_sz
    ldr x3, [x3, #0]
    add x22, x21, x3
    # const_off
    lea x3, g_has_c
    ldr x3, [x3, #0]
    cmp x3, 0
    b.eq lb_noc
    add x23, x22, 15
    lsr x23, x23, 4
    lsl x23, x23, 4
    b lb_hasc
lb_noc:
    mov x23, 0
lb_hasc:
    lea x3, g_const_off
    str x23, [x3, #0]
    # const_end
    lea x3, g_has_c
    ldr x3, [x3, #0]
    cmp x3, 0
    b.eq lb_ce0
    lea x3, g_const_off
    ldr x3, [x3, #0]
    lea x4, g_csz
    ldr x4, [x4, #0]
    add x24, x3, x4
    b lb_ce1
lb_ce0:
    mov x24, x22
lb_ce1:
    str x24, [sp, #80]
    lea x3, g_cend
    str x24, [x3, #0]
    # data_off = align16k(const_end)
    lsr x25, x24, 14
    mov x10, 16383
    and x10, x24, x10
    cmp x10, 0
    cset x10, ne
    add x25, x25, x10
    lsl x25, x25, 14
    lea x3, g_data_off
    str x25, [x3, #0]
    # bsoff = align16(dsz)
    lea x3, g_dsz
    ldr x3, [x3, #0]
    add x3, x3, 15
    lsr x3, x3, 4
    lsl x3, x3, 4
    lea x5, g_bsoff
    str x3, [x5, #0]
    # sig_off = data_off + dsz
    lea x3, g_dsz
    ldr x3, [x3, #0]
    add x26, x25, x3
    lea x3, g_sig_off
    str x26, [x3, #0]
    # nslots
    lsr x3, x26, 14
    mov x10, 16383
    and x10, x26, x10
    cmp x10, 0
    cset x10, ne
    add x3, x3, x10
    lea x5, g_nslots
    str x3, [x5, #0]
    # hashoff = 88 + ident_n + 1
    lea x3, g_ident_n
    ldr x3, [x3, #0]
    add x3, x3, 89
    lea x5, g_hashoff
    str x3, [x5, #0]
    # cdlen = hashoff + 32*nslots
    lsl x4, x3, 5
    lea x3, g_hashoff
    ldr x3, [x3, #0]
    add x3, x3, x4
    lea x5, g_cdlen
    str x3, [x5, #0]
    # datasize = align16(20+cdlen)
    add x3, x3, 20
    add x3, x3, 15
    lsr x3, x3, 4
    lsl x3, x3, 4
    lea x5, g_datasize
    str x3, [x5, #0]
    # textvmsz = align16k(const_end)
    ldr x4, [sp, #80]
    lsr x4, x4, 14
    mov x10, 16383
    and x10, x4, x10
    cmp x10, 0
    cset x10, ne
    add x4, x4, x10
    lsl x4, x4, 14
    lea x5, g_textvmsz
    str x4, [x5, #0]
    # entry
    lea x3, g_entry_p
    ldr x3, [x3, #0]
    cmp x3, 0
    b.ne lb_entry
    lea x1, m_noentry
    mov x0, 0
    b fl_fail_ln
lb_entry:
    lea x3, g_entry_p
    ldr x0, [x3, #0]
    lea x3, g_entry_n
    ldr x1, [x3, #0]
    bl ps_lab_find
    cmp x0, 0
    b.ge lb_entok
    lea x1, m_noentlab
    mov x0, 0
    b fl_fail_ln
lb_entok:
    bl fx_addr
    mov x1, 0x100000000
    sub x0, x0, x1
    lea x3, g_entryoff
    str x0, [x3, #0]
    ldp x19, x20, [sp, #16]
    ldp x21, x22, [sp, #32]
    ldp x23, x24, [sp, #48]
    ldp x25, x26, [sp, #64]
    ldp x29, x30, [sp], #96
    ret

# fx_addr(labidx) -> x0 = VM-adress
fx_addr:
    lsl x1, x0, 4
    lsr x2, x1, 1
    add x2, x1, x2
    lea x3, g_labs
    add x3, x3, x2
    ldr x4, [x3, #16]
    ldr x5, [x3, #24]
    cmp x4, 0
    b.ne fx_a_c
    lea x6, g_text_off
    ldr x6, [x6, #0]
    add x0, x5, x6
    b fx_a_f
fx_a_c:
    cmp x4, 1
    b.ne fx_a_d
    lea x6, g_const_off
    ldr x6, [x6, #0]
    add x0, x5, x6
    b fx_a_f
fx_a_d:
    cmp x4, 2
    b.ne fx_a_b
    lea x6, g_data_off
    ldr x6, [x6, #0]
    add x0, x5, x6
    b fx_a_f
fx_a_b:
    lea x6, g_data_off
    ldr x6, [x6, #0]
    lea x7, g_bsoff
    ldr x7, [x7, #0]
    add x0, x6, x7
    add x0, x0, x5
fx_a_f:
    mov x6, 0x100000000
    add x0, x0, x6
    ret

# fx_resolve: loser alla fixups
fx_resolve:
    stp x29, x30, [sp, #-80]!
    stp x19, x20, [sp, #16]
    stp x21, x22, [sp, #32]
    stp x23, x24, [sp, #48]
    lea x3, g_nfix
    ldr x19, [x3, #0]
    mov x20, 0
fx_r_l:
    cmp x20, x19
    b.ge fx_r_e
    lsl x21, x20, 6
    lsr x22, x21, 3
    sub x21, x21, x22
    lea x3, g_fixs
    add x3, x3, x21
    ldr x4, [x3, #0]
    ldr x5, [x3, #8]
    ldr x6, [x3, #16]
    ldr x7, [x3, #24]
    ldr x8, [x3, #32]
    ldr x10, [x3, #40]
    str x3, [sp, #64]
    # hitta etikett
    mov x0, x7
    mov x1, x8
    bl ps_lab_find
    cmp x0, 0
    b.ge fx_r_ok
    lea x1, m_uklab
    mov x0, x5
    b fl_fail_ln
fx_r_ok:
    str x0, [sp, #72]
    bl fx_addr
    mov x11, x0
    # pc = VM + text_off + tbufoff
    lea x3, g_text_off
    ldr x3, [x3, #0]
    add x12, x3, x10
    mov x13, 0x100000000
    add x12, x12, x13
    ldr x3, [sp, #64]
    ldr x4, [x3, #0]
    # off = (target - pc) >> 2
    sub x14, x11, x12
    asr x14, x14, 2
    # skrivposition i tbuf
    lea x15, g_tbuf
    add x15, x15, x10
    cmp x4, 0
    b.eq fx_r_b
    cmp x4, 1
    b.eq fx_r_bl
    cmp x4, 2
    b.eq fx_r_bc
    cmp x4, 3
    b.eq fx_r_cbz
    cmp x4, 4
    b.eq fx_r_cbnz
    cmp x4, 5
    b.eq fx_r_tbz
    cmp x4, 6
    b.eq fx_r_tbnz
    b fx_r_lea
fx_r_b:
    mov x10, 0x2000000
    add x9, x14, x10
    mov x10, 0x4000000
    cmp x9, x10
    b.hs fx_r_range
    mov x9, 0x3FFFFFF
    and x9, x14, x9
    mov x10, 0x14000000
    orr x9, x10, x9
    str w9, [x15, #0]
    b fx_r_n
fx_r_bl:
    mov x10, 0x2000000
    add x9, x14, x10
    mov x10, 0x4000000
    cmp x9, x10
    b.hs fx_r_range
    mov x9, 0x3FFFFFF
    and x9, x14, x9
    mov x10, 0x94000000
    orr x9, x10, x9
    str w9, [x15, #0]
    b fx_r_n
fx_r_bc:
    mov x10, 0x40000
    add x9, x14, x10
    mov x10, 0x80000
    cmp x9, x10
    b.hs fx_r_range
    mov x9, 0x7FFFF
    and x9, x14, x9
    lsl x9, x9, 5
    mov x10, 0x54000000
    orr x9, x10, x9
    orr x9, x9, x6
    str w9, [x15, #0]
    b fx_r_n
fx_r_cbz:
    mov x10, 0x80000
    add x9, x14, x10
    mov x10, 0x100000
    cmp x9, x10
    b.hs fx_r_range
    mov x9, 0xFFFFF
    and x9, x14, x9
    lsl x9, x9, 5
    lsr x16, x6, 8
    mov x10, 1
and x16, x16, x10
    cmp x16, 0
    b.ne fx_r_cbz64
    mov x10, 0x34000000
    b fx_r_cbzfin
fx_r_cbz64:
    mov x10, 0xB4000000
fx_r_cbzfin:
    orr x9, x9, x10
    mov x10, 0xFF
and x16, x6, x10
    orr x9, x9, x16
    str w9, [x15, #0]
    b fx_r_n
fx_r_cbnz:
    mov x10, 0x80000
    add x9, x14, x10
    mov x10, 0x100000
    cmp x9, x10
    b.hs fx_r_range
    mov x9, 0xFFFFF
    and x9, x14, x9
    lsl x9, x9, 5
    lsr x16, x6, 8
    mov x10, 1
and x16, x16, x10
    cmp x16, 0
    b.ne fx_r_cbnz64
    mov x10, 0x35000000
    b fx_r_cbnzfin
fx_r_cbnz64:
    mov x10, 0xB5000000
fx_r_cbnzfin:
    orr x9, x9, x10
    mov x10, 0xFF
and x16, x6, x10
    orr x9, x9, x16
    str w9, [x15, #0]
    b fx_r_n
fx_r_tbz:
    mov x17, 0x36000000
    b fx_r_tbz_go
fx_r_tbnz:
    mov x17, 0x37000000
fx_r_tbz_go:
    mov x10, 0x2000
    add x9, x14, x10
    mov x10, 0x4000
    cmp x9, x10
    b.hs fx_r_range
    mov x9, 0x3FFF
    and x9, x14, x9
    lsl x9, x9, 5
    orr x9, x9, x17
    mov x10, 0x3F
and x11, x6, x10
    lsr x10, x11, 5
    lsl x10, x10, 31
    orr x9, x9, x10
    mov x10, 0x1F
and x10, x11, x10
    lsl x10, x10, 19
    orr x9, x9, x10
    lsr x10, x6, 8
    mov x11, 0x1F
and x10, x10, x11
    orr x9, x9, x10
    str w9, [x15, #0]
    b fx_r_n
fx_r_lea:
    # diff_pages = ((t & ~0xFFF) - (pc & ~0xFFF)) >> 12
    mov x10, 0xFFF
    mov x9, -1
    eor x10, x10, x9
    and x9, x11, x10
    and x4, x12, x10
    mov x10, 0xFFF
    and x12, x11, x10
    sub x9, x9, x4
    asr x9, x9, 12
    mov x10, 0x100000
    add x10, x9, x10
    mov x11, 0x200000
    cmp x10, x11
    b.hs fx_r_range
    mov x10, 3
    and x13, x9, x10
    lsl x13, x13, 29
    mov x10, 0x90000000
    orr x13, x10, x13
    lsr x10, x9, 2
    mov x11, 0x7FFFF
and x10, x10, x11
    lsl x10, x10, 5
    orr x13, x13, x10
    orr x13, x13, x6
    str w13, [x15, #0]
    lsl x10, x12, 10
    mov x13, 0x91000000
    orr x13, x13, x10
    lsl x10, x6, 5
    orr x13, x13, x10
    orr x13, x13, x6
    str w13, [x15, #4]
fx_r_n:
    add x20, x20, 1
    b fx_r_l
fx_r_range:
    lea x1, m_brreach
    mov x0, x5
    b fl_fail_ln
fx_r_e:
    ldp x19, x20, [sp, #16]
    ldp x21, x22, [sp, #32]
    ldp x23, x24, [sp, #48]
    ldp x29, x30, [sp], #80
    ret

# ----------------------------------------------------------------------------
# sh_sha256(msg, len, out): 32-byte BIG-endian digest i out
# ----------------------------------------------------------------------------

sh_sha256:
    stp x29, x30, [sp, #-128]!
    stp x19, x20, [sp, #16]
    stp x21, x22, [sp, #32]
    stp x23, x24, [sp, #48]
    stp x25, x26, [sp, #64]
    stp x27, x28, [sp, #80]
    str x0, [sp, #96]
    str x1, [sp, #104]
    str x2, [sp, #112]
    # H = init-konstanter
    lea x9, sh_h
    lea x10, g_H
    ldr w11, [x9, #0]
    str w11, [x10, #0]
    ldr w11, [x9, #4]
    str w11, [x10, #4]
    ldr w11, [x9, #8]
    str w11, [x10, #8]
    ldr w11, [x9, #12]
    str w11, [x10, #12]
    ldr w11, [x9, #16]
    str w11, [x10, #16]
    ldr w11, [x9, #20]
    str w11, [x10, #20]
    ldr w11, [x9, #24]
    str w11, [x10, #24]
    ldr w11, [x9, #28]
    str w11, [x10, #28]
    lea x27, sh_k
    lea x28, g_W
    # blockcount = (len + 63) >> 6
    ldr x9, [sp, #104]
    add x9, x9, 63
    lsr x9, x9, 6
    str x9, [sp, #120]
    mov x14, 0
sh_blk:
    ldr x9, [sp, #120]
    cmp x14, x9
    b.ge sh_fin
    # W[0..15] = rev32(msg-ord)
    ldr x0, [sp, #96]
    lsl x1, x14, 6
    add x0, x0, x1
    mov x15, 0
sh_ld2:
    cmp x15, 16
    b.ge sh_exp
    lsl x1, x15, 2
    ldr w9, [x0, x1]
    rev w9, w9
    str w9, [x28, x1]
    add x15, x15, 1
    b sh_ld2
sh_exp:
    mov x15, 16
sh_exp_l:
    cmp x15, 64
    b.ge sh_comp
    lsl x16, x15, 2
    # w15 = W[j-15]
    sub x1, x16, 60
    ldr w9, [x28, x1]
    ror w10, w9, 7
    ror w11, w9, 18
    eor w10, w10, w11
    lsr w11, w9, 3
    eor w10, w10, w11
    # w2 = W[j-2]
    sub x1, x16, 8
    ldr w11, [x28, x1]
    ror w12, w11, 17
    ror w13, w11, 19
    eor w12, w12, w13
    lsr w13, w11, 10
    eor w12, w12, w13
    # W[j] = W[j-16] + s0 + W[j-7] + s1
    sub x1, x16, 64
    ldr w13, [x28, x1]
    add w13, w13, w10
    sub x1, x16, 28
    ldr w9, [x28, x1]
    add w13, w13, w9
    add w13, w13, w12
    str w13, [x28, x16]
    add x15, x15, 1
    b sh_exp_l
sh_comp:
    # a..h = H
    lea x9, g_H
    ldr w19, [x9, #0]
    ldr w20, [x9, #4]
    ldr w21, [x9, #8]
    ldr w22, [x9, #12]
    ldr w23, [x9, #16]
    ldr w24, [x9, #20]
    ldr w25, [x9, #24]
    ldr w26, [x9, #28]
    mov x15, 0
sh_round:
    cmp x15, 64
    b.ge sh_acc
    lsl x16, x15, 2
    # S1 = ror(e,6)^ror(e,11)^ror(e,25)
    ror w9, w23, 6
    ror w10, w23, 11
    eor w9, w9, w10
    ror w10, w23, 25
    eor w9, w9, w10
    # ch = (e&f)^(~e&g)
    and w10, w23, w24
    eor w11, w23, wzr
    and w11, w11, w25
    eor w10, w10, w11
    # t1 = h + S1 + ch + K + W
    add w9, w26, w9
    add w9, w9, w10
    ldr w11, [x27, x16]
    add w9, w9, w11
    ldr w11, [x28, x16]
    add w9, w9, w11
    # S0 = ror(a,2)^ror(a,13)^ror(a,22)
    ror w10, w19, 2
    ror w12, w19, 13
    eor w10, w10, w12
    ror w12, w19, 22
    eor w10, w10, w12
    # maj = (a&b)^(a&c)^(b&c)
    and w12, w19, w20
    and w13, w19, w21
    eor w12, w12, w13
    and w13, w20, w21
    eor w12, w12, w13
    add w10, w10, w12
    # rotera
    mov w26, w25
    mov w25, w24
    mov w24, w23
    add w23, w22, w9
    mov w22, w21
    mov w21, w20
    mov w20, w19
    mov w19, w10
    add x15, x15, 1
    b sh_round
sh_acc:
    lea x9, g_H
    ldr w11, [x9, #0]
    add w11, w11, w19
    str w11, [x9, #0]
    ldr w11, [x9, #4]
    add w11, w11, w20
    str w11, [x9, #4]
    ldr w11, [x9, #8]
    add w11, w11, w21
    str w11, [x9, #8]
    ldr w11, [x9, #12]
    add w11, w11, w22
    str w11, [x9, #12]
    ldr w11, [x9, #16]
    add w11, w11, w23
    str w11, [x9, #16]
    ldr w11, [x9, #20]
    add w11, w11, w24
    str w11, [x9, #20]
    ldr w11, [x9, #24]
    add w11, w11, w25
    str w11, [x9, #24]
    ldr w11, [x9, #28]
    add w11, w11, w26
    str w11, [x9, #28]
    add x14, x14, 1
    b sh_blk
sh_fin:
    # skriv 8 ord BIG-endian (rev + 4 strb)
    ldr x0, [sp, #112]
    lea x9, g_H
    mov x10, 0
sh_fin_w:
    cmp x10, 8
    b.ge sh_e
    lsl x1, x10, 2
    ldr w11, [x9, x1]
    rev w11, w11
    strb w11, [x0, #0]
    lsr w11, w11, 8
    strb w11, [x0, #1]
    lsr w11, w11, 8
    strb w11, [x0, #2]
    lsr w11, w11, 8
    strb w11, [x0, #3]
    add x0, x0, 4
    add x10, x10, 1
    b sh_fin_w
sh_e:
    ldp x19, x20, [sp, #16]
    ldp x21, x22, [sp, #32]
    ldp x23, x24, [sp, #48]
    ldp x25, x26, [sp, #64]
    ldp x27, x28, [sp, #80]
    ldp x29, x30, [sp], #128
    ret

# ----------------------------------------------------------------------------
# Mach-O-bygge
# ----------------------------------------------------------------------------

# mm_cpy(dst, src, n)
mm_cpy:
    mov x3, 0
mm_cpy_l:
    cmp x3, x2
    b.ge mm_cpy_e
    add x4, x0, x3
    add x5, x1, x3
    ldrb w6, [x5, #0]
    strb w6, [x4, #0]
    add x3, x3, 1
    b mm_cpy_l
mm_cpy_e:
    ret

# mm_name16(addr, ptr, len): 16 byte namn, nollstallt + kopierat
mm_name16:
    str xzr, [x0, #0]
    str xzr, [x0, #8]
    mov x3, 0
mm_name16_l:
    cmp x3, x2
    b.ge mm_name16_e
    cmp x3, 16
    b.ge mm_name16_e
    add x4, x1, x3
    ldrb w5, [x4, #0]
    add x4, x0, x3
    strb w5, [x4, #0]
    add x3, x3, 1
    b mm_name16_l
mm_name16_e:
    ret

# mm_be32(addr, val)
mm_be32:
    rev w9, w1
    str w9, [x0, #0]
    ret

# mm_sect64(x0=addr, x1=nameptr, x2=namelen, x3=segptr, x4=seglen,
#           x5=vmaddr, x6=size, x7=fileoff, x8=flags)
mm_sect64:
    stp x29, x30, [sp, #-64]!
    stp x3, x4, [sp, #16]
    str x5, [sp, #24]
    str x6, [sp, #32]
    str x7, [sp, #40]
    str x8, [sp, #48]
    bl mm_name16
    ldr x1, [sp, #16]
    ldr x2, [sp, #24]
    add x0, x0, 16
    bl mm_name16
    add x0, x0, 16
    ldr x5, [sp, #24]
    ldr x6, [sp, #32]
    str x5, [x0, #0]
    str x6, [x0, #8]
    add x0, x0, 16
    ldr x7, [sp, #40]
    str w7, [x0, #0]
    mov w9, 4
    str w9, [x0, #4]
    str wzr, [x0, #8]
    str wzr, [x0, #12]
    ldr w9, [sp, #48]
    str w9, [x0, #16]
    str wzr, [x0, #20]
    str wzr, [x0, #24]
    str wzr, [x0, #28]
    ldp x29, x30, [sp], #64
    ret

# mm_build
mm_build:
    stp x29, x30, [sp, #-112]!
    stp x19, x20, [sp, #16]
    stp x21, x22, [sp, #32]
    stp x23, x24, [sp, #48]
    stp x25, x26, [sp, #64]
    stp x27, x28, [sp, #80]
    lea x19, g_imgbuf
    mov x20, x19
    # --- mach_header_64 ---
    mov x0, 0xFEEDFACF
    str w0, [x20, #0]
    mov x0, 0x0100000C
    str w0, [x20, #4]
    str wzr, [x20, #8]
    mov x0, 2
    str w0, [x20, #12]
    # ncmds = 7 + dseg
    lea x3, g_dseg
    ldr x3, [x3, #0]
    add x0, x3, 7
    str w0, [x20, #16]
    lea x3, g_cmdsz
    ldr x3, [x3, #0]
    str w3, [x20, #20]
    mov x0, 0x200001
    str w0, [x20, #24]
    str wzr, [x20, #28]
    add x20, x20, 32
    # --- __PAGEZERO ---
    mov w9, 0x19
    str w9, [x20, #0]
    mov w9, 72
    str w9, [x20, #4]
    add x0, x20, 8
    lea x1, c_pgzero
    mov x2, 10
    bl mm_name16
    add x20, x20, 24
    str xzr, [x20, #0]
    mov x9, 0x100000000
    str x9, [x20, #8]
    str xzr, [x20, #16]
    str xzr, [x20, #24]
    str wzr, [x20, #32]
    str wzr, [x20, #36]
    str wzr, [x20, #40]
    str wzr, [x20, #44]
    add x20, x20, 48
    # --- __TEXT ---
    mov w9, 0x19
    str w9, [x20, #0]
    # cmdsize = 72 + 80*nsects
    lea x3, g_nsects
    ldr x3, [x3, #0]
    lsl x4, x3, 6
    lsl x5, x3, 4
    add x4, x4, x5
    mov x5, 72
    add x4, x4, x5
    str w4, [x20, #4]
    add x0, x20, 8
    lea x1, c_text
    mov x2, 6
    bl mm_name16
    add x20, x20, 24
    mov x9, 0x100000000
    str x9, [x20, #0]
    lea x3, g_textvmsz
    ldr x3, [x3, #0]
    str x3, [x20, #8]
    str xzr, [x20, #16]
    lea x3, g_cend
    ldr x3, [x3, #0]
    str x3, [x20, #24]
    mov w9, 5
    str w9, [x20, #32]
    str w9, [x20, #36]
    lea x3, g_nsects
    ldr x3, [x3, #0]
    str w3, [x20, #40]
    str wzr, [x20, #44]
    add x20, x20, 48
    # __text-sektion
    mov x0, x20
    lea x1, c_textn
    mov x2, 6
    lea x3, c_text
    mov x4, 6
    lea x5, g_text_off
    ldr x5, [x5, #0]
    mov x9, 0x100000000
    add x5, x5, x9
    lea x6, g_text_sz
    ldr x6, [x6, #0]
    lea x7, g_text_off
    ldr x7, [x7, #0]
    mov x8, xzr
    bl mm_sect64
    add x20, x20, 80
    # ev __const
    lea x3, g_has_c
    ldr x3, [x3, #0]
    cmp x3, 0
    b.eq mm_data
    mov x0, x20
    lea x1, c_constn
    mov x2, 7
    lea x3, c_text
    mov x4, 6
    lea x5, g_const_off
    ldr x5, [x5, #0]
    mov x9, 0x100000000
    add x5, x5, x9
    lea x6, g_csz
    ldr x6, [x6, #0]
    lea x7, g_const_off
    ldr x7, [x7, #0]
    mov x8, xzr
    bl mm_sect64
    add x20, x20, 80
mm_data:
    lea x3, g_dseg
    ldr x3, [x3, #0]
    cmp x3, 0
    b.eq mm_linkedit
    mov w9, 0x19
    str w9, [x20, #0]
    mov w9, 232
    str w9, [x20, #4]
    add x0, x20, 8
    lea x1, c_data
    mov x2, 6
    bl mm_name16
    add x20, x20, 24
    lea x3, g_data_off
    ldr x3, [x3, #0]
    mov x9, 0x100000000
    add x3, x3, x9
    str x3, [x20, #0]
    # vmsize = align16k(dsz + bsoff + bs)
    lea x3, g_dsz
    ldr x3, [x3, #0]
    lea x4, g_bsoff
    ldr x4, [x4, #0]
    add x3, x3, x4
    lea x4, g_bs_sz
    ldr x4, [x4, #0]
    add x3, x3, x4
    lsr x3, x3, 14
    mov x4, 16383
    and x4, x3, x4
    cmp x4, 0
    cset x4, ne
    add x3, x3, x4
    lsl x3, x3, 14
    str x3, [x20, #8]
    lea x3, g_data_off
    ldr x3, [x3, #0]
    str x3, [x20, #16]
    lea x3, g_dsz
    ldr x3, [x3, #0]
    str x3, [x20, #24]
    mov w9, 3
    str w9, [x20, #32]
    str w9, [x20, #36]
    mov w9, 2
    str w9, [x20, #40]
    str wzr, [x20, #44]
    add x20, x20, 48
    # __data
    mov x0, x20
    lea x1, c_datan
    mov x2, 6
    lea x3, c_data
    mov x4, 6
    lea x5, g_data_off
    ldr x5, [x5, #0]
    mov x9, 0x100000000
    add x5, x5, x9
    lea x6, g_dsz
    ldr x6, [x6, #0]
    lea x7, g_data_off
    ldr x7, [x7, #0]
    mov x8, xzr
    bl mm_sect64
    add x20, x20, 80
    # __bss
    mov x0, x20
    lea x1, c_bssn
    mov x2, 5
    lea x3, c_data
    mov x4, 6
    lea x5, g_data_off
    ldr x5, [x5, #0]
    lea x6, g_bsoff
    ldr x6, [x6, #0]
    add x5, x5, x6
    mov x9, 0x100000000
    add x5, x5, x9
    lea x6, g_bs_sz
    ldr x6, [x6, #0]
    mov x7, 0
    mov x8, 1
    bl mm_sect64
    add x20, x20, 80
mm_linkedit:
    mov w9, 0x19
    str w9, [x20, #0]
    mov w9, 72
    str w9, [x20, #4]
    add x0, x20, 8
    lea x1, c_linkedit
    mov x2, 10
    bl mm_name16
    add x20, x20, 24
    lea x3, g_sig_off
    ldr x3, [x3, #0]
    mov x9, 0x100000000
    add x3, x3, x9
    str x3, [x20, #0]
    lea x3, g_datasize
    ldr x3, [x3, #0]
    lsr x3, x3, 14
    mov x4, 16383
    and x4, x3, x4
    cmp x4, 0
    cset x4, ne
    add x3, x3, x4
    lsl x3, x3, 14
    str x3, [x20, #8]
    lea x3, g_sig_off
    ldr x3, [x3, #0]
    str x3, [x20, #16]
    lea x3, g_datasize
    ldr x3, [x3, #0]
    str x3, [x20, #24]
    mov w9, 1
    str w9, [x20, #32]
    str w9, [x20, #36]
    str wzr, [x20, #40]
    str wzr, [x20, #44]
    add x20, x20, 48
    # --- LC_BUILD_VERSION ---
    mov w9, 0x32
    str w9, [x20, #0]
    mov w9, 24
    str w9, [x20, #4]
    mov w9, 1
    str w9, [x20, #8]
    mov w9, 0xB0000
    str w9, [x20, #12]
    str w9, [x20, #16]
    str wzr, [x20, #20]
    add x20, x20, 24
    # --- LC_LOAD_DYLINKER ---
    mov w9, 0xE
    str w9, [x20, #0]
    mov w9, 32
    str w9, [x20, #4]
    mov w9, 12
    str w9, [x20, #8]
    add x0, x20, 12
    lea x1, c_dyldp
    mov x2, 14
    bl mm_cpy
    add x20, x20, 32
    # --- LC_MAIN ---
    mov w9, 0x80000028
    str w9, [x20, #0]
    mov w9, 24
    str w9, [x20, #4]
    lea x3, g_entryoff
    ldr x3, [x3, #0]
    str x3, [x20, #8]
    str xzr, [x20, #16]
    add x20, x20, 24
    # --- LC_CODE_SIGNATURE ---
    mov w9, 0x1D
    str w9, [x20, #0]
    mov w9, 16
    str w9, [x20, #4]
    lea x3, g_sig_off
    ldr x3, [x3, #0]
    str w3, [x20, #8]
    lea x3, g_datasize
    ldr x3, [x3, #0]
    str w3, [x20, #12]
    # --- kopiera sektioner ---
    lea x3, g_text_off
    ldr x0, [x3, #0]
    add x0, x0, x19
    lea x1, g_tbuf
    lea x2, g_text_sz
    ldr x2, [x2, #0]
    bl mm_cpy
    lea x3, g_has_c
    ldr x3, [x3, #0]
    cmp x3, 0
    b.eq mm_bdata
    lea x3, g_const_off
    ldr x0, [x3, #0]
    add x0, x0, x19
    lea x1, g_cbuf
    lea x2, g_csz
    ldr x2, [x2, #0]
    bl mm_cpy
mm_bdata:
    lea x3, g_dseg
    ldr x3, [x3, #0]
    cmp x3, 0
    b.eq mm_sig
    lea x3, g_data_off
    ldr x0, [x3, #0]
    add x0, x0, x19
    lea x1, g_dbuf
    lea x2, g_dsz
    ldr x2, [x2, #0]
    bl mm_cpy
mm_sig:
    # --- superblob + codedirectory ---
    lea x3, g_sig_off
    ldr x21, [x3, #0]
    add x22, x19, x21
    lea x3, g_datasize
    ldr x3, [x3, #0]
    str x3, [sp, #88]
    mov x0, x22
    mov x1, 0xFADE0CC0
    bl mm_be32
    ldr x1, [sp, #88]
    add x0, x22, 4
    bl mm_be32
    mov x1, 1
    add x0, x22, 8
    bl mm_be32
    mov x1, 0
    add x0, x22, 12
    bl mm_be32
    mov x1, 20
    add x0, x22, 16
    bl mm_be32
    add x23, x22, 20
    mov x1, 0xFADE0C02
    mov x0, x23
    bl mm_be32
    lea x3, g_cdlen
    ldr x1, [x3, #0]
    add x0, x23, 4
    bl mm_be32
    mov x1, 0x20400
    add x0, x23, 8
    bl mm_be32
    mov x1, 2
    add x0, x23, 12
    bl mm_be32
    lea x3, g_hashoff
    ldr x1, [x3, #0]
    add x0, x23, 16
    bl mm_be32
    mov x1, 88
    add x0, x23, 20
    bl mm_be32
    mov x1, 0
    add x0, x23, 24
    bl mm_be32
    lea x3, g_nslots
    ldr x1, [x3, #0]
    add x0, x23, 28
    bl mm_be32
    ldr x1, [sp, #88]
    ldr x1, [sp, #88]
    mov x1, x21
    add x0, x23, 32
    bl mm_be32
    # +36: hashSize=32 hashType=2 platform=0 pageSizeLog2=14
    mov w9, 32
    strb w9, [x23, #36]
    mov w9, 2
    strb w9, [x23, #37]
    strb wzr, [x23, #38]
    mov w9, 14
    strb w9, [x23, #39]
    # execSeg (cdb+64..87)
    str xzr, [x23, #64]
    lea x3, g_textvmsz
    ldr x3, [x3, #0]
    str x3, [x23, #72]
    mov x9, 1
    str x9, [x23, #80]
    # ident vid cdb+88
    lea x1, g_ident
    lea x3, g_ident_n
    ldr x2, [x3, #0]
    add x0, x23, 88
    bl mm_cpy
    mov w9, 0
    lea x3, g_ident_n
    ldr x3, [x3, #0]
    add x4, x23, 88
    add x4, x4, x3
    strb w9, [x4, #0]
    # hashsidor
    mov x24, 0
mm_hash_l:
    lea x3, g_nslots
    ldr x3, [x3, #0]
    cmp x24, x3
    b.ge mm_done
    lsl x0, x24, 14
    add x0, x0, x19
    mov x1, 16384
    lea x3, g_hashoff
    ldr x3, [x3, #0]
    add x3, x23, x3
    lsl x4, x24, 5
    add x2, x3, x4
    bl sh_sha256
    add x24, x24, 1
    b mm_hash_l
mm_done:
    ldr x3, [sp, #88]
    add x3, x3, x21
    lea x4, g_outlen
    str x3, [x4, #0]
    ldp x19, x20, [sp, #16]
    ldp x21, x22, [sp, #32]
    ldp x23, x24, [sp, #48]
    ldp x25, x26, [sp, #64]
    ldp x27, x28, [sp, #80]
    ldp x29, x30, [sp], #112
    ret

# ----------------------------------------------------------------------------
# _start: las infil, assemblera, skriv utfil
# ----------------------------------------------------------------------------

_start:
    cmp x0, 3
    b.ne st_usage
    mov x19, x1
    ldr x0, [x19, #8]
    bl io_open_ro
    cmp x0, 0
    b.lt st_errin
    mov x20, x0
    # initiera pekare
    lea x3, g_tbufp
    lea x4, g_tbuf
    str x4, [x3, #0]
    lea x3, g_cbufp
    lea x4, g_cbuf
    str x4, [x3, #0]
    lea x3, g_dbufp
    lea x4, g_dbuf
    str x4, [x3, #0]
    # standardident "dunder"
    lea x0, g_ident
    lea x1, c_duder
    mov x2, 6
    bl mm_cpy
    lea x3, g_ident_n
    mov x4, 6
    str x4, [x3, #0]
    # las hela infilen
    mov x21, 0
st_read:
    mov x0, x20
    lea x1, g_srcbuf
    add x1, x1, x21
    mov x2, 1048576
    sub x2, x2, x21
    bl io_read1
    cmp x0, 1
    b.lt st_readdone
    add x21, x21, x0
    b st_read
st_readdone:
    mov x0, x20
    bl io_close
    lea x3, g_srclen
    str x21, [x3, #0]
    # parse rader
    mov x22, 0
    mov x23, 1
st_lp:
    cmp x22, x21
    b.ge st_parsed
    lea x26, g_srcbuf
    mov x24, x22
st_lscan:
    cmp x24, x21
    b.ge st_lfound
    add x25, x26, x24
    ldrb w9, [x25, #0]
    cmp w9, 10
    b.eq st_lfound
    add x24, x24, 1
    b st_lscan
st_lfound:
    add x0, x26, x22
    add x1, x26, x24
    mov x2, x23
    bl ps_parse_line
    add x22, x24, 1
    add x23, x23, 1
    b st_lp
st_parsed:
    bl lb_layout
    bl fx_resolve
    bl mm_build
    # skriv utfil
    ldr x0, [x19, #16]
    bl io_open_out
    cmp x0, 0
    b.lt st_errout
    mov x20, x0
    mov x0, x20
    lea x1, g_imgbuf
    lea x2, g_outlen
    ldr x2, [x2, #0]
    bl io_writen
    mov x0, x20
    bl io_close
    mov x0, 0
    bl io_exit
st_usage:
    mov x0, 2
    lea x1, m_usage
    mov x2, 27
    bl io_writen
    mov x0, 2
    lea x1, m_nl
    mov x2, 1
    bl io_writen
    mov x0, 2
    bl io_exit
st_errin:
    mov x0, 2
    lea x1, m_rdinput
    mov x2, 23
    bl io_writen
    mov x0, 2
    lea x1, m_nl
    mov x2, 1
    bl io_writen
    mov x0, 1
    bl io_exit
st_errout:
    mov x0, 2
    lea x1, m_rdwrite
    mov x2, 22
    bl io_writen
    mov x0, 2
    lea x1, m_nl
    mov x2, 1
    bl io_writen
    mov x0, 1
    bl io_exit

# ============================================================================
# Konstantdata
# ============================================================================

.const

c_duder:
    .str "dunder"
c_pgzero:
    .str "__PAGEZERO"
c_text:
    .str "__TEXT"
c_linkedit:
    .str "__LINKEDIT"
c_data:
    .str "__DATA"
c_textn:
    .str "__text"
c_constn:
    .str "__const"
c_datan:
    .str "__data"
c_bssn:
    .str "__bss"
c_dyldp:
    .strz "/usr/lib/dyld"
s_xzr:
    .str "xzr"
s_wzr:
    .str "wzr"
s_sp:
    .str "sp"
s_b:
    .str "b"
s_bl:
    .str "bl"
s_cbz:
    .str "cbz"
s_cbnz:
    .str "cbnz"
s_tbz:
    .str "tbz"
s_tbnz:
    .str "tbnz"
s_lea:
    .str "lea"
s_mov:
    .str "mov"
s_movz:
    .str "movz"
s_movn:
    .str "movn"
s_movk:
    .str "movk"
s_add:
    .str "add"
s_sub:
    .str "sub"
s_adds:
    .str "adds"
s_subs:
    .str "subs"
s_cmp:
    .str "cmp"
s_cmn:
    .str "cmn"
s_mul:
    .str "mul"
s_smulh:
    .str "smulh"
s_umulh:
    .str "umulh"
s_udiv:
    .str "udiv"
s_sdiv:
    .str "sdiv"
s_and:
    .str "and"
s_orr:
    .str "orr"
s_eor:
    .str "eor"
s_lsl:
    .str "lsl"
s_lsr:
    .str "lsr"
s_asr:
    .str "asr"
s_ror:
    .str "ror"
s_cset:
    .str "cset"
s_csel:
    .str "csel"
s_sxtw:
    .str "sxtw"
s_uxtw:
    .str "uxtw"
s_rev:
    .str "rev"
s_stp:
    .str "stp"
s_ldp:
    .str "ldp"
s_ldr:
    .str "ldr"
s_str:
    .str "str"
s_ldrb:
    .str "ldrb"
s_strb:
    .str "strb"
s_ldrh:
    .str "ldrh"
s_strh:
    .str "strh"
s_ldrsb:
    .str "ldrsb"
s_ldrsw:
    .str "ldrsw"
s_nop:
    .str "nop"
s_svc:
    .str "svc"
s_brk:
    .str "brk"
s_ret:
    .str "ret"
s_br:
    .str "br"
s_blr:
    .str "blr"
c_dtext:
    .str ".text"
c_dconst:
    .str ".const"
c_ddata:
    .str ".data"
c_dbss:
    .str ".bss"
c_dentry:
    .str ".entry"
c_dident:
    .str ".ident"
c_dspace:
    .str ".space"
c_dalign:
    .str ".align"
c_du8:
    .str ".u8"
c_du16:
    .str ".u16"
c_du32:
    .str ".u32"
c_du64:
    .str ".u64"
c_dstr:
    .str ".str"
c_dstrz:
    .str ".strz"
m_pre:
    .strz "seedasm: "
m_rad:
    .strz "rad "
m_kol:
    .strz ": "
m_nl:
    .strz "\n"
m_cit:
    .strz "'"
m_usage:
    .strz "anvandning: seed in.s ut.bin"
m_rdinput:
    .strz "kunde inte lasa infilen"
m_rdwrite:
    .strz "kunde inte skriva utfil"
m_ntok:
    .strz "for manga token"
m_duplab:
    .strz "dubblettetikett"
m_badreg:
    .strz "ogiltigt register"
m_badimm:
    .strz "ogiltig immediate"
m_badcond:
    .strz "okant villkor"
m_leax:
    .strz "lea kraver x-register"
m_escend:
    .strz "avslutad escape"
m_escbad:
    .strz "okand escape"
m_strq:
    .strz "str kraver citattecken"
m_baddir:
    .strz "okant direktiv"
m_mixed:
    .strz "blandade w/x-register"
m_imm12:
    .strz "add/sub-immediate utanfor imm12"
m_immrange:
    .strz "immediate utanfor intervall"
m_shrange:
    .strz "shift utanfor intervall"
m_badmem:
    .strz "ogiltig minnesoperand"
m_postidx:
    .strz "post-index kraver tom klammer"
m_pairimm:
    .strz "stp/ldp kraver immediate-offset"
m_pairalign:
    .strz "stp/ldp-offset ej justerad"
m_pairrange:
    .strz "stp/ldp-offset utanfor intervall"
m_pairaddr:
    .strz "ogiltig stp/ldp-adressering"
m_badoff:
    .strz "ogiltig offset"
m_imm12o:
    .strz "offset utanfor imm12"
m_idxw:
    .strz "indexregister maste vara x"
m_lslel:
    .strz "endast lsl stods for registeroffset"
m_noregoff:
    .strz "registeroffset stods inte"
m_noidx:
    .strz "minnesoperand utan index"
m_badins:
    .strz "okand instruktion"
m_noentry:
    .strz ".entry saknas"
m_noentlab:
    .strz ".entry-etiketten saknas"
m_uklab:
    .strz "okand etikett"
m_brreach:
    .strz "hopp utanfor rackvidd"

# konditioner (c0<<24 | c1<<16)
c_conds:
    .u32 0x65710000, 0x6E650000, 0x63730000, 0x63630000
    .u32 0x6D690000, 0x706C0000, 0x76730000, 0x76630000
    .u32 0x68690000, 0x6C730000, 0x67650000, 0x6C740000
    .u32 0x67740000, 0x6C650000, 0x616C0000, 0x6E760000
    .u32 0x68730000, 0x6C6F0000

# skiftregisterbaser: [mn*2+sf] lsl32 lsl64 lsr32 lsr64 asr32 asr64
c_shbase:
    .u32 0x1AC02000, 0x9AC02000, 0x1AC02400, 0x9AC02400
    .u32 0x1AC02800, 0x9AC02800

# stp/ldp-baser: index = (sf<<3)|(pre<<2)|(post<<1)|mn
c_pairbase:
    .u32 0x29000000, 0x29400000, 0x29800000, 0x29C00000
    .u32 0x28800000, 0x28C00000, 0, 0
    .u32 0xA9000000, 0xA9400000, 0xA9800000, 0xA9C00000
    .u32 0xA8800000, 0xA8C00000, 0, 0

# ldr/str-immediatebaser: index = mnid*2+wform
# ordning: ldr ldr str str ldrb ldrb strh?? nej: mnid 0..7
# 0=ldr 1=str 2=ldrb 3=strb 4=ldrh 5=strh 6=ldrsb 7=ldrsw
c_ldbase:
    .u32 0xB9400000, 0xF9400000, 0xB9000000, 0xF9000000
    .u32 0x39400000, 0x39400000, 0x39000000, 0x39000000
    .u32 0x79400000, 0x79400000, 0x79000000, 0x79000000
    .u32 0x39C00000, 0x39800000, 0xB9800000, 0xB9800000

# registeroffsetbaser: ldr_x ldr_w str_x str_w ldrb strb
c_ldroff:
    .u32 0xF8606800, 0xB8606800, 0xF8206800, 0xB8206800
    .u32 0x38606800, 0x38206800

# ldr-familjens storlekar: (immshift<<4)|size
c_ldsize:
    .u32 0x22, 0x22, 0x00, 0x00, 0x11, 0x11, 0x00, 0x22

# SHA-256 K
sh_k:
    .u32 0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5
    .u32 0x395c6ba5, 0x59f111f1, 0x923f82a4, 0xab1c5ed5
    .u32 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3
    .u32 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174
    .u32 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc
    .u32 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da
    .u32 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7
    .u32 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967
    .u32 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13
    .u32 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85
    .u32 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3
    .u32 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070
    .u32 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5
    .u32 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3
    .u32 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208
    .u32 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2

# SHA-256 init
sh_h:
    .u32 0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a
    .u32 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19

# ============================================================================
# Minne
# ============================================================================

.bss
srcbuf:
g_srcbuf:
    .space 1048576
imgbuf:
g_imgbuf:
    .space 2097152
tbuf:
g_tbuf:
    .space 524288
cbuf:
g_cbuf:
    .space 65536
dbuf:
g_dbuf:
    .space 65536
g_toks:
    .space 1024
g_labs:
labs:
    .space 393216
g_fixs:
fixs:
    .space 458752
g_W:
    .space 256
g_H:
    .space 32
g_scratch:
    .space 4096
g_scratch2:
    .space 4096
g_ident:
    .space 256
g_zbuf:
    .space 64
g_ntok:
    .space 8
g_nlab:
    .space 8
g_nfix:
    .space 8
g_tbufp:
    .space 8
g_cbufp:
    .space 8
g_dbufp:
    .space 8
g_bsssz:
    .space 8
g_cursec:
    .space 8
g_ln:
    .space 8
g_entry_p:
    .space 8
g_entry_n:
    .space 8
g_ident_n:
    .space 8
g_srclen:
    .space 8
g_outlen:
    .space 8
g_text_sz:
    .space 8
g_csz:
    .space 8
g_dsz:
    .space 8
g_bs_sz:
    .space 8
g_has_c:
    .space 8
g_has_d:
    .space 8
g_has_b:
    .space 8
g_dseg:
    .space 8
g_nsects:
    .space 8
g_cmdsz:
    .space 8
g_text_off:
    .space 8
g_const_off:
    .space 8
g_cend:
    .space 8
g_data_off:
    .space 8
g_bsoff:
    .space 8
g_sig_off:
    .space 8
g_nslots:
    .space 8
g_hashoff:
    .space 8
g_cdlen:
    .space 8
g_datasize:
    .space 8
g_entryoff:
    .space 8
g_textvmsz:
    .space 8
g_nul1:
    .space 1
