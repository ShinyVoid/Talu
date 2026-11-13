use std::sync::{Arc, Mutex};
use raylib::prelude::*;
use wolflang::{WolfEngine, tokens::Token};
use crate::engine::draw::Shapes;


#[derive(Debug, PartialEq)]
pub enum Kind { Sprite, Rect }

#[derive(Debug)]
pub struct EntityDef {
    pub kind: Kind,
    pub name: String,
    pub image: Option<String>,
    pub x: f32, pub y: f32,
    pub w: f32, pub h: f32,
    pub rot: f32, pub scale: f32,
    pub color: Color,
    pub tags: Vec<String>,
}

#[derive(Debug)]
pub struct SceneError { pub line: usize, pub msg: String }

fn err(line: usize, msg: impl Into<String>) -> SceneError {
    SceneError { line, msg: msg.into() }
}

fn num(v: &str, line: usize, key: &str) -> Result<f32, SceneError> {
    v.parse().map_err(|_| err(line, format!("{key}: number expected, found: '{v}'")))
}

fn parse_hex_color(hex: &str) -> Color {
    let clean_hex = hex.trim_start_matches('#');
    if clean_hex.len() == 6 {
        let r = u8::from_str_radix(&clean_hex[0..2], 16).unwrap_or(255);
        let g = u8::from_str_radix(&clean_hex[2..4], 16).unwrap_or(255);
        let b = u8::from_str_radix(&clean_hex[4..6], 16).unwrap_or(255);
        Color::new(r, g, b, 255)
    } else {
        Color::WHITE
    }
}

pub fn parse_scene(src: &str) -> Result<Vec<EntityDef>, SceneError> {
    let mut out = Vec::new();
    for (i, raw) in src.lines().enumerate() {
        let ln = i + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') { continue; }

        let mut words = line.split_whitespace();
        if words.next() != Some("add") {
            return Err(err(ln, "Line needs to be start with 'add' "));
        }
        let kind = match words.next() {
            Some("sprite") => Kind::Sprite,
            Some("rect") => Kind::Rect,
            Some(o) => return Err(err(ln, format!("Unknown Type: '{o}'"))),
            None => return Err(err(ln, "Type missing")),
        };
        let name = words.next().ok_or_else(|| err(ln, "Name not found"))?.to_string();

        let mut d = EntityDef {
            kind,
            name,
            image: None,
            x: 0.0, y: 0.0,
            w: 50.0, h: 50.0,
            rot: 0.0, scale: 1.0,
            color: Color::WHITE,
            tags: vec![],
        };
        
        for w in words {
            let (k, v) = w.split_once('=')
                .ok_or_else(|| err(ln, format!("'{w}' anahtar=değer olmalı")))?;
            match k {
                "image" => d.image = Some(v.to_string()),
                "x" => d.x = num(v, ln, k)?,
                "y" => d.y = num(v, ln, k)?,
                "w" => d.w = num(v, ln, k)?,    
                "h" => d.h = num(v, ln, k)?,
                "color" => d.color = parse_hex_color(v),
                "rot" => d.rot = num(v, ln, k)?,
                "scale" => d.scale = num(v, ln, k)?,
                "tag" => d.tags = v.split(',').map(String::from).collect(),
                _ => return Err(err(ln, format!("bilinmeyen anahtar '{k}'"))),
            }
        }
        out.push(d);

    }

    Ok(out)
}

pub fn inject_scene(entities: &[EntityDef], draw_list: &Arc<Mutex<Vec<Shapes>>>) {
    let mut list = draw_list.lock().unwrap();

    for entity in entities {
        match entity.kind {
            Kind::Rect => {
                // Varsayılan boyutlar ve renk ile Rectangle ekleme
                list.push(Shapes::Rectangle {
                    pos: Vector2 { x: entity.x, y: entity.y },
                    size: Vector2 { x: entity.w * entity.scale, y: entity.h * entity.scale },
                    col: entity.color,
                });
            }
            Kind::Sprite => {
                // Dokulardan çekmek için (Texture ID varsayılan 0 veya asset registry üzerinden belirlenebilir)
                list.push(Shapes::Texture {
                    pos: Vector2 { x: entity.x, y: entity.y },
                    tex_id: 0,
                    col: entity.color,
                });
            }
        }
    }
}


pub fn scene_register(engine: &mut WolfEngine, draw_list: Arc<Mutex<Vec<Shapes>>>) {
    let dl = draw_list.clone();
    
    engine.push_fn("loadScene", move |args| {
        // Betikten gelen dosya yolunu al (örnek: "scene.tas")
        if let Some(Token::String(path)) = args.get(0) {
            match std::fs::read_to_string(path) {
                Ok(content) => {
                    match parse_scene(&content) {
                        Ok(entities) => {
                            inject_scene(&entities, &dl);
                            println!("✅ [Scene] Sahne yüklendi: {}", path);
                        }
                        Err(e) => {
                            eprintln!("⚠️ [Scene] Parse hatası (Satır {}): {}", e.line, e.msg);
                        }
                    }
                }
                Err(_) => {
                    eprintln!("⚠️ [Scene] Dosya okunamadı: {}", path);
                }
            }
        }
        Token::Unknown
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_sprite() {
        let defs = parse_scene("add sprite player").unwrap();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].kind, Kind::Sprite);
        assert_eq!(defs[0].name, "player");
    }

    #[test]
    fn recognizes_rect() {
        let defs = parse_scene("add rect floor").unwrap();
        assert_eq!(defs[0].kind, Kind::Rect);
    }

    #[test]
    fn unknown_kind_is_error() {
        let e = parse_scene("add circle ball").unwrap_err();
        assert_eq!(e.line, 1);
        assert!(e.msg.contains("circle"));
    }

    #[test]
    fn line_must_start_with_add() {
        let e = parse_scene("foo sprite player").unwrap_err();
        assert_eq!(e.line, 1);
    }

    #[test]
    fn defaults_are_applied() {
        let d = &parse_scene("add sprite player").unwrap()[0];
        assert_eq!((d.x, d.y, d.rot, d.scale), (0.0, 0.0, 0.0, 1.0));
        assert!(d.image.is_none());
        assert!(d.tags.is_empty());
    }

    #[test]
    fn parses_key_values() {
        let src = "add sprite goblin image=goblin.png x=300 y=500 rot=45 scale=2 tag=enemy,flying";
        let d = &parse_scene(src).unwrap()[0];
        assert_eq!(d.image.as_deref(), Some("goblin.png"));
        assert_eq!((d.x, d.y, d.rot, d.scale), (300.0, 500.0, 45.0, 2.0));
        assert_eq!(d.tags, vec!["enemy", "flying"]);
    }

    #[test]
    fn skips_comments_and_blank_lines() {
        let src = "# sahne\n\nadd sprite a\n  \n# bitti\nadd rect b";
        let defs = parse_scene(src).unwrap();
        assert_eq!(defs.len(), 2);
    }

    #[test]
    fn error_reports_real_line_number() {
        let src = "# yorum\nadd sprite a\n\nadd circle b";
        let e = parse_scene(src).unwrap_err();
        assert_eq!(e.line, 4);
    }

    #[test]
    fn bad_number_is_error() {
        let e = parse_scene("add sprite a x=abc").unwrap_err();
        assert!(e.msg.contains("x"));
    }

    #[test]
    fn unknown_key_is_error() {
        let e = parse_scene("add sprite a scal=2").unwrap_err();
        assert!(e.msg.contains("scal"));
    }

    #[test]
    fn missing_name_is_error() {
        let e = parse_scene("add sprite").unwrap_err();
        assert!(e.msg.contains("isim"));
    }
}