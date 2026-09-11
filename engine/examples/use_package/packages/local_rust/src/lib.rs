use wolflang::{WolfEngine, tokens::Token};

// Safe Rust registration (for static linking & web targets)
pub fn register(engine: &mut WolfEngine) {
    println!("Project-level Rust package registered successfully!");
    engine.push_fn("local_rust_hello", |_args| {
        println!("Hello from local_rust (project-level Rust package)!");
        Token::Integer(42)
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

