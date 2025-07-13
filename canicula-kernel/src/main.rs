#![no_main]
#![no_std]

use log::*;
use uefi::prelude::*;
use wasmi::{Caller, Engine, Linker, Module, Store};

#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();
    info!("Hello world!");

    // let wasm = include_bytes!("/home/hanbings/github/canicula/target/wasm32-unknown-unknown/release/hello_wasm.wasm");
    let wasm = include_bytes!("/home/hanbings/github/canicula/hello-moonbit/target/wasm/release/build/lib/lib.wasm");

    let engine = Engine::default();
    let module = Module::new(&engine, wasm).unwrap();

    type HostState = u32;
    let mut store = Store::new(&engine, 42);

    let mut linker = <Linker<HostState>>::new(&engine);
    let _ = linker.func_wrap("host", "hello", |caller: Caller<'_, HostState>, param: i32| -> i32 {
        info!("Got {param} from Moonbit WebAssembly and my host state is: {}", caller.data());
        0
    });

    let instance = linker
        .instantiate(&mut store, &module)
        .unwrap()
        .start(&mut store)
        .unwrap();

    instance
        .get_typed_func::<(), ()>(&store, "call_host")
        .unwrap()
        .call(&mut store, ())
        .unwrap();

    boot::stall(10_000_000);
    Status::SUCCESS
}