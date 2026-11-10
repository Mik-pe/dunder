# Etapp 0: minimal bild som returnerar en vald exitkod.
# Instruktioner: mov x0,#42 (0xd2800540) ; ret (0xd65f03c0)
.ident "dunder-etapp0"
.entry main
.text
main:
    mov x0, 42
    ret
