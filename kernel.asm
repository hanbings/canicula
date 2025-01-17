BITS 64

mov rax, 0xff408deb

display:
    mov [rdi], rax
    add rdi, 4
    loop display

jmp $