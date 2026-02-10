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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuperBlock {
    InodesCount,
    BlocksCountLo,
    RBlocksCountLo,
    FreeBlocksCountLo,
    FreeInodesCount,
    FirstDataBlock,
    LogBlockSize,
    LogClusterSize,
    BlocksPerGroup,
    ClustersPerGroup,
    InodesPerGroup,
    Mtime,
    Wtime,
    MntCount,
    MaxMntCount,
    Magic,
    State,
    Errors,
    MinorRevLevel,
    LastCheck,
    CheckInterval,
    CreatorOs,
    RevLevel,
    DefResuid,
    DefResgid,
    FirstIno,
    InodeSize,
    BlockGroupNr,
    FeatureCompat,
    FeatureIncompat,
    FeatureRoCompat,
    Uuid,
    VolumeName,
    LastMounted,
    AlgorithmUsageBitmap,
    PreallocBlocks,
    PreallocDirBlocks,
    ReservedGdtBlocks,
    JournalUuid,
    JournalInum,
    JournalDev,
    LastOrphan,
    HashSeed,
    DefHashVersion,
    JnlBackupType,
    DescSize,
    DefaultMountOpts,
    FirstMetaBg,
    MkfsTime,
    JnlBlocks,
    BlocksCountHi,
    RBlocksCountHi,
    FreeBlocksCountHi,
    MinExtraIsize,
    WantExtraIsize,
    Flags,
    RaidStride,
    MMPInterval,
    MMPBlock,
    RaidStripeWidth,
    LogGroupsPerFlex,
    ChecksumType,
    ReservedPad,
    KbytesWritten,
    SnapshotInum,
    SnapshotId,
    SnapshotRBlocksCount,
    SnapshotList,
    ErrorCount,
    FirstErrorTime,
    FirstErrorIno,
    FirstErrorBlock,
    FirstErrorFunc,
    FirstErrorLine,
    LastErrorTime,
    LastErrorIno,
    LastErrorLine,
    LastErrorBlock,
    LastErrorFunc,
    MountOpts,
    UsrQuotaInum,
    GrpQuotaInum,
    OverheadBlocks,
    BackupBgs,
    EncryptAlgos,
    EncryptPwSalt,
    LpfIno,
    PrjQuotaInum,
    ChecksumSeed,
    Reserved,
    Checksum,
}

impl SuperBlock {
    pub fn slice(&self) -> SuperBlockSlice {
        match self {
            SuperBlock::InodesCount => SuperBlockSlice { offset: 0, size: 4 },
            SuperBlock::BlocksCountLo => SuperBlockSlice { offset: 4, size: 4 },
            SuperBlock::RBlocksCountLo => SuperBlockSlice { offset: 8, size: 4 },
            SuperBlock::FreeBlocksCountLo => SuperBlockSlice {
                offset: 12,
                size: 4,
            },
            SuperBlock::FreeInodesCount => SuperBlockSlice {
                offset: 16,
                size: 4,
            },
            SuperBlock::FirstDataBlock => SuperBlockSlice {
                offset: 20,
                size: 4,
            },
            SuperBlock::LogBlockSize => SuperBlockSlice {
                offset: 24,
                size: 4,
            },
            SuperBlock::LogClusterSize => SuperBlockSlice {
                offset: 28,
                size: 4,
            },
            SuperBlock::BlocksPerGroup => SuperBlockSlice {
                offset: 32,
                size: 4,
            },
            SuperBlock::ClustersPerGroup => SuperBlockSlice {
                offset: 36,
                size: 4,
            },
            SuperBlock::InodesPerGroup => SuperBlockSlice {
                offset: 40,
                size: 4,
            },
            SuperBlock::Mtime => SuperBlockSlice {
                offset: 44,
                size: 4,
            },
            SuperBlock::Wtime => SuperBlockSlice {
                offset: 48,
                size: 4,
            },
            SuperBlock::MntCount => SuperBlockSlice {
                offset: 52,
                size: 2,
            },
            SuperBlock::MaxMntCount => SuperBlockSlice {
                offset: 54,
                size: 2,
            },
            SuperBlock::Magic => SuperBlockSlice {
                offset: 56,
                size: 2,
            },
            SuperBlock::State => SuperBlockSlice {
                offset: 58,
                size: 2,
            },
            SuperBlock::Errors => SuperBlockSlice {
                offset: 60,
                size: 2,
            },
            SuperBlock::MinorRevLevel => SuperBlockSlice {
                offset: 62,
                size: 2,
            },
            SuperBlock::LastCheck => SuperBlockSlice {
                offset: 64,
                size: 4,
            },
            SuperBlock::CheckInterval => SuperBlockSlice {
                offset: 68,
                size: 4,
            },
            SuperBlock::CreatorOs => SuperBlockSlice {
                offset: 72,
                size: 4,
            },
            SuperBlock::RevLevel => SuperBlockSlice {
                offset: 76,
                size: 4,
            },
            SuperBlock::DefResuid => SuperBlockSlice {
                offset: 80,
                size: 2,
            },
            SuperBlock::DefResgid => SuperBlockSlice {
                offset: 82,
                size: 2,
            },
            SuperBlock::FirstIno => SuperBlockSlice {
                offset: 84,
                size: 4,
            },
            SuperBlock::InodeSize => SuperBlockSlice {
                offset: 88,
                size: 2,
            },
            SuperBlock::BlockGroupNr => SuperBlockSlice {
                offset: 90,
                size: 2,
            },
            SuperBlock::FeatureCompat => SuperBlockSlice {
                offset: 92,
                size: 4,
            },
            SuperBlock::FeatureIncompat => SuperBlockSlice {
                offset: 96,
                size: 4,
            },
            SuperBlock::FeatureRoCompat => SuperBlockSlice {
                offset: 100,
                size: 4,
            },
            SuperBlock::Uuid => SuperBlockSlice {
                offset: 102,
                size: 16,
            },
            SuperBlock::VolumeName => SuperBlockSlice {
                offset: 118,
                size: 16,
            },
            SuperBlock::LastMounted => SuperBlockSlice {
                offset: 134,
                size: 4,
            },
            SuperBlock::AlgorithmUsageBitmap => SuperBlockSlice {
                offset: 138,
                size: 4,
            },
            SuperBlock::PreallocBlocks => SuperBlockSlice {
                offset: 142,
                size: 4,
            },
            SuperBlock::PreallocDirBlocks => SuperBlockSlice {
                offset: 146,
                size: 4,
            },
            SuperBlock::ReservedGdtBlocks => SuperBlockSlice {
                offset: 150,
                size: 16,
            },
            SuperBlock::JournalUuid => SuperBlockSlice {
                offset: 166,
                size: 16,
            },
            SuperBlock::JournalInum => SuperBlockSlice {
                offset: 182,
                size: 4,
            },
            SuperBlock::JournalDev => SuperBlockSlice {
                offset: 186,
                size: 4,
            },
            SuperBlock::LastOrphan => SuperBlockSlice {
                offset: 190,
                size: 4,
            },
            SuperBlock::HashSeed => SuperBlockSlice {
                offset: 194,
                size: 8,
            },
            SuperBlock::DefHashVersion => SuperBlockSlice {
                offset: 202,
                size: 4,
            },
            SuperBlock::JnlBackupType => SuperBlockSlice {
                offset: 206,
                size: 4,
            },
            SuperBlock::DescSize => SuperBlockSlice {
                offset: 210,
                size: 4,
            },
            SuperBlock::DefaultMountOpts => SuperBlockSlice {
                offset: 214,
                size: 4,
            },
            SuperBlock::FirstMetaBg => SuperBlockSlice {
                offset: 218,
                size: 4,
            },
            SuperBlock::MkfsTime => SuperBlockSlice {
                offset: 222,
                size: 4,
            },
            SuperBlock::JnlBlocks => SuperBlockSlice {
                offset: 226,
                size: 12,
            },
            SuperBlock::RBlocksCountHi => SuperBlockSlice {
                offset: 238,
                size: 4,
            },
            SuperBlock::FreeBlocksCountHi => SuperBlockSlice {
                offset: 242,
                size: 4,
            },
            SuperBlock::MinExtraIsize => SuperBlockSlice {
                offset: 246,
                size: 4,
            },
            SuperBlock::WantExtraIsize => SuperBlockSlice {
                offset: 250,
                size: 4,
            },
            SuperBlock::Flags => SuperBlockSlice {
                offset: 254,
                size: 4,
            },
            SuperBlock::RaidStride => SuperBlockSlice {
                offset: 258,
                size: 4,
            },
            SuperBlock::MMPInterval => SuperBlockSlice {
                offset: 262,
                size: 4,
            },
            SuperBlock::MMPBlock => SuperBlockSlice {
                offset: 266,
                size: 4,
            },
            SuperBlock::RaidStripeWidth => SuperBlockSlice {
                offset: 270,
                size: 4,
            },
            SuperBlock::LogGroupsPerFlex => SuperBlockSlice {
                offset: 274,
                size: 4,
            },
            SuperBlock::ChecksumType => SuperBlockSlice {
                offset: 278,
                size: 4,
            },
            SuperBlock::ReservedPad => SuperBlockSlice {
                offset: 282,
                size: 6,
            },
            SuperBlock::KbytesWritten => SuperBlockSlice {
                offset: 288,
                size: 8,
            },
            SuperBlock::SnapshotInum => SuperBlockSlice {
                offset: 296,
                size: 4,
            },
            SuperBlock::SnapshotId => SuperBlockSlice {
                offset: 300,
                size: 4,
            },
            SuperBlock::SnapshotRBlocksCount => SuperBlockSlice {
                offset: 304,
                size: 8,
            },
            SuperBlock::SnapshotList => SuperBlockSlice {
                offset: 312,
                size: 8,
            },
            SuperBlock::ErrorCount => SuperBlockSlice {
                offset: 320,
                size: 4,
            },
            SuperBlock::FirstErrorTime => SuperBlockSlice {
                offset: 324,
                size: 4,
            },
            SuperBlock::FirstErrorIno => SuperBlockSlice {
                offset: 328,
                size: 4,
            },
            SuperBlock::FirstErrorBlock => SuperBlockSlice {
                offset: 332,
                size: 4,
            },
            SuperBlock::FirstErrorFunc => SuperBlockSlice {
                offset: 336,
                size: 4,
            },
            SuperBlock::FirstErrorLine => SuperBlockSlice {
                offset: 340,
                size: 4,
            },
            SuperBlock::LastErrorTime => SuperBlockSlice {
                offset: 344,
                size: 8,
            },
            SuperBlock::LastErrorIno => SuperBlockSlice {
                offset: 352,
                size: 4,
            },
            SuperBlock::LastErrorLine => SuperBlockSlice {
                offset: 364,
                size: 4,
            },
            SuperBlock::LastErrorBlock => SuperBlockSlice {
                offset: 356,
                size: 4,
            },
            SuperBlock::LastErrorFunc => SuperBlockSlice {
                offset: 360,
                size: 4,
            },
            SuperBlock::MountOpts => SuperBlockSlice {
                offset: 368,
                size: 64,
            },
            SuperBlock::UsrQuotaInum => SuperBlockSlice {
                offset: 432,
                size: 4,
            },
            SuperBlock::GrpQuotaInum => SuperBlockSlice {
                offset: 436,
                size: 4,
            },
            SuperBlock::OverheadBlocks => SuperBlockSlice {
                offset: 440,
                size: 4,
            },
            SuperBlock::BackupBgs => SuperBlockSlice {
                offset: 444,
                size: 6,
            },
            SuperBlock::EncryptAlgos => SuperBlockSlice {
                offset: 450,
                size: 4,
            },
            SuperBlock::EncryptPwSalt => SuperBlockSlice {
                offset: 454,
                size: 16,
            },
            SuperBlock::LpfIno => SuperBlockSlice {
                offset: 470,
                size: 4,
            },
            SuperBlock::PrjQuotaInum => SuperBlockSlice {
                offset: 474,
                size: 4,
            },
            SuperBlock::ChecksumSeed => SuperBlockSlice {
                offset: 478,
                size: 8,
            },
            SuperBlock::Reserved => SuperBlockSlice {
                offset: 486,
                size: 2,
            },
            SuperBlock::Checksum => SuperBlockSlice {
                offset: 488,
                size: 4,
            },
            _ => SuperBlockSlice { offset: 0, size: 0 },
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuperBlockSlice {
    pub offset: usize,
    pub size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuperBlockSnapshot {
    pub s_inodes_count: u32,
    pub s_blocks_count_lo: u32,
    pub s_r_blocks_count_lo: u32,
    pub s_free_blocks_count_lo: u32,
    pub s_free_inodes_count: u32,
    pub s_first_data_block: u32,
    pub s_log_block_size: u32,
    pub s_log_cluster_size: u32,
    pub s_blocks_per_group: u32,
    pub s_clusters_per_group: u32,
    pub s_inodes_per_group: u32,
    pub s_mtime: u32,
    pub s_wtime: u32,
    pub s_mnt_count: u16,
    pub s_max_mnt_count: u16,
    pub s_magic: u16,
    pub s_state: u16,
    pub s_errors: u16,
    pub s_minor_rev_level: u16,
    pub s_lastcheck: u32,
    pub s_checkinterval: u32,
    pub s_creator_os: u32,
    pub s_rev_level: u32,
    pub s_def_resuid: u16,
    pub s_def_resgid: u16,
    pub s_first_ino: u32,
    pub s_inode_size: u16,
    pub s_block_group_nr: u16,
    pub s_feature_compat: u32,
    pub s_feature_incompat: u32,
    pub s_feature_ro_compat: u32,
    pub s_uuid: [u8; 16],
    pub s_volume_name: [char; 16],
    pub s_last_mounted: [char; 64],
    pub s_algorithm_usage_bitmap: u32,
    pub s_prealloc_blocks: u8,
    pub s_prealloc_dir_blocks: u8,
    pub s_reserved_gdt_blocks: u16,
    pub s_journal_uuid: [u8; 16],
    pub s_journal_inum: u32,
    pub s_journal_dev: u32,
    pub s_last_orphan: u32,
    pub s_hash_seed: [u32; 4],
    pub s_def_hash_version: u8,
    pub s_jnl_backup_type: u8,
    pub s_desc_size: u16,
    pub s_default_mount_opts: u32,
    pub s_first_meta_bg: u32,
    pub s_mkfs_time: u32,
    pub s_jnl_blocks: [u32; 17],
    pub s_blocks_count_hi: u32,
    pub s_r_blocks_count_hi: u32,
    pub s_free_blocks_count_hi: u32,
    pub s_min_extra_isize: u16,
    pub s_want_extra_isize: u16,
    pub s_flags: u32,
    pub s_raid_stride: u16,
    pub s_mmp_interval: u16,
    pub s_mmp_block: u64,
    pub s_raid_stripe_width: u32,
    pub s_log_groups_per_flex: u8,
    pub s_checksum_type: u8,
    pub s_reserved_pad: u16,
    pub s_kbytes_written: u64,
    pub s_snapshot_inum: u32,
    pub s_snapshot_id: u32,
    pub s_snapshot_r_blocks_count: u64,
    pub s_snapshot_list: u32,
    pub s_error_count: u32,
    pub s_first_error_time: u32,
    pub s_first_error_ino: u32,
    pub s_first_error_block: u64,
    pub s_first_error_func: [u8; 32],
    pub s_first_error_line: u32,
    pub s_last_error_time: u32,
    pub s_last_error_ino: u32,
    pub s_last_error_line: u32,
    pub s_last_error_block: u64,
    pub s_last_error_func: [u8; 32],
    pub s_mount_opts: [u8; 64],
    pub s_usr_quota_inum: u32,
    pub s_grp_quota_inum: u32,
    pub s_overhead_blocks: u32,
    pub s_backup_bgs: [u32; 2],
    pub s_encrypt_algos: [u8; 4],
    pub s_encrypt_pw_salt: [u8; 16],
    pub s_lpf_ino: u32,
    pub s_prj_quota_inum: u32,
    pub s_checksum_seed: u32,
    pub s_reserved: [u32; 98],
    pub s_checksum: u32,
}
