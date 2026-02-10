#![allow(dead_code)]
pub const EXT4_SUPER_BLOCK_OFFSET: usize = 1024;
pub const EXT4_SUPER_BLOCK_MAGIC: u16 = 0xEF53;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuperBlockHeader {
    pub inodes_count: u32,
    pub blocks_count_lo: u32,
    pub free_blocks_count_lo: u32,
    pub free_inodes_count: u32,
    pub log_block_size: u32,
    pub blocks_per_group: u32,
    pub inodes_per_group: u32,
    pub magic: u16,
    pub inode_size: u16,
    pub feature_incompat: u32,
    pub feature_ro_compat: u32,
}

impl SuperBlockHeader {
    pub fn block_size(&self) -> usize {
        1024usize << self.log_block_size
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuperBlockSlice {
    pub offset: usize,
    pub size: usize,
}

macro_rules! define_super_block_fields {
    ($( $name:ident => ($offset:expr, $size:expr), )+) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum SuperBlock {
            $( $name, )+
        }

        impl SuperBlock {
            pub fn slice(&self) -> SuperBlockSlice {
                match self {
                    $( Self::$name => SuperBlockSlice { offset: $offset, size: $size }, )+
                }
            }

            #[allow(dead_code)]
            pub fn offset(&self) -> usize {
                self.slice().offset
            }

            pub fn absolute_offset(&self) -> usize {
                EXT4_SUPER_BLOCK_OFFSET + self.offset()
            }

            #[allow(dead_code)]
            pub fn size(&self) -> usize {
                self.slice().size
            }
        }
    };
}

define_super_block_fields! {
    InodesCount => (0, 4),
    BlocksCountLo => (4, 4),
    RBlocksCountLo => (8, 4),
    FreeBlocksCountLo => (12, 4),
    FreeInodesCount => (16, 4),
    FirstDataBlock => (20, 4),
    LogBlockSize => (24, 4),
    LogClusterSize => (28, 4),
    BlocksPerGroup => (32, 4),
    ClustersPerGroup => (36, 4),
    InodesPerGroup => (40, 4),
    Mtime => (44, 4),
    Wtime => (48, 4),
    MntCount => (52, 2),
    MaxMntCount => (54, 2),
    Magic => (56, 2),
    State => (58, 2),
    Errors => (60, 2),
    MinorRevLevel => (62, 2),
    LastCheck => (64, 4),
    CheckInterval => (68, 4),
    CreatorOs => (72, 4),
    RevLevel => (76, 4),
    DefResuid => (80, 2),
    DefResgid => (82, 2),
    FirstIno => (84, 4),
    InodeSize => (88, 2),
    BlockGroupNr => (90, 2),
    FeatureCompat => (92, 4),
    FeatureIncompat => (96, 4),
    FeatureRoCompat => (100, 4),
    Uuid => (102, 16),
    VolumeName => (118, 16),
    LastMounted => (134, 4),
    AlgorithmUsageBitmap => (138, 4),
    PreallocBlocks => (142, 4),
    PreallocDirBlocks => (146, 4),
    ReservedGdtBlocks => (150, 16),
    JournalUuid => (166, 16),
    JournalInum => (182, 4),
    JournalDev => (186, 4),
    LastOrphan => (190, 4),
    HashSeed => (194, 8),
    DefHashVersion => (202, 4),
    JnlBackupType => (206, 4),
    DescSize => (210, 4),
    DefaultMountOpts => (214, 4),
    FirstMetaBg => (218, 4),
    MkfsTime => (222, 4),
    JnlBlocks => (226, 12),
    BlocksCountHi => (0, 0),
    RBlocksCountHi => (238, 4),
    FreeBlocksCountHi => (242, 4),
    MinExtraIsize => (246, 4),
    WantExtraIsize => (250, 4),
    Flags => (254, 4),
    RaidStride => (258, 4),
    MMPInterval => (262, 4),
    MMPBlock => (266, 4),
    RaidStripeWidth => (270, 4),
    LogGroupsPerFlex => (274, 4),
    ChecksumType => (278, 4),
    ReservedPad => (282, 6),
    KbytesWritten => (288, 8),
    SnapshotInum => (296, 4),
    SnapshotId => (300, 4),
    SnapshotRBlocksCount => (304, 8),
    SnapshotList => (312, 8),
    ErrorCount => (320, 4),
    FirstErrorTime => (324, 4),
    FirstErrorIno => (328, 4),
    FirstErrorBlock => (332, 4),
    FirstErrorFunc => (336, 4),
    FirstErrorLine => (340, 4),
    LastErrorTime => (344, 8),
    LastErrorIno => (352, 4),
    LastErrorLine => (364, 4),
    LastErrorBlock => (356, 4),
    LastErrorFunc => (360, 4),
    MountOpts => (368, 64),
    UsrQuotaInum => (432, 4),
    GrpQuotaInum => (436, 4),
    OverheadBlocks => (440, 4),
    BackupBgs => (444, 6),
    EncryptAlgos => (450, 4),
    EncryptPwSalt => (454, 16),
    LpfIno => (470, 4),
    PrjQuotaInum => (474, 4),
    ChecksumSeed => (478, 8),
    Reserved => (486, 2),
    Checksum => (488, 4),
}
