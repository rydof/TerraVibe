//! Lays out SpriteFont strings and spawns them as UI image nodes.

use bevy::prelude::*;
use terravibe_assets::{Glyph, SpriteFont};

/// Resolves a glyph from the font, falling back from U+2019 right single quote
/// to ASCII apostrophe when the font lacks the curly glyph.
fn resolve_glyph(font: &SpriteFont, c: char) -> Option<&Glyph> {
    font.glyph(c).or_else(|| {
        if c == '\u{2019}' {
            font.glyph('\'')
        } else {
            None
        }
    })
}

/// Positions each glyph of `s` (top-left, px) the way a SpriteFont draws: the pen starts at
/// the left bearing, glyph crop offsets shift the quad, and the pen advances by
/// `kl + kw + kr + spacing`. `\n` starts a new line. Unknown chars are skipped.
pub fn layout_text(font: &SpriteFont, s: &str) -> Vec<(Glyph, [f32; 2])> {
    let mut out = Vec::new();
    let (mut x, mut y) = (0.0f32, 0.0f32);
    for c in s.chars() {
        if c == '\n' {
            x = 0.0;
            y += font.line_spacing as f32;
            continue;
        }
        let Some(g) = resolve_glyph(font, c) else {
            continue;
        };
        let [kl, kw, kr] = g.kerning;
        out.push((g.clone(), [x + kl + g.crop[0] as f32, y + g.crop[1] as f32]));
        x += kl + kw + kr + font.spacing;
    }
    out
}

/// Width in px of the widest line of `s`.
pub fn measure(font: &SpriteFont, s: &str) -> f32 {
    s.lines()
        .map(|l| {
            l.chars()
                .filter_map(|c| resolve_glyph(font, c))
                .map(|g| g.kerning.iter().sum::<f32>() + font.spacing)
                .sum::<f32>()
        })
        .fold(0.0, f32::max)
}

/// Marks a `HudText` drawn with the `Item_Stack` font.
#[derive(Component)]
pub struct StackText;

/// Text color for `HudText` glyphs. When omitted, defaults to `Color::WHITE`.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct HudTextColor(pub Color);

/// A UI node whose children are the glyph quads of `text`; rebuilt when it changes.
#[derive(Component)]
pub struct HudText {
    pub text: String,
    pub death: bool,
}

pub fn rebuild_text(
    mut commands: Commands,
    mut q: Query<
        (
            Entity,
            &HudText,
            Option<&Children>,
            Has<StackText>,
            Option<&HudTextColor>,
            Option<&mut Node>,
        ),
        Or<(Changed<HudText>, Changed<HudTextColor>)>,
    >,
    fonts: Res<super::HudFonts>,
) {
    let px_of = |v: Val| match v {
        Val::Px(x) => x,
        _ => 0.0,
    };
    for (e, t, children, stack, color, mut node) in &mut q {
        for c in children.into_iter().flatten() {
            commands.entity(*c).despawn();
        }
        let (font, image) = if stack {
            (&fonts.stack.0, &fonts.stack.1)
        } else if t.death {
            (&fonts.death.0, &fonts.death.1)
        } else {
            (&fonts.mouse.0, &fonts.mouse.1)
        };
        let total_w = measure(font, &t.text);
        let total_h = if t.text.is_empty() {
            0.0
        } else {
            t.text.split('\n').count() as f32 * font.line_spacing as f32
        };
        let (pad_l, pad_t) = if let Some(ref mut n) = node {
            let pl = px_of(n.padding.left);
            let pr = px_of(n.padding.right);
            let pt = px_of(n.padding.top);
            let pb = px_of(n.padding.bottom);
            let bl = px_of(n.border.left);
            let br = px_of(n.border.right);
            let bt = px_of(n.border.top);
            let bb = px_of(n.border.bottom);
            if n.position_type == PositionType::Relative {
                n.width = Val::Px(if total_w > 0.0 {
                    total_w + pl + pr + bl + br
                } else {
                    0.0
                });
                n.height = Val::Px(if total_h > 0.0 {
                    total_h + pt + pb + bt + bb
                } else {
                    0.0
                });
            } else {
                if n.width == Val::Auto {
                    n.width = Val::Px(total_w);
                }
                if n.height == Val::Auto {
                    n.height = Val::Px(total_h);
                }
            }
            (pl, pt)
        } else {
            (0.0, 0.0)
        };

        let text_color = color.map(|c| c.0).unwrap_or(Color::WHITE);
        let shadow_color = if stack {
            Color::srgb(0.0, 0.0, 0.0)
        } else {
            Color::srgba(0.0, 0.0, 0.0, 0.85)
        };
        for (g, [x, y]) in layout_text(font, &t.text) {
            let [sx, sy, w, h] = g.src;
            let rect = Some(Rect::new(
                sx as f32,
                sy as f32,
                (sx + w) as f32,
                (sy + h) as f32,
            ));
            let gx = x + pad_l;
            let gy = y + pad_t;

            // 1px dark drop shadow for readability over bright sky/snow/UI
            let mut shadow = ImageNode::new(image.clone());
            shadow.rect = rect;
            shadow.color = shadow_color;
            let shadow_child = commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(gx + 1.0),
                        top: Val::Px(gy + 1.0),
                        width: Val::Px(w as f32),
                        height: Val::Px(h as f32),
                        ..default()
                    },
                    shadow,
                ))
                .id();
            commands.entity(e).add_child(shadow_child);

            let mut img = ImageNode::new(image.clone());
            img.rect = rect;
            img.color = text_color;
            let child = commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(gx),
                        top: Val::Px(gy),
                        width: Val::Px(w as f32),
                        height: Val::Px(h as f32),
                        ..default()
                    },
                    img,
                ))
                .id();
            commands.entity(e).add_child(child);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terravibe_assets::RgbaImage;

    fn font() -> SpriteFont {
        let g = |ch, kerning| Glyph {
            ch,
            src: [0, 0, 4, 4],
            crop: [0, 0, 4, 4],
            kerning,
        };
        SpriteFont {
            atlas: RgbaImage {
                width: 1,
                height: 1,
                pixels: vec![[0; 4]],
            },
            glyphs: vec![
                g('a', [1.0, 5.0, 2.0]),
                g('b', [0.0, 3.0, 1.0]),
                g('\'', [0.0, 2.0, 1.0]),
            ],
            line_spacing: 10,
            spacing: 0.5,
            default_char: None,
        }
    }

    #[test]
    fn layout_text_advances() {
        let l = layout_text(&font(), "aba\nb");
        let pos: Vec<[f32; 2]> = l.iter().map(|p| p.1).collect();
        // a: pen 0 + kl 1; advance 1+5+2+0.5 = 8.5. b: 8.5 + 0; advance 4.5 -> 13. a: 14.
        assert_eq!(pos, vec![[1.0, 0.0], [8.5, 0.0], [14.0, 0.0], [0.0, 10.0]]);
        assert_eq!(measure(&font(), "ab"), 13.0);
    }

    #[test]
    fn layout_text_curly_apostrophe_fallback() {
        let f = font();
        let input_with_curly = "a\u{2019}b";
        let l = layout_text(&f, input_with_curly);
        let chars: Vec<char> = l.iter().map(|(g, _)| g.ch).collect();
        assert_eq!(chars, vec!['a', '\'', 'b']);
        assert_eq!(measure(&f, input_with_curly), measure(&f, "a'b"));
    }

    #[test]
    fn rebuild_text_triggers_on_hud_text_color_change() {
        use std::sync::Arc;

        let mut app = App::new();
        let f = Arc::new(font());
        let handle = Handle::<Image>::default();
        app.insert_resource(super::super::HudFonts {
            mouse: (f.clone(), handle.clone()),
            death: (f.clone(), handle.clone()),
            stack: (f, handle),
        });
        let e = app
            .world_mut()
            .spawn((
                HudText {
                    text: "a".to_string(),
                    death: false,
                },
                HudTextColor(Color::WHITE),
            ))
            .id();
        app.add_systems(Update, rebuild_text);
        app.update();

        let children = app.world().get::<Children>(e).unwrap();
        assert_eq!(children.len(), 2);
        let child_e = children[1];
        let img = app.world().get::<ImageNode>(child_e).unwrap();
        assert_eq!(img.color, Color::WHITE);

        let blue = Color::srgb(0.0, 0.0, 1.0);
        app.world_mut().get_mut::<HudTextColor>(e).unwrap().0 = blue;
        app.update();

        let children = app.world().get::<Children>(e).unwrap();
        assert_eq!(children.len(), 2);
        let child_e = children[1];
        let img = app.world().get::<ImageNode>(child_e).unwrap();
        assert_eq!(img.color, blue);
    }

    #[test]
    fn rebuild_text_sizes_relative_flexbox_node() {
        use std::sync::Arc;

        let mut app = App::new();
        let f = Arc::new(font());
        let handle = Handle::<Image>::default();
        app.insert_resource(super::super::HudFonts {
            mouse: (f.clone(), handle.clone()),
            death: (f.clone(), handle.clone()),
            stack: (f, handle),
        });
        let e = app
            .world_mut()
            .spawn((
                HudText {
                    text: "ab".to_string(),
                    death: false,
                },
                Node::default(),
            ))
            .id();
        app.add_systems(Update, rebuild_text);
        app.update();

        let node = app.world().get::<Node>(e).unwrap();
        assert_eq!(node.width, Val::Px(13.0));
        assert_eq!(node.height, Val::Px(10.0));
    }
}
