pub enum ELFDataType {
    Elf32Addr(u32),
    Elf32Half(u16),
    Elf32Off(u32),
    Elf32Sword(u32),
    Elf32Word(u32),
    UnsignedChar(u8),
}

pub struct ELFHeader {
    e_ident: [ELFDataType::UnsignedChar; 16],
    e_type: ELFDataType::Elf32Half,
    e_machine: ELFDataType::Elf32Half,
    e_version: ELFDataType::Elf32Sword,
    e_entry: ELFDataType::Elf32Addr,
    e_phoff: ELFDataType::Elf32Off,
    e_shoff: ELFDataType::Elf32Off,
    e_flags: ELFDataType::Elf32Word,
    e_ehsize: ELFDataType::Elf32Half,
    e_phentsize: ELFDataType::Elf32Half,
    e_phnum: ELFDataType::Elf32Half,
    e_shentsize: ELFDataType::Elf32Half,
    e_shnum: ELFDataType::Elf32Half,
    e_shstrndx: ELFDataType::Elf32Half,
}

pub struct ELFProgramHeaderTable {
    p_type: ELFDataType::Elf32Sword,
    p_offset: ELFDataType::Elf32Off,
    p_vaddr: ELFDataType::Elf32Addr,
    p_paddr: ELFDataType::Elf32Addr,
    p_filesz: ELFDataType::Elf32Sword,
    p_memsz: ELFDataType::Elf32Sword,
    p_flags: ELFDataType::Elf32Sword,
    p_align: ELFDataType::Elf32Sword,
}

pub struct ELFSectionHeaderTable {
    sh_name: ELFDataType::Elf32Sword,
    sh_type: ELFDataType::Elf32Sword,
    sh_flags: ELFDataType::Elf32Sword,
    sh_addr: ELFDataType::Elf32Addr,
    sh_offset: ELFDataType::Elf32Off,
    sh_size: ELFDataType::Elf32Sword,
    sh_link: ELFDataType::Elf32Sword,
    sh_info: ELFDataType::Elf32Sword,
    sh_addralign: ELFDataType::Elf32Sword,
    sh_entsize: ELFDataType::Elf32Sword,
}
