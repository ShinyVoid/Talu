use std::path::PathBuf;
use wolflang::WolfEngine;

#[cfg(not(target_arch = "wasm32"))]
pub type PluginHandle = libloading::Library;

#[cfg(target_arch = "wasm32")]
pub struct PluginHandle;

/// Loads dynamic plugins on desktop (Windows, Linux, macOS)
#[cfg(not(target_arch = "wasm32"))]
pub fn load_packages(engine: &mut WolfEngine, plugins: Vec<String>) -> Vec<PluginHandle> {
    let mut loaded_libs = Vec::new();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    println!("Package loader starting. CWD: {:?}", cwd);

    for plugin_name in plugins {
        if plugin_name.is_empty() { continue; }

        let lib_filename = format!(
            "{}{}.{}",
            std::env::consts::DLL_PREFIX,
            plugin_name,
            std::env::consts::DLL_EXTENSION
        );

        // Potential search paths for the dynamic library
        let mut search_paths = vec![
            cwd.join("packages").join(&lib_filename),
            cwd.join("packages").join(&plugin_name).join("target/release").join(&lib_filename),
            cwd.join(&lib_filename),
        ];

        // Also check relative to executable (engine root)
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                search_paths.push(exe_dir.join("packages").join(&lib_filename));
                search_paths.push(exe_dir.join(&lib_filename));

                if let Some(engine_root) = exe_dir.parent().and_then(|p| p.parent()) {
                    search_paths.push(engine_root.join("packages").join(&lib_filename));
                    search_paths.push(engine_root.join("packages").join(&plugin_name).join("target/release").join(&lib_filename));
                }
            }
        }

        let mut loaded = false;
        for path in search_paths {
            let abs_path = if path.is_absolute() { path } else { cwd.join(path) };

            if abs_path.exists() {
                unsafe {
                    match libloading::Library::new(&abs_path) {
                        Ok(lib) => {
                            match lib.get::<unsafe extern "C" fn(*mut WolfEngine)>(b"register_talu_plugin") {
                                Ok(func) => {
                                    func(engine as *mut WolfEngine);
                                    loaded_libs.push(lib);
                                    loaded = true;
                                    println!("Successfully activated Rust plugin: '{}' from {:?}", plugin_name, abs_path);
                                    break;
                                }
                                Err(e) => {
                                    eprintln!("Failed to find symbol 'register_talu_plugin' in {:?}: {:?}", abs_path, e);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to load Rust dynamic library {:?}: {:?}", abs_path, e);
                        }
                    }
                }
            }
        }

        if !loaded {
            eprintln!("Warning: Could not find or load dynamic plugin '{}'", plugin_name);
        }
    }

    loaded_libs
}

/// Fallback for WebAssembly / Web targets (no dynamic linking / dlopen)
#[cfg(target_arch = "wasm32")]
pub fn load_packages(_engine: &mut WolfEngine, plugins: Vec<String>) -> Vec<PluginHandle> {
    if !plugins.is_empty() {
        println!("Web build: dynamic library loading is disabled. Plugins are statically linked: {:?}", plugins);
    }
    Vec::new()
}
