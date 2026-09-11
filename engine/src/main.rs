use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use wolflang::WolfEngine;
use wolflang::tokens::Token;
use raylib::prelude::*;
use raylib::color::Color;

mod engine;
mod setup;
use engine::utils::utils::*;

use engine::draw::{draw_register, Shapes};
use engine::input::input as input_register;
use crate::engine::asset_pipeline::load_assets;

use setup::cli;

/// Initializes a fresh WolfEngine and registers all engine subsystems and plugins
fn init_wolf_engine(
    draw_list: Arc<Mutex<Vec<Shapes>>>,
    plugins: &[String],
    code: &str,
) -> Result<WolfEngine, String> {
    let mut engine = WolfEngine::new();

    // Register engine subsystems
    draw_register(&mut engine, draw_list);
    input_register(&mut engine);
    crate::engine::physics::physics_register(&mut engine);

    engine.push_fn("floatToInt", |args| {
        if let Some(Token::Float(f)) = args.get(0) {
            return Token::Integer(*f as i64);
        }
        Token::Integer(0)
    });

    // Load custom plugins
    let _ = crate::engine::packages::load_packages(&mut engine, plugins.to_vec());

    // Execute script and call start()
    engine.run(code).map_err(|e| format!("Script Error: {:?}", e))?;
    engine.run("start()").map_err(|e| format!("start() Error: {:?}", e))?;

    Ok(engine)
}

/// Finds the most recent modification time among .wolf files in the project
fn get_latest_script_mtime(run_path: &str) -> SystemTime {
    let mut latest = std::fs::metadata(run_path)
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);

    if let Ok(entries) = std::fs::read_dir(".") {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("wolf") {
                if let Ok(meta) = entry.metadata() {
                    if let Ok(mtime) = meta.modified() {
                        if mtime > latest {
                            latest = mtime;
                        }
                    }
                }
            }
        }
    }

    latest
}

fn main() {
    let (config_path, run_path, plugins) = cli().expect("Setup failed. Check package.talu and directory.");
    let draw_list: Arc<Mutex<Vec<Shapes>>> = Arc::new(Mutex::new(vec![]));
    let mut config_engine: WolfEngine = WolfEngine::new();

    let game_config = std::fs::read_to_string(&config_path).map_err(|_| "Config read failed").expect("Config read failed");
    config_engine.run(&game_config).expect("Config script failed");

    let screen_size_x = opt_to_i32(config_engine.get_int("screen_size_x"));
    let screen_size_y = opt_to_i32(config_engine.get_int("screen_size_y"));

    let t: Option<String> = config_engine.get_str("title");
    let title: &str = get_str_slice(&t);

    let (mut rl, thread) = raylib::init()
        .size(screen_size_x, screen_size_y)
        .title(title)
        .build();

    // Load assets (textures)
    let texture_registry = load_assets(&mut rl, &thread);

    // Initial script load
    let code = std::fs::read_to_string(&run_path).unwrap_or_default();
    let mut script_crashed = false;
    let mut crash_message = String::new();

    let initial_engine_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        init_wolf_engine(draw_list.clone(), &plugins, &code)
    }));

    let mut engine: Option<WolfEngine> = match initial_engine_result {
        Ok(Ok(eng)) => Some(eng),
        Ok(Err(err)) => {
            script_crashed = true;
            crash_message = err;
            None
        }
        Err(panic_err) => {
            script_crashed = true;
            if let Some(s) = panic_err.downcast_ref::<&'static str>() {
                crash_message = s.to_string();
            } else if let Some(s) = panic_err.downcast_ref::<String>() {
                crash_message = s.clone();
            } else {
                crash_message = "Script panic during initialization".to_string();
            }
            None
        }
    };

    // Hot-reload tracking
    let mut last_script_mod_time = get_latest_script_mtime(&run_path);
    let mut last_reload_check = Instant::now();
    let mut reload_toast_timer: f32 = 0.0;

    let mut last_frame_time = Instant::now();

    while !rl.window_should_close() {
        let now = Instant::now();
        let delta_time: f64 = now.duration_since(last_frame_time).as_secs_f64();
        last_frame_time = now;

        // --- HOT RELOAD CHECK (Every 150ms) ---
        if now.duration_since(last_reload_check) >= Duration::from_millis(150) {
            last_reload_check = now;
            let current_mtime = get_latest_script_mtime(&run_path);

            if current_mtime > last_script_mod_time {
                last_script_mod_time = current_mtime;

                // Brief pause to allow editor file flushes
                std::thread::sleep(Duration::from_millis(20));

                if let Ok(new_code) = std::fs::read_to_string(&run_path) {
                    println!("🔥 [Talu HotReload] File change detected, reloading scripts...");

                    let reload_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        init_wolf_engine(draw_list.clone(), &plugins, &new_code)
                    }));

                    match reload_result {
                        Ok(Ok(new_engine)) => {
                            engine = Some(new_engine);
                            script_crashed = false;
                            reload_toast_timer = 2.0;
                            draw_list.lock().unwrap().clear();
                            println!("✅ [Talu HotReload] Reload successful!");
                        }
                        Ok(Err(err)) => {
                            script_crashed = true;
                            crash_message = err;
                            eprintln!("⚠️ [Talu HotReload] Evaluation failed: {}", crash_message);
                        }
                        Err(panic_err) => {
                            script_crashed = true;
                            if let Some(s) = panic_err.downcast_ref::<&'static str>() {
                                crash_message = s.to_string();
                            } else if let Some(s) = panic_err.downcast_ref::<String>() {
                                crash_message = s.clone();
                            } else {
                                crash_message = "Script panic during hot-reload".to_string();
                            }
                            eprintln!("⚠️ [Talu HotReload] Panic: {}", crash_message);
                        }
                    }
                }
            }
        }

        // --- UPDATE & RUN SCRIPT ---
        if let Some(ref mut eng) = engine {
            eng.push_float("deltaTime", delta_time);

            if !script_crashed {
                draw_list.lock().unwrap().clear();

                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    eng.run("update()")
                }));

                match result {
                    Ok(Err(eval_err)) => {
                        script_crashed = true;
                        crash_message = format!("Eval Error: {:?}", eval_err);
                    }
                    Err(panic_err) => {
                        script_crashed = true;
                        if let Some(s) = panic_err.downcast_ref::<&'static str>() {
                            crash_message = s.to_string();
                        } else if let Some(s) = panic_err.downcast_ref::<String>() {
                            crash_message = s.clone();
                        } else {
                            crash_message = "Unknown script panic".to_string();
                        }
                    }
                    _ => {}
                }
            }
        }

        // --- DRAWING ---
        let mut d = rl.begin_drawing(&thread);
        d.clear_background(Color::BLACK);

        // Draw game shapes collected during update
        if !script_crashed {
            let shapes = draw_list.lock().unwrap();
            for shape in shapes.iter() {
                match shape {
                    Shapes::Circle { pos, rad, col } => d.draw_circle_v(pos, *rad, col),
                    Shapes::Rectangle { pos, size, col } => d.draw_rectangle_v(pos, size, col),
                    Shapes::Line { start, end, col } => d.draw_line_v(start, end, col),
                    Shapes::Texture { pos, tex_id, col } => {
                        if let Some(texture) = texture_registry.get(tex_id) {
                            d.draw_texture_v(texture, *pos, *col);
                        }
                    }
                }
            }
        }

        // Crash overlay
        if script_crashed {
            d.draw_rectangle(0, 0, screen_size_x, 75, Color::new(220, 30, 30, 220));
            d.draw_text(&format!("SCRIPT CRASHED: {}", crash_message), 10, 12, 18, Color::WHITE);
            d.draw_text("Fix the script in your editor and save (Ctrl+S) to Hot Reload.", 10, 42, 16, Color::YELLOW);
        }

        // Hot reload notification toast
        if reload_toast_timer > 0.0 {
            reload_toast_timer -= delta_time as f32;

            let alpha = if reload_toast_timer < 0.5 {
                ((reload_toast_timer / 0.5) * 230.0) as u8
            } else {
                230
            };

            let toast_text = "🔥 Script Reloaded!";
            let text_w = d.measure_text(toast_text, 16);
            let box_w = text_w + 30;
            let box_h = 32;
            let box_x = screen_size_x - box_w - 15;
            let box_y = 15;

            d.draw_rectangle_rounded(
                Rectangle::new(box_x as f32, box_y as f32, box_w as f32, box_h as f32),
                0.3,
                4,
                Color::new(34, 139, 34, alpha),
            );
            d.draw_text(toast_text, box_x + 15, box_y + 8, 16, Color::new(255, 255, 255, alpha));
        }
    }
}
