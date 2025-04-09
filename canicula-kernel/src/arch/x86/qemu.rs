pub fn shutdown(exit_code: u32) {
    use x86_64::instructions::port::Port;

    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code);
    }
}
