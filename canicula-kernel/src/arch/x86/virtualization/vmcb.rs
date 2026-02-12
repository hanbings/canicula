#![allow(dead_code)]

#[repr(C, align(4096))]
pub struct Vmcb {
    bytes: [u8; 4096],
}

pub const VMCB_CONTROL_BASE: usize = 0x000;
pub const VMCB_SAVE_BASE: usize = 0x400;

pub mod control {
    pub const INTERCEPTS: usize = 0x000;
    pub const IOPM_BASE_PA: usize = 0x040;
    pub const MSRPM_BASE_PA: usize = 0x048;
    pub const ASID: usize = 0x058;
    pub const TLB_CTL: usize = 0x05c;
    pub const INT_CTL: usize = 0x060;
    pub const INT_VECTOR: usize = 0x064;
    pub const INT_STATE: usize = 0x068;
    pub const EXIT_CODE: usize = 0x070;
    pub const EXIT_CODE_HI: usize = 0x074;
    pub const EXIT_INFO_1: usize = 0x078;
    pub const EXIT_INFO_2: usize = 0x080;
    pub const NESTED_CTL: usize = 0x090;
    pub const CLEAN: usize = 0x0c0;
    pub const NEXT_RIP: usize = 0x0c8;
    pub const INSN_LEN: usize = 0x0d0;
}

pub mod save {
    pub const ES: usize = 0x000;
    pub const CS: usize = 0x010;
    pub const SS: usize = 0x020;
    pub const DS: usize = 0x030;
    pub const FS: usize = 0x040;
    pub const GS: usize = 0x050;
    pub const GDTR: usize = 0x060;
    pub const LDTR: usize = 0x070;
    pub const IDTR: usize = 0x080;
    pub const TR: usize = 0x090;

    pub const CPL: usize = 0x0cb;
    pub const EFER: usize = 0x0d0;
    pub const CR4: usize = 0x148;
    pub const CR3: usize = 0x150;
    pub const CR0: usize = 0x158;
    pub const DR7: usize = 0x160;
    pub const DR6: usize = 0x168;
    pub const RFLAGS: usize = 0x170;
    pub const RIP: usize = 0x178;
    pub const RSP: usize = 0x1d8;
    pub const RAX: usize = 0x1f8;
}

pub mod intercept {
    pub const HLT: u32 = 79;
    pub const VMRUN: u32 = 128;
    pub const VMMCALL: u32 = 129;
}

pub mod nested_ctl {
    pub const NP_ENABLE: u64 = 1 << 0;
}

pub mod tlb_ctl {
    pub const DO_NOTHING: u8 = 0;
    pub const FLUSH_ALL_ASID: u8 = 1;
    pub const FLUSH_ASID: u8 = 3;
    pub const FLUSH_ASID_LOCAL: u8 = 7;
}

pub mod exit_code {
    pub const HLT: u32 = 0x078;
    pub const VMMCALL: u32 = 0x081;
    pub const NPF: u32 = 0x400;
    pub const ERR: u32 = 0xFFFF_FFFF;
}

impl Vmcb {
    pub const SIZE: usize = 4096;

    pub fn zeroed() -> Self {
        Self { bytes: [0; 4096] }
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.bytes.as_ptr()
    }

    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.bytes.as_mut_ptr()
    }

    pub unsafe fn write_u64(&mut self, offset: usize, value: u64) {
        unsafe {
            let dst = self.as_mut_ptr().add(offset) as *mut u64;
            dst.write_unaligned(value.to_le());
        }
    }

    pub unsafe fn write_u32(&mut self, offset: usize, value: u32) {
        unsafe {
            let dst = self.as_mut_ptr().add(offset) as *mut u32;
            dst.write_unaligned(value.to_le());
        }
    }

    pub unsafe fn write_u16(&mut self, offset: usize, value: u16) {
        unsafe {
            let dst = self.as_mut_ptr().add(offset) as *mut u16;
            dst.write_unaligned(value.to_le());
        }
    }

    pub unsafe fn write_u8(&mut self, offset: usize, value: u8) {
        unsafe {
            let dst = self.as_mut_ptr().add(offset);
            dst.write(value);
        }
    }

    pub unsafe fn read_u64(&self, offset: usize) -> u64 {
        unsafe {
            let src = self.as_ptr().add(offset) as *const u64;
            u64::from_le(src.read_unaligned())
        }
    }

    pub unsafe fn read_u32(&self, offset: usize) -> u32 {
        unsafe {
            let src = self.as_ptr().add(offset) as *const u32;
            u32::from_le(src.read_unaligned())
        }
    }

    pub fn set_intercept(&mut self, bit: u32) {
        let word = (bit / 32) as usize;
        let shift = bit % 32;
        let offset = VMCB_CONTROL_BASE + control::INTERCEPTS + word * 4;
        unsafe {
            let old = self.read_u32(offset);
            self.write_u32(offset, old | (1u32 << shift));
        }
    }

    pub fn write_save_seg(
        &mut self,
        seg_off: usize,
        selector: u16,
        attrib: u16,
        limit: u32,
        base: u64,
    ) {
        let base_off = VMCB_SAVE_BASE + seg_off;
        unsafe {
            self.write_u16(base_off + 0, selector);
            self.write_u16(base_off + 2, attrib);
            self.write_u32(base_off + 4, limit);
            self.write_u64(base_off + 8, base);
        }
    }
}
