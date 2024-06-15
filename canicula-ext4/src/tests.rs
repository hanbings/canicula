mod test {
    #[test]
    fn test() {
        use crate::Ext4FS;
        use canicula_common::fs::OperateError;

        let read_byte = |_offset: usize| -> Result<u8, OperateError> {
            // Implement your read_byte function here
            Ok(0)
        };

        let write_byte = |_byte: u8, _offset: usize| -> Result<usize, OperateError> {
            // Implement your write_byte function here
            Ok(1)
        };

        let _fs: Ext4FS<1024> = Ext4FS::new(read_byte, write_byte);
    }
}
