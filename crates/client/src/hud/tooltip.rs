//! Unified item tooltip builder, positioning, hover delay, and rendering.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use terravibe_sim::coin::format_coins;
use terravibe_sim::crafting::is_material;
use terravibe_sim::inventory::ItemStack;
use terravibe_sim::item::{
    ItemClass, ItemData, ItemId, Prefix, Rarity, display_name, item_data, mana_cost, prefix_stats,
};

use super::HudFonts;
use super::text::{HudText, HudTextColor, measure};
use crate::session::AppState;

/// A single styled line inside an item tooltip.
#[derive(Clone, Debug, PartialEq)]
pub struct TooltipLine {
    pub text: String,
    pub color: Color,
}

/// Terraria 1.4-accurate item rarity color palette, or a high-contrast Okabe-Ito
/// inspired colorblind-safe palette when `colorblind` is true.
pub fn rarity_color_for(r: Rarity, colorblind: bool) -> Color {
    if !colorblind {
        return rarity_color(r);
    }
    match r {
        Rarity::Gray => Color::srgb_u8(155, 155, 155),
        Rarity::White => Color::WHITE,
        Rarity::Blue => Color::srgb_u8(0, 114, 178),
        Rarity::Green => Color::srgb_u8(0, 158, 115),
        Rarity::Orange => Color::srgb_u8(230, 159, 0),
        Rarity::LightRed => Color::srgb_u8(213, 94, 0),
        Rarity::Pink => Color::srgb_u8(204, 121, 167),
        Rarity::LightPurple => Color::srgb_u8(170, 130, 220),
        Rarity::Lime => Color::srgb_u8(86, 180, 233),
        Rarity::Yellow => Color::srgb_u8(240, 228, 66),
        Rarity::Cyan => Color::srgb_u8(100, 220, 255),
        Rarity::Red => Color::srgb_u8(255, 80, 40),
        Rarity::Purple => Color::srgb_u8(148, 70, 200),
    }
}

/// Terraria 1.4-accurate item rarity color palette.
pub fn rarity_color(r: Rarity) -> Color {
    match r {
        Rarity::Gray => Color::srgb_u8(130, 130, 130),
        Rarity::White => Color::WHITE,
        Rarity::Blue => Color::srgb_u8(150, 150, 255),
        Rarity::Green => Color::srgb_u8(150, 255, 150),
        Rarity::Orange => Color::srgb_u8(255, 200, 150),
        Rarity::LightRed => Color::srgb_u8(255, 150, 150),
        Rarity::Pink => Color::srgb_u8(255, 150, 255),
        Rarity::LightPurple => Color::srgb_u8(210, 160, 255),
        Rarity::Lime => Color::srgb_u8(150, 255, 10),
        Rarity::Yellow => Color::srgb_u8(255, 255, 10),
        Rarity::Cyan => Color::srgb_u8(5, 200, 255),
        Rarity::Red => Color::srgb_u8(255, 40, 100),
        Rarity::Purple => Color::srgb_u8(180, 40, 255),
    }
}

fn class_label(class: ItemClass) -> &'static str {
    match class {
        ItemClass::Melee => "Melee",
        ItemClass::Ranged => "Ranged",
        ItemClass::Ammo => "Ammo",
        ItemClass::Tool => "Tool",
        ItemClass::Block => "Block",
        ItemClass::Placeable => "Placeable",
        ItemClass::Wall => "Wall",
        ItemClass::Armor => "Armor",
        ItemClass::Consumable => "Consumable",
        ItemClass::Magic => "Magic",
        ItemClass::Accessory => "Accessory",
        ItemClass::Other => "Misc",
    }
}

fn speed_label(use_time: i32) -> &'static str {
    match use_time {
        ..=8 => "Insanely fast",
        9..=20 => "Very fast",
        21..=25 => "Fast",
        26..=30 => "Average",
        31..=35 => "Slow",
        36..=45 => "Very slow",
        46..=55 => "Extremely slow",
        _ => "Snail",
    }
}

fn knockback_label(kb: f32) -> &'static str {
    if kb <= 0.0 {
        "No"
    } else if kb <= 1.5 {
        "Extremely weak"
    } else if kb <= 3.0 {
        "Very weak"
    } else if kb <= 4.0 {
        "Weak"
    } else if kb <= 6.0 {
        "Average"
    } else if kb <= 7.0 {
        "Strong"
    } else if kb <= 9.0 {
        "Very strong"
    } else if kb <= 11.0 {
        "Extremely strong"
    } else {
        "Insane"
    }
}

fn push_stat_delta(lines: &mut Vec<TooltipLine>, label: &str, diff: i32) {
    let (text, color) = if diff > 0 {
        (
            format!("  (+{diff} {label})"),
            Color::srgb_u8(120, 255, 120),
        )
    } else if diff < 0 {
        (format!("  ({diff} {label})"), Color::srgb_u8(255, 120, 120))
    } else {
        (format!("  (same {label})"), Color::srgb_u8(180, 180, 180))
    };
    lines.push(TooltipLine { text, color });
}

/// Builds a unified, rarity-colored tooltip for any item stack using the default palette.
#[cfg(test)]
pub fn format_item_tooltip(
    stack: &ItemStack,
    equipped: Option<&ItemData>,
    price_override: Option<(&str, u32)>,
) -> Vec<TooltipLine> {
    format_item_tooltip_for(stack, equipped, price_override, false)
}

fn push_prefix_lines(lines: &mut Vec<TooltipLine>, data: &ItemData, prefix: Prefix) {
    if prefix.0 == 0 {
        return;
    }
    let pos_col = Color::srgb_u8(120, 255, 120);
    let neg_col = Color::srgb_u8(255, 120, 120);
    if data.class == ItemClass::Accessory {
        let bonus = match prefix.0 {
            62 => Some("+1 defense"),
            63 => Some("+2 defense"),
            64 => Some("+3 defense"),
            65 => Some("+4 defense"),
            66 => Some("+20 mana"),
            67 => Some("+2% critical strike chance"),
            68 => Some("+4% critical strike chance"),
            69 => Some("+1% damage"),
            70 => Some("+2% damage"),
            71 => Some("+3% damage"),
            72 => Some("+4% damage"),
            73 => Some("+1% movement speed"),
            74 => Some("+2% movement speed"),
            75 => Some("+3% movement speed"),
            76 => Some("+4% movement speed"),
            77 => Some("+1% melee speed"),
            78 => Some("+2% melee speed"),
            79 => Some("+3% melee speed"),
            80 => Some("+4% melee speed"),
            _ => None,
        };
        if let Some(txt) = bonus {
            lines.push(TooltipLine {
                text: txt.to_string(),
                color: pos_col,
            });
        }
        return;
    }
    let ps = prefix_stats(prefix);
    let push_pct = |lines: &mut Vec<TooltipLine>, pct: i32, label: &str, positive_good: bool| {
        if pct == 0 {
            return;
        }
        let good = if positive_good { pct > 0 } else { pct < 0 };
        lines.push(TooltipLine {
            text: format!("{pct:+}% {label}"),
            color: if good { pos_col } else { neg_col },
        });
    };
    push_pct(
        lines,
        ((ps.damage - 1.0) * 100.0).round() as i32,
        "damage",
        true,
    );
    push_pct(
        lines,
        ((1.0 - ps.use_time) * 100.0).round() as i32,
        "speed",
        true,
    );
    if ps.crit != 0 {
        push_pct(lines, ps.crit, "critical strike chance", true);
    }
    push_pct(
        lines,
        ((ps.velocity - 1.0) * 100.0).round() as i32,
        "velocity",
        true,
    );
    push_pct(
        lines,
        ((ps.mana - 1.0) * 100.0).round() as i32,
        "mana cost",
        false,
    );
    push_pct(
        lines,
        ((ps.scale - 1.0) * 100.0).round() as i32,
        "size",
        true,
    );
    push_pct(
        lines,
        ((ps.knock_back - 1.0) * 100.0).round() as i32,
        "knockback",
        true,
    );
}

/// Builds a unified, rarity-colored tooltip for any item stack with optional colorblind palette.
pub fn format_item_tooltip_for(
    stack: &ItemStack,
    equipped: Option<&ItemData>,
    price_override: Option<(&str, u32)>,
    colorblind: bool,
) -> Vec<TooltipLine> {
    let data = item_data(stack.id);
    let ps = prefix_stats(stack.prefix);
    let mut lines = Vec::new();

    let name = display_name(stack.id, stack.prefix);
    let title = if stack.count > 1 {
        format!("{name} ({})", stack.count)
    } else {
        name
    };
    lines.push(TooltipLine {
        text: title,
        color: rarity_color_for(data.rarity, colorblind),
    });

    lines.push(TooltipLine {
        text: class_label(data.class).to_string(),
        color: Color::srgb_u8(190, 200, 220),
    });

    let eff_damage = (data.damage as f32 * ps.damage).round_ties_even() as i32;
    let eff_crit = if stack.id == terravibe_sim::item::FIRECRACKER {
        0
    } else {
        (data.crit + ps.crit).max(0)
    };
    let eff_kb = data.knock_back * ps.knock_back;
    let eff_use = (data.use_time as f32 * ps.use_time)
        .round_ties_even()
        .max(1.0) as i32;
    let eff_mana = mana_cost(data.mana, stack.prefix);

    if eff_damage > 0 {
        lines.push(TooltipLine {
            text: format!("{eff_damage} damage"),
            color: Color::WHITE,
        });
        if eff_crit > 0 {
            lines.push(TooltipLine {
                text: format!("{eff_crit}% critical strike chance"),
                color: Color::WHITE,
            });
        }
        if eff_kb > 0.0 {
            lines.push(TooltipLine {
                text: format!("{:.1} knockback ({})", eff_kb, knockback_label(eff_kb)),
                color: Color::WHITE,
            });
        }
    }

    if data.use_time > 0 && (data.damage > 0 || data.pick > 0 || data.axe > 0 || data.hammer > 0) {
        lines.push(TooltipLine {
            text: format!("{} speed (Use time: {eff_use})", speed_label(eff_use)),
            color: Color::WHITE,
        });
    }

    if data.pick > 0 {
        lines.push(TooltipLine {
            text: format!("{}% pickaxe power", data.pick),
            color: Color::WHITE,
        });
    }
    if data.axe > 0 {
        lines.push(TooltipLine {
            text: format!("{}% axe power", data.axe * 5),
            color: Color::WHITE,
        });
    }
    if data.hammer > 0 {
        lines.push(TooltipLine {
            text: format!("{}% hammer power", data.hammer),
            color: Color::WHITE,
        });
    }
    if eff_mana > 0 {
        lines.push(TooltipLine {
            text: format!("Uses {eff_mana} mana"),
            color: Color::srgb_u8(150, 180, 255),
        });
    }
    if data.defense > 0 {
        lines.push(TooltipLine {
            text: format!("{} defense", data.defense),
            color: Color::WHITE,
        });
    }
    if data.buff_secs > 0 {
        lines.push(TooltipLine {
            text: format!("{}s duration", data.buff_secs),
            color: Color::srgb_u8(180, 220, 255),
        });
    }

    push_prefix_lines(&mut lines, data, stack.prefix);

    if let Some(eq) = equipped {
        lines.push(TooltipLine {
            text: format!("Equipped: {}", eq.name),
            color: Color::srgb_u8(180, 180, 180),
        });
        if data.defense > 0 || eq.defense > 0 {
            push_stat_delta(&mut lines, "defense", data.defense - eq.defense);
        }
        if eff_damage > 0 || eq.damage > 0 {
            push_stat_delta(&mut lines, "damage", eff_damage - eq.damage);
        }
    }

    if !data.tooltip.is_empty() {
        lines.push(TooltipLine {
            text: data.tooltip.to_string(),
            color: Color::srgb_u8(215, 215, 215),
        });
    }

    if matches!(
        stack.id,
        terravibe_sim::item::COPPER_HELMET
            | terravibe_sim::item::COPPER_CHAINMAIL
            | terravibe_sim::item::COPPER_GREAVES
            | terravibe_sim::item::IRON_HELMET
            | terravibe_sim::item::IRON_CHAINMAIL
            | terravibe_sim::item::IRON_GREAVES
    ) {
        lines.push(TooltipLine {
            text: "Set bonus: +2 defense".to_string(),
            color: Color::srgb_u8(165, 235, 165),
        });
    } else if matches!(
        stack.id,
        terravibe_sim::item::SILVER_HELMET
            | terravibe_sim::item::SILVER_CHAINMAIL
            | terravibe_sim::item::SILVER_GREAVES
            | terravibe_sim::item::GOLD_HELMET
            | terravibe_sim::item::GOLD_CHAINMAIL
            | terravibe_sim::item::GOLD_GREAVES
    ) {
        lines.push(TooltipLine {
            text: "Set bonus: +3 defense".to_string(),
            color: Color::srgb_u8(165, 235, 165),
        });
    } else if matches!(
        stack.id,
        terravibe_sim::item::SHADOW_HELMET
            | terravibe_sim::item::SHADOW_SCALEMAIL
            | terravibe_sim::item::SHADOW_GREAVES
    ) {
        lines.push(TooltipLine {
            text: "Set bonus: 15% increased movement speed".to_string(),
            color: Color::srgb_u8(165, 235, 165),
        });
    } else if matches!(
        stack.id,
        terravibe_sim::item::MOLTEN_HELMET
            | terravibe_sim::item::MOLTEN_BREASTPLATE
            | terravibe_sim::item::MOLTEN_GREAVES
    ) {
        lines.push(TooltipLine {
            text: "Set bonus: 10% increased melee damage and immunity to On Fire!".to_string(),
            color: Color::srgb_u8(255, 190, 120),
        });
    }

    if is_material(stack.id) {
        lines.push(TooltipLine {
            text: "Material".to_string(),
            color: Color::srgb_u8(180, 220, 255),
        });
    }

    if let Some((label, copper)) = price_override {
        lines.push(TooltipLine {
            text: format!("{label}: {}", format_coins(copper)),
            color: Color::srgb_u8(255, 220, 120),
        });
    } else if data.value > 0 {
        let total = data.value.saturating_mul(stack.count.max(1));
        lines.push(TooltipLine {
            text: format!("Sell: {}", format_coins(total)),
            color: Color::srgb_u8(255, 220, 120),
        });
    }

    lines
}

/// Clamps tooltip top-left position inside `[0, win_size - box_size]`.
pub fn clamp_tooltip_pos(cursor: Vec2, box_size: Vec2, win_size: Vec2) -> Vec2 {
    let max_x = (win_size.x - box_size.x).max(0.0);
    let max_y = (win_size.y - box_size.y).max(0.0);
    Vec2::new(
        (cursor.x + 16.0).clamp(0.0, max_x),
        (cursor.y + 16.0).clamp(0.0, max_y),
    )
}

/// Tracks hover target stability for a 0.2s tooltip reveal delay.
#[derive(Resource, Default, Clone, Debug)]
pub struct HoverDelay {
    target: Option<u64>,
    elapsed: f32,
}

impl HoverDelay {
    pub const DELAY_SECS: f32 = 0.2;

    #[cfg(test)]
    pub fn update(&mut self, target: Option<u64>, dt: f32) -> bool {
        self.update_with_delay(target, dt, Self::DELAY_SECS)
    }

    pub fn update_with_delay(&mut self, target: Option<u64>, dt: f32, delay_secs: f32) -> bool {
        if target != self.target {
            self.target = target;
            self.elapsed = 0.0;
        }
        if self.target.is_none() {
            return false;
        }
        self.elapsed += dt;
        self.elapsed >= delay_secs.max(0.0)
    }
}

/// Hovered item submitted by any HUD surface during the current frame.
#[derive(Clone, Debug)]
pub struct TooltipCandidate {
    pub target_id: u64,
    pub stack: ItemStack,
    pub equipped: Option<ItemId>,
    pub price_override: Option<(&'static str, u32)>,
}

/// Shared per-frame tooltip request and hover-delay state.
#[derive(Resource, Default)]
pub struct TooltipOverlay {
    pub delay: HoverDelay,
    pub candidate: Option<TooltipCandidate>,
}

#[derive(Component)]
pub struct TooltipRoot;

#[derive(Component)]
pub struct TooltipLineNode(pub usize);

const MAX_TOOLTIP_LINES: usize = 16;
const PAD_X: f32 = 10.0;
const PAD_Y: f32 = 8.0;

pub fn setup(mut commands: Commands) {
    commands
        .spawn((
            TooltipRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(0.0),
                height: Val::Px(0.0),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_BTN)),
                ..default()
            },
            BackgroundColor(super::style::PANEL_BG_SOLID),
            BorderColor::all(super::style::PANEL_BORDER),
            Visibility::Hidden,
            GlobalZIndex(610),
            DespawnOnExit(AppState::InGame),
        ))
        .with_children(|p| {
            for i in 0..MAX_TOOLTIP_LINES {
                p.spawn((
                    TooltipLineNode(i),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(PAD_X),
                        top: Val::Px(PAD_Y),
                        ..default()
                    },
                    HudText {
                        text: String::new(),
                        death: false,
                    },
                    HudTextColor(Color::WHITE),
                ));
            }
        });
}

/// Clears last frame's candidate before surface update systems run.
pub fn begin_frame(mut overlay: ResMut<TooltipOverlay>) {
    overlay.candidate = None;
}

/// Renders the active tooltip if `HoverDelay` has elapsed, clamped to screen bounds.
pub fn render_tooltip(
    time: Res<Time>,
    window: Single<&Window, With<PrimaryWindow>>,
    fonts: Res<HudFonts>,
    settings: Option<Res<crate::settings::Settings>>,
    mut overlay: ResMut<TooltipOverlay>,
    mut root: Query<(&mut Node, &mut Visibility), With<TooltipRoot>>,
    mut line_q: Query<
        (
            &TooltipLineNode,
            &mut Node,
            &mut HudText,
            &mut HudTextColor,
            &mut Visibility,
        ),
        Without<TooltipRoot>,
    >,
) {
    let Ok((mut root_node, mut root_vis)) = root.single_mut() else {
        return;
    };
    let target = overlay.candidate.as_ref().map(|c| c.target_id);
    let delay_secs = settings
        .as_ref()
        .map_or(HoverDelay::DELAY_SECS, |s| s.tooltip_delay);
    let show = overlay
        .delay
        .update_with_delay(target, time.delta_secs(), delay_secs);
    let (Some(cand), Some(cursor)) = (overlay.candidate.as_ref(), window.cursor_position()) else {
        *root_vis = Visibility::Hidden;
        return;
    };
    if !show {
        *root_vis = Visibility::Hidden;
        return;
    }

    let eq_data = cand.equipped.map(item_data);
    let colorblind = settings.as_ref().is_some_and(|s| s.colorblind_palette);
    let lines = format_item_tooltip_for(&cand.stack, eq_data, cand.price_override, colorblind);
    let font = &fonts.mouse.0;
    let line_h = (font.line_spacing as f32).max(18.0);
    let count = lines.len().min(MAX_TOOLTIP_LINES);
    let max_w = lines
        .iter()
        .take(count)
        .map(|l| measure(font, &l.text))
        .fold(80.0_f32, f32::max);
    let box_size = Vec2::new(max_w + PAD_X * 2.0, count as f32 * line_h + PAD_Y * 2.0);
    let win_size = Vec2::new(window.width(), window.height());
    let pos = clamp_tooltip_pos(cursor, box_size, win_size);

    root_node.left = Val::Px(pos.x);
    root_node.top = Val::Px(pos.y);
    root_node.width = Val::Px(box_size.x);
    root_node.height = Val::Px(box_size.y);
    *root_vis = Visibility::Inherited;

    for (idx, mut node, mut text, mut color, mut vis) in &mut line_q {
        if let Some(line) = lines.get(idx.0) {
            node.left = Val::Px(PAD_X);
            node.top = Val::Px(PAD_Y + idx.0 as f32 * line_h);
            if text.text != line.text {
                text.text = line.text.clone();
            }
            if color.0 != line.color {
                color.0 = line.color;
            }
            *vis = Visibility::Inherited;
        } else {
            if !text.text.is_empty() {
                text.text.clear();
            }
            *vis = Visibility::Hidden;
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use bevy::prelude::Vec2;
    use std::collections::HashSet;
    use terravibe_sim::inventory::ItemStack;
    use terravibe_sim::item::{COPPER_SHORTSWORD, Prefix, WOOD, item_data};

    #[test]
    fn rarity_colors_are_distinct() {
        assert_eq!(rarity_color(Rarity::White), Color::WHITE);
        assert_ne!(rarity_color(Rarity::Blue), rarity_color(Rarity::Green));
        assert_ne!(rarity_color(Rarity::Green), rarity_color(Rarity::Orange));
        assert_ne!(rarity_color(Rarity::Blue), rarity_color(Rarity::Orange));

        let rarities = [
            Rarity::Gray,
            Rarity::White,
            Rarity::Blue,
            Rarity::Green,
            Rarity::Orange,
            Rarity::LightRed,
            Rarity::Pink,
            Rarity::LightPurple,
            Rarity::Lime,
            Rarity::Yellow,
            Rarity::Cyan,
            Rarity::Red,
            Rarity::Purple,
        ];
        let mut set = HashSet::new();
        for &r in &rarities {
            let col = rarity_color(r);
            let s = format!("{:?}", col);
            assert!(set.insert(s), "Duplicate color found for rarity {:?}", r);
        }
        assert_eq!(set.len(), 13);
    }

    #[test]
    fn tooltip_includes_name_class_stats_effect_material_and_sell_value() {
        let sword = ItemStack {
            id: COPPER_SHORTSWORD,
            count: 1,
            prefix: Prefix(0),
        };
        let lines = format_item_tooltip(&sword, None, None);
        assert!(!lines.is_empty());
        assert_eq!(
            lines[0].color,
            rarity_color(item_data(COPPER_SHORTSWORD).rarity)
        );
        assert!(lines[0].text.contains("Copper Shortsword"));
        let joined: String = lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(joined.contains("Melee"), "missing class in:\n{joined}");
        assert!(joined.contains("damage"), "missing damage in:\n{joined}");
        assert!(
            joined.contains("critical strike"),
            "missing crit in:\n{joined}"
        );
        assert!(
            joined.contains("knockback"),
            "missing knockback in:\n{joined}"
        );
        assert!(
            joined.contains("speed") || joined.contains("Use time"),
            "missing use time in:\n{joined}"
        );
        assert!(
            joined.contains(item_data(COPPER_SHORTSWORD).tooltip),
            "missing effect tooltip in:\n{joined}"
        );
        assert!(joined.contains("Sell:"), "missing sell value in:\n{joined}");

        let wood = ItemStack {
            id: WOOD,
            count: 10,
            prefix: Prefix(0),
        };
        let wood_lines = format_item_tooltip(&wood, None, None);
        let wood_joined: String = wood_lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            wood_joined.contains("Material"),
            "missing Material in:\n{wood_joined}"
        );
    }

    #[test]
    fn tooltip_clamps_to_1080p_screen_edges() {
        let pos = clamp_tooltip_pos(
            Vec2::new(1900.0, 1060.0),
            Vec2::new(260.0, 180.0),
            Vec2::new(1920.0, 1080.0),
        );
        assert!(
            pos.x >= 0.0 && pos.x <= 1920.0 - 260.0,
            "x out of bounds: {}",
            pos.x
        );
        assert!(
            pos.y >= 0.0 && pos.y <= 1080.0 - 180.0,
            "y out of bounds: {}",
            pos.y
        );
    }

    #[test]
    fn hover_delay_waits_before_showing_and_resets_on_target_change() {
        let mut delay = HoverDelay::default();
        assert!(!delay.update(Some(1), 0.1));
        assert!(delay.update(Some(1), 0.15));
        assert!(!delay.update(Some(2), 0.05));
    }

    #[test]
    fn shop_tooltip_shows_equipped_comparison_and_buy_price() {
        use terravibe_sim::item::{COPPER_HELMET, IRON_HELMET};
        let shop_item = ItemStack {
            id: IRON_HELMET,
            count: 1,
            prefix: Prefix(0),
        };
        let eq = item_data(COPPER_HELMET);
        let lines = format_item_tooltip(&shop_item, Some(eq), Some(("Buy", 500)));
        let joined: String = lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            joined.contains("Equipped: Copper Helmet"),
            "missing equipped in:\n{joined}"
        );
        assert!(
            joined.contains("+1 defense"),
            "missing defense delta in:\n{joined}"
        );
        assert!(
            joined.contains("Buy: 5s"),
            "missing Buy price in:\n{joined}"
        );
    }

    #[test]
    fn colorblind_rarity_palette_differs_from_default() {
        let rarities = [
            Rarity::Gray,
            Rarity::White,
            Rarity::Blue,
            Rarity::Green,
            Rarity::Orange,
            Rarity::LightRed,
            Rarity::Pink,
            Rarity::LightPurple,
            Rarity::Lime,
            Rarity::Yellow,
            Rarity::Cyan,
            Rarity::Red,
            Rarity::Purple,
        ];
        let mut set = std::collections::HashSet::new();
        let mut diff_count = 0;
        for r in rarities {
            let def = rarity_color_for(r, false);
            let cb = rarity_color_for(r, true);
            assert_eq!(def, rarity_color(r));
            if cb != def {
                diff_count += 1;
            }
            let srgba = cb.to_srgba();
            let key = format!("{:.3},{:.3},{:.3}", srgba.red, srgba.green, srgba.blue);
            assert!(set.insert(key), "Duplicate colorblind color for {r:?}");
        }
        assert!(
            diff_count >= 8,
            "expected colorblind palette to differ from default"
        );
    }

    #[test]
    fn armor_set_bonus_lines_and_all_141_items_format_cleanly() {
        use terravibe_sim::item::{
            MOLTEN_BREASTPLATE, MOLTEN_GREAVES, MOLTEN_HELMET, Prefix, SHADOW_GREAVES,
            SHADOW_HELMET, SHADOW_SCALEMAIL, all_item_ids,
        };
        for id in [SHADOW_HELMET, SHADOW_SCALEMAIL, SHADOW_GREAVES] {
            let lines = format_item_tooltip(
                &ItemStack {
                    id,
                    count: 1,
                    prefix: Prefix(0),
                },
                None,
                None,
            );
            assert!(
                lines
                    .iter()
                    .any(|l| l.text.contains("Set bonus: 15% increased movement speed")),
                "Shadow armor piece {id:?} must display set bonus"
            );
        }
        for id in [MOLTEN_HELMET, MOLTEN_BREASTPLATE, MOLTEN_GREAVES] {
            let lines = format_item_tooltip(
                &ItemStack {
                    id,
                    count: 1,
                    prefix: Prefix(0),
                },
                None,
                None,
            );
            assert!(
                lines.iter().any(|l| l
                    .text
                    .contains("Set bonus: 10% increased melee damage and immunity to On Fire!")),
                "Molten armor piece {id:?} must display set bonus"
            );
        }
        let mut count = 0;
        for id in all_item_ids() {
            count += 1;
            let lines = format_item_tooltip(
                &ItemStack {
                    id,
                    count: 1,
                    prefix: Prefix(0),
                },
                None,
                None,
            );
            assert!(
                lines.len() >= 2,
                "item {id:?} must have title and class lines"
            );
        }
        assert_eq!(count, 221);
    }

    #[test]
    fn tooltip_scales_axe_power_and_applies_prefixes() {
        use terravibe_sim::item::{COPPER_AXE, COPPER_BROADSWORD, MOLTEN_HAMAXE};
        let copper_axe = ItemStack {
            id: COPPER_AXE,
            count: 1,
            prefix: Prefix(0),
        };
        let axe_text: String = format_item_tooltip(&copper_axe, None, None)
            .into_iter()
            .map(|l| l.text)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(axe_text.contains("35% axe power"), "{axe_text}");

        let hamaxe = ItemStack {
            id: MOLTEN_HAMAXE,
            count: 1,
            prefix: Prefix(0),
        };
        let hamaxe_text: String = format_item_tooltip(&hamaxe, None, None)
            .into_iter()
            .map(|l| l.text)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(hamaxe_text.contains("150% axe power"), "{hamaxe_text}");

        let legendary = ItemStack {
            id: COPPER_BROADSWORD,
            count: 1,
            prefix: Prefix(81),
        };
        let leg_text: String = format_item_tooltip(&legendary, None, None)
            .into_iter()
            .map(|l| l.text)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            leg_text.contains("Legendary Copper Broadsword"),
            "{leg_text}"
        );
        assert!(leg_text.contains("10 damage"), "{leg_text}");
        assert!(leg_text.contains("9% critical strike chance"), "{leg_text}");
        assert!(leg_text.contains("+15% damage"), "{leg_text}");
    }
}
