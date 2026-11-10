# Etapp 1-prov: funktionsanrop med argument och returvärde.
# main anropar add_två via bl; add_två lägger till 2 och returnerar.
.ident "dunder-etapp1-call"
.entry main
.text
main:
    stp x29, x30, [sp, #-16]!
    mov x0, 40
    bl add_tva
    ldp x29, x30, [sp], #16
    ret
add_tva:
    add x0, x0, 2
    ret
