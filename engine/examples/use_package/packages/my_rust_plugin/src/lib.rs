use wolflang::{WolfEngine, tokens::Token};

// Safe Rust registration (for static linking & web targets)
pub fn register(engine: &mut WolfEngine) {
    println!(">>> My Rust Plugin is being registered!");

    engine.push_fn("hello_from_rust", |_args| {
        println!(">>> Hello! This message comes from a Rust dynamic plugin!");
        Token::Unknown
    });
}

// C-ABI exported entrypoint for dynamic loading on desktop
#[no_mangle]
#[cfg(not(target_arch = "wasm32"))]
pub extern "C" fn register_talu_plugin(engine_ptr: *mut WolfEngine) {
    if engine_ptr.is_null() { return; }
    let engine = unsafe { &mut *engine_ptr };
    register(engine);
}

