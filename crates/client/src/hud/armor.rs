//! Armor HUD: head/body/legs slots under the hotbar and the total defense.
//! The server owns equipment (`ServerMsg::Armor`); `E` asks it to equip the selected hotbar item.
#![allow(dead_code)]

use bevy::prelude::*;
use terravibe_sim::item::{ItemId, Prefix, accessory_prefix_bonus, item_data};
use terravibe_sim::net::ClientMsg;

use super::hotbar::{HotbarAssets, icon_of};
use super::inventory::fit_icon;
use super::text::HudText;
use crate::net::{LocalAccessories, LocalAccessoryPrefixes, LocalArmor, LocalInventory, Net};
use crate::session::AppState;

const SLOT: f32 = 44.2; // same size as hotbar slots (52 * 0.85)
const PITCH: f32 = 47.6;
const ORIGIN: [f32; 2] = [20.0, 70.0];

/// `[x, y, size]` of armor slot `i` (head, body, legs) at UI scale 1.
pub fn slot_rect(i: usize) -> [f32; 3] {
    [ORIGIN[0] + i as f32 * PITCH, ORIGIN[1], SLOT]
}

/// Left edge of the defense text, right of the last slot.
pub const DEFENSE_X: f32 = ORIGIN[0] + 3.0 * PITCH + 6.0;

/// Sum of `Item.defense` over worn pieces plus set bonuses.
pub fn total_defense(p: &[Option<ItemId>; 3]) -> i32 {
    let base: i32 = p.iter().flatten().map(|i| item_data(*i).defense).sum();
    base + terravibe_sim::player::armor_set_defense_bonus(p)
}

/// Total defense including worn armor, set bonuses, and accessories.
pub fn total_defense_with_accessories(p: &[Option<ItemId>; 3], acc: &[Option<ItemId>; 5]) -> i32 {
    let acc_def: i32 = acc.iter().flatten().map(|i| item_data(*i).defense).sum();
    total_defense(p) + acc_def
}

/// Total defense including worn armor, set bonuses, accessories, and accessory prefixes.
pub fn total_defense_with_accessory_prefixes(
    p: &[Option<ItemId>; 3],
    acc: &[Option<ItemId>; 5],
    prefixes: &[Prefix; 5],
) -> i32 {
    let prefix_def: i32 = acc
        .iter()
        .zip(prefixes.iter())
        .filter_map(|(id, pref)| id.map(|_| accessory_prefix_bonus(*pref).defense))
        .sum();
    total_defense_with_accessories(p, acc) + prefix_def
}

#[derive(Component)]
pub struct ArmorBack;
#[derive(Component)]
pub struct ArmorIcon(usize);
#[derive(Component)]
pub struct DefenseText;

pub fn setup(mut commands: Commands, assets: Res<HotbarAssets>) {
    let abs = |x: f32, y: f32, w: f32, h: f32| Node {
        position_type: PositionType::Absolute,
        left: Val::Px(x),
        top: Val::Px(y),
        width: Val::Px(w),
        height: Val::Px(h),
        ..default()
    };
    for i in 0..3 {
        let [x, y, s] = slot_rect(i);
        commands.spawn((
            ArmorBack,
            abs(x, y, s, s),
            ImageNode::new(assets.back.clone()),
            GlobalZIndex(10),
            DespawnOnExit(AppState::InGame),
        ));
        commands.spawn((
            ArmorIcon(i),
            abs(x, y, 0.0, 0.0),
            ImageNode::new(assets.back.clone()),
            GlobalZIndex(20),
            Visibility::Hidden,
            DespawnOnExit(AppState::InGame),
        ));
    }
    commands.spawn((
        DefenseText,
        HudText {
            text: String::new(),
            death: false,
        },
        abs(DEFENSE_X, ORIGIN[1] + 12.0, 0.0, 0.0),
        GlobalZIndex(30),
        DespawnOnExit(AppState::InGame),
    ));
}

pub fn update(
    armor: Option<Res<LocalArmor>>,
    accessories: Option<Res<LocalAccessories>>,
    acc_prefixes: Option<Res<LocalAccessoryPrefixes>>,
    mut assets: ResMut<HotbarAssets>,
    mut image_assets: ResMut<Assets<Image>>,
    mut icons: Query<(&ArmorIcon, &mut Node, &mut ImageNode, &mut Visibility)>,
    mut text: Single<&mut HudText, With<DefenseText>>,
) {
    let pieces = armor.map_or([None; 3], |a| a.0);
    for (ic, mut node, mut img, mut vis) in &mut icons {
        let Some(item) = pieces[ic.0] else {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
            continue;
        };
        let (handle, size) = icon_of(&mut assets, &mut image_assets, item);
        let [x, y, s] = slot_rect(ic.0);
        let d = fit_icon(size, 0.85);
        node.left = Val::Px((x + (s - d.x) / 2.0).round());
        node.top = Val::Px((y + (s - d.y) / 2.0).round());
        node.width = Val::Px(d.x);
        node.height = Val::Px(d.y);
        if img.image != handle {
            img.image = handle;
        }
        if *vis != Visibility::Inherited {
            *vis = Visibility::Inherited;
        }
    }
    let acc = accessories.map_or([None; 5], |a| a.0);
    let prefixes = acc_prefixes.map_or([Prefix(0); 5], |p| p.0);
    let want = format!(
        "Defense: {}",
        total_defense_with_accessory_prefixes(&pieces, &acc, &prefixes)
    );
    if text.text != want {
        text.text = want;
    }
}

/// `E` (or gamepad DPadDown) equips the selected hotbar item when it is armor; the server validates and answers.
pub fn equip_input(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    inv: Option<Res<LocalInventory>>,
    net: Option<Res<Net>>,
    settings: Option<Res<crate::settings::Settings>>,
) {
    let equip_key = settings.as_deref().map_or(KeyCode::KeyE, |s| {
        s.bindings.key(crate::input_map::GameAction::EquipArmor)
    });
    let gamepad_pressed = gamepads
        .iter()
        .any(|g| g.just_pressed(GamepadButton::DPadDown));
    if !keys.just_pressed(equip_key) && !gamepad_pressed {
        return;
    }
    let (Some(inv), Some(net)) = (inv, net) else {
        return;
    };
    if let Some(s) = inv.0.selected_stack()
        && item_data(s.id).armor_slot.is_some()
    {
        let _ = net.send(ClientMsg::Equip {
            from: inv.0.selected as u8,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terravibe_sim::item::{GOLD_CHAINMAIL, IRON_HELMET};

    #[test]
    fn armor_hud_layout_fits_1280x720() {
        for i in 0..3 {
            let [x, y, s] = slot_rect(i);
            assert!(x >= 0.0 && y >= 0.0 && x + s <= 1280.0 && y + s <= 720.0);
        }
        // Below the hotbar row (y 20..64.2), and clear of the hearts column at x 500+.
        assert!(slot_rect(0)[1] > 20.0 + 44.2);
        assert!(DEFENSE_X + 120.0 < 500.0);
    }

    #[test]
    fn armor_hud_total_defense() {
        assert_eq!(total_defense(&[None; 3]), 0);
        assert_eq!(
            total_defense(&[Some(IRON_HELMET), Some(GOLD_CHAINMAIL), None]),
            7
        );
    }
}
