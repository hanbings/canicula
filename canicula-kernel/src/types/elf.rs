pub type Elf32Addr = u32;
pub type Elf32Half = u16;
pub type Elf32Off = u32;
pub type Elf32Word = u32;
pub type Elf32Sword = u32;
pub type UnsignedChar = u8;

pub struct ELFHeader {
    e_ident: [UnsignedChar; 16],
    e_type: Elf32Half,
    e_machine: Elf32Half,
    e_version: Elf32Sword,
    e_entry: Elf32Addr,
    e_phoff: Elf32Off,
    e_shoff: Elf32Off,
    e_flags: Elf32Word,
    e_ehsize: Elf32Half,
    e_phentsize: Elf32Half,
    e_phnum: Elf32Half,
    e_shentsize: Elf32Half,
    e_shnum: Elf32Half,
    e_shstrndx: Elf32Half,
}

pub struct ELFProgramHeaderTable {
    p_type: Elf32Sword,
    p_offset: Elf32Off,
    p_vaddr: Elf32Addr,
    p_paddr: Elf32Addr,
    p_filesz: Elf32Sword,
    p_memsz: Elf32Sword,
    p_flags: Elf32Sword,
    p_align: Elf32Sword,
}

pub struct ELFSectionHeaderTable {
    sh_name: Elf32Sword,
    sh_type: Elf32Sword,
    sh_flags: Elf32Sword,
    sh_addr: Elf32Addr,
    sh_offset: Elf32Off,
    sh_size: Elf32Sword,
    sh_link: Elf32Sword,
    sh_info: Elf32Sword,
    sh_addralign: Elf32Sword,
    sh_entsize: Elf32Sword,
}
