#![no_main]
#![no_std]

use log::*;
use wasmi::*;
use uefi::prelude::*;

#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();
    info!("Hello world!");

    let wasm = r#"
        (module
            (import "host" "hello" (func $host_hello (param i32)))
            (func (export "hello")
                (call $host_hello (i32.const 3))
            )
        )
    "#;

    let engine = Engine::default();
    let module = Module::new(&engine, wasm).unwrap();

    type HostState = u32;
    let mut store = Store::new(&engine, 42);

    let mut linker = <Linker<HostState>>::new(&engine);
    let _ = linker.func_wrap("host", "hello", |caller: Caller<'_, HostState>, param: i32| {
        info!("Got {param} from WebAssembly and my host state is: {}", caller.data());
    });

    let instance = linker
        .instantiate(&mut store, &module)
        .unwrap()
        .start(&mut store)
        .unwrap();

    instance
        .get_typed_func::<(), ()>(&store, "hello")
        .unwrap()
        .call(&mut store, ())
        .unwrap();

    boot::stall(10_000_000);
    Status::SUCCESS
}