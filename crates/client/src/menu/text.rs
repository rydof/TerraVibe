//! Menu text component and layout systems.

use bevy::prelude::*;
use terravibe_assets::SpriteFont;

use crate::hud::HudFonts;
use crate::hud::text::{layout_text, measure};

/// Text color for `MenuText` glyphs. When omitted, defaults to `Color::WHITE`.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct MenuTextColor(pub Color);

/// Opts a `MenuText` entity out of the default 1px dark drop shadow.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuTextNoShadow;

/// Component representing SpriteFont-rendered text in the menu.
#[derive(Component, Clone, Debug)]
pub struct MenuText {
    pub text: String,
    pub scale: f32,
}

impl MenuText {
    pub fn new(text: impl Into<String>, scale: f32) -> Self {
        Self {
            text: text.into(),
            scale,
        }
    }
}

/// Helper calculating scaled pixel size of text using a SpriteFont.
pub fn measure_scaled(font: &SpriteFont, text: &str, scale: f32) -> Vec2 {
    let w = measure(font, text) * scale;
    let lines = text.lines().count().max(1) as f32;
    let h = font.line_spacing as f32 * lines * scale;
    Vec2::new(w, h)
}

/// Rebuilds children quads when `MenuText` or `MenuTextColor` changes.
#[allow(clippy::type_complexity)]
pub fn rebuild_menu_text(
    mut commands: Commands,
    mut q: Query<
        (
            Entity,
            &MenuText,
            Option<&MenuTextColor>,
            Has<MenuTextNoShadow>,
            Option<&Children>,
            Option<&mut Node>,
        ),
        Or<(Changed<MenuText>, Changed<MenuTextColor>)>,
    >,
    fonts: Option<Res<HudFonts>>,
) {
    let Some(fonts) = fonts else { return };
    for (e, t, color, no_shadow, children, maybe_node) in &mut q {
        for c in children.into_iter().flatten() {
            commands
                .entity(*c)
                .queue_silenced(move |entity: EntityWorldMut| {
                    entity.despawn();
                });
        }
        let (font, image) = (&fonts.mouse.0, &fonts.mouse.1);
        let scale = t.scale.max(0.1);
        let size = measure_scaled(font, &t.text, scale);
        let text_color = color.map(|c| c.0).unwrap_or(Color::WHITE);

        if let Some(mut node) = maybe_node {
            if node.position_type == PositionType::Relative {
                node.width = Val::Px(size.x);
                node.height = Val::Px(size.y);
            }
        } else {
            commands
                .entity(e)
                .queue_silenced(move |mut entity: EntityWorldMut| {
                    entity.insert(Node {
                        width: Val::Px(size.x),
                        height: Val::Px(size.y),
                        ..default()
                    });
                });
        }

        let mut glyphs = Vec::new();
        for (g, [x, y]) in layout_text(font, &t.text) {
            let [sx, sy, w, h] = g.src;
            let rect = Rect::new(sx as f32, sy as f32, (sx + w) as f32, (sy + h) as f32);
            glyphs.push((
                rect,
                x * scale,
                y * scale,
                w as f32 * scale,
                h as f32 * scale,
            ));
        }

        let img_handle = image.clone();
        let draw_shadow = !no_shadow;
        commands
            .entity(e)
            .queue_silenced(move |mut entity: EntityWorldMut| {
                entity.with_children(|parent| {
                    if draw_shadow {
                        let shadow_color = Color::srgba(0.02, 0.03, 0.06, 0.88);
                        for &(rect, left, top, width, height) in &glyphs {
                            let mut shadow_img = ImageNode::new(img_handle.clone());
                            shadow_img.rect = Some(rect);
                            shadow_img.color = shadow_color;
                            parent.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: Val::Px(left + 1.0),
                                    top: Val::Px(top + 1.0),
                                    width: Val::Px(width),
                                    height: Val::Px(height),
                                    ..default()
                                },
                                shadow_img,
                            ));
                        }
                    }
                    for (rect, left, top, width, height) in glyphs {
                        let mut img = ImageNode::new(img_handle.clone());
                        img.rect = Some(rect);
                        img.color = text_color;
                        parent.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(left),
                                top: Val::Px(top),
                                width: Val::Px(width),
                                height: Val::Px(height),
                                ..default()
                            },
                            img,
                        ));
                    }
                });
            });
    }
}

pub struct TextPlugin;

impl Plugin for TextPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, rebuild_menu_text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use terravibe_assets::{Glyph, RgbaImage, SpriteFont};

    fn dummy_font() -> SpriteFont {
        SpriteFont {
            atlas: RgbaImage {
                width: 1,
                height: 1,
                pixels: vec![[0; 4]],
            },
            glyphs: vec![Glyph {
                ch: 'a',
                src: [0, 0, 4, 4],
                crop: [0, 0, 4, 4],
                kerning: [0.0, 4.0, 0.0],
            }],
            line_spacing: 10,
            spacing: 0.0,
            default_char: None,
        }
    }

    #[test]
    fn despawned_menu_text_entity_leaves_no_orphan_glyphs() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(TextPlugin);

        let mut images = Assets::<Image>::default();
        let handle = images.add(Image::default());
        let font = Arc::new(dummy_font());

        app.insert_resource(images);
        app.insert_resource(HudFonts {
            mouse: (font.clone(), handle.clone()),
            death: (font.clone(), handle.clone()),
            stack: (font.clone(), handle.clone()),
        });

        // Spawn an entity with MenuText
        let e = app.world_mut().spawn(MenuText::new("a", 1.0)).id();

        // Despawn the entity before rebuild_menu_text deferred commands apply
        app.world_mut().despawn(e);

        app.update();

        // There should be NO ImageNode glyph entities orphaned in the world
        let glyph_count = app
            .world_mut()
            .query::<&ImageNode>()
            .iter(app.world())
            .count();
        assert_eq!(
            glyph_count, 0,
            "despawned MenuText entity must leave no orphan glyph entities in the world"
        );
    }
}
