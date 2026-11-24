//! Terraria's hotbar: ten slots top-left, item icons, stack counts and the selected name.
//!
//! Positions follow `Main.DrawInventory` (gameplay spec): slot `i` of row 0 is drawn at
//! `x = 20 + i * 56 * inventoryScale`, `y = 20`, with `Main.inventoryScale = 0.85` and a 52 px
//! `Inventory_Back` texture. The selected item's name is drawn by the same method with
//! `MouseText` at `(236 - width / 2, 0)`. Icons follow `ItemSlot.Draw`: shrunk to fit 32 px,
//! then multiplied by the slot scale and centred in the slot. Stack counts sit at
//! `(slot.x + 10 * scale, slot.y + 26 * scale)`.

use std::collections::HashMap;

use bevy::prelude::*;
use terravibe_assets::RgbaImage;
use terravibe_sim::inventory::ItemStack;
use terravibe_sim::item::{ItemId, display_name};

use super::HudFonts;
use super::inventory::{InventoryOpen, fit_icon};
use super::text::{HudText, HudTextColor, StackText, measure};
use crate::net::LocalInventory;
use crate::session::AppState;
use crate::view::to_bevy_image;

pub(super) const SLOT_SCALE: f32 = 0.85;
pub(super) const SLOT_TEX: f32 = 52.0;
pub(super) const SLOT_PITCH: f32 = 56.0;
pub(super) const ORIGIN: f32 = 20.0;
const NAME_CENTRE_X: f32 = 236.0;

/// Cache key for the generic icon shown when an item has no texture at all.
const FALLBACK_ICON: u16 = u16::MAX;

/// `Item.AffixName` for the selected stack: empty when the slot is empty.
fn stack_name(stack: Option<&ItemStack>) -> String {
    stack.map_or_else(String::new, |s| display_name(s.id, s.prefix))
}

/// Cycles a hotbar slot index (`0..10`) by `delta` with wrap-around.
pub(crate) fn cycle_slot(slot: u8, delta: i32) -> u8 {
    (slot as i32 + delta).rem_euclid(10) as u8
}

/// Returns `-1` for LB (`LeftTrigger`), `+1` for RB (`RightTrigger`), or `0` otherwise.
pub(crate) fn gamepad_hotbar_delta(gamepad: &Gamepad) -> i32 {
    let mut delta = 0;
    if gamepad.just_pressed(GamepadButton::LeftTrigger) {
        delta -= 1;
    }
    if gamepad.just_pressed(GamepadButton::RightTrigger) {
        delta += 1;
    }
    delta
}

/// `[x, y, size]` of hotbar slot `i` at UI scale 1.
pub fn slot_rect(i: usize) -> [f32; 3] {
    [
        ORIGIN + i as f32 * SLOT_PITCH * SLOT_SCALE,
        ORIGIN,
        SLOT_TEX * SLOT_SCALE,
    ]
}

/// CPU images handed over by `ViewPlugin`.
#[derive(Resource)]
pub struct HotbarSource {
    pub back: RgbaImage,
    pub back_selected: RgbaImage,
    pub icons: HashMap<u16, RgbaImage>,
}

#[derive(Resource)]
pub struct HotbarAssets {
    pub(super) back: Handle<Image>,
    pub back_selected: Handle<Image>,
    pub(super) icons: HashMap<u16, (Handle<Image>, Vec2)>,
}

/// Native-size RGBA fallback used when an item icon is missing from `HotbarAssets`.
fn fallback_image() -> RgbaImage {
    const S: u32 = 16;
    let mut pixels = vec![[0; 4]; (S * S) as usize];
    for y in 0..S {
        for x in 0..S {
            let edge = x == 0 || y == 0 || x == S - 1 || y == S - 1;
            let checker = (x / 4 + y / 4) % 2 == 0;
            pixels[(y * S + x) as usize] = if edge {
                [235, 235, 245, 255]
            } else if checker {
                [90, 90, 115, 255]
            } else {
                [45, 45, 65, 255]
            };
        }
    }
    RgbaImage {
        width: S,
        height: S,
        pixels,
    }
}

fn fallback_icon(assets: &mut HotbarAssets, images: &mut Assets<Image>) -> (Handle<Image>, Vec2) {
    if let Some((handle, size)) = assets.icons.get(&FALLBACK_ICON) {
        return (handle.clone(), *size);
    }
    let img = fallback_image();
    let size = Vec2::new(img.width as f32, img.height as f32);
    let handle = images.add(to_bevy_image(&img));
    assets.icons.insert(FALLBACK_ICON, (handle.clone(), size));
    (handle, size)
}

/// Icon handle and native pixel size for `item`.
///
/// Icons are cached in `HotbarAssets`. If an icon was not preloaded, a generic 16x16 icon
/// is returned and cached so every item always has a visible icon.
pub(crate) fn icon_of(
    assets: &mut HotbarAssets,
    images: &mut Assets<Image>,
    item: ItemId,
) -> (Handle<Image>, Vec2) {
    if let Some((handle, size)) = assets.icons.get(&item.0) {
        return (handle.clone(), *size);
    }
    let (handle, size) = fallback_icon(assets, images);
    assets.icons.insert(item.0, (handle.clone(), size));
    (handle, size)
}

#[derive(Component)]
pub struct SlotBack(usize);
#[derive(Component)]
pub struct SlotIcon(usize);
#[derive(Component)]
pub struct SlotCount(usize);
#[derive(Component)]
pub struct SlotKeybind;
#[derive(Component)]
pub struct SelectedName;

pub fn setup(mut commands: Commands, src: Res<HotbarSource>, mut images: ResMut<Assets<Image>>) {
    let assets = HotbarAssets {
        back: images.add(to_bevy_image(&src.back)),
        back_selected: images.add(to_bevy_image(&src.back_selected)),
        icons: src
            .icons
            .iter()
            .map(|(id, img)| {
                let size = Vec2::new(img.width as f32, img.height as f32);
                (*id, (images.add(to_bevy_image(img)), size))
            })
            .collect(),
    };
    let abs = |x: f32, y: f32, w: f32, h: f32| Node {
        position_type: PositionType::Absolute,
        left: Val::Px(x),
        top: Val::Px(y),
        width: Val::Px(w),
        height: Val::Px(h),
        ..default()
    };
    for i in 0..10 {
        let [x, y, s] = slot_rect(i);
        commands.spawn((
            SlotBack(i),
            abs(x, y, s, s),
            ImageNode::new(assets.back.clone()),
            GlobalZIndex(10),
            DespawnOnExit(AppState::InGame),
        ));
        commands.spawn((
            SlotIcon(i),
            abs(x, y, 0.0, 0.0),
            ImageNode::new(assets.back.clone()),
            GlobalZIndex(20),
            Visibility::Hidden,
            DespawnOnExit(AppState::InGame),
        ));
        commands.spawn((
            SlotCount(i),
            StackText,
            HudText {
                text: String::new(),
                death: false,
            },
            abs(x + 10.0 * SLOT_SCALE, y + 26.0 * SLOT_SCALE, 0.0, 0.0),
            GlobalZIndex(30),
            DespawnOnExit(AppState::InGame),
        ));
        let key_label = if i == 9 {
            "0".to_string()
        } else {
            (i + 1).to_string()
        };
        commands.spawn((
            SlotKeybind,
            StackText,
            HudTextColor(super::style::TEXT_PRIMARY),
            HudText {
                text: key_label,
                death: false,
            },
            abs(x + 4.0 * SLOT_SCALE, y + 3.0 * SLOT_SCALE, 0.0, 0.0),
            GlobalZIndex(30),
            DespawnOnExit(AppState::InGame),
        ));
    }
    commands.spawn((
        SelectedName,
        HudTextColor(Color::WHITE),
        HudText {
            text: String::new(),
            death: false,
        },
        abs(NAME_CENTRE_X, 0.0, 0.0, 0.0),
        GlobalZIndex(40),
        DespawnOnExit(AppState::InGame),
    ));
    commands.insert_resource(assets);
}

fn set_vis(v: &mut Visibility, show: bool) {
    let want = if show {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if *v != want {
        *v = want;
    }
}

#[allow(clippy::type_complexity)]
pub fn update(
    mut commands: Commands,
    inv: Option<Res<LocalInventory>>,
    open: Option<Res<InventoryOpen>>,
    settings: Option<Res<crate::settings::Settings>>,
    mut assets: ResMut<HotbarAssets>,
    mut image_assets: ResMut<Assets<Image>>,
    fonts: Res<HudFonts>,
    mut backs: Query<
        (&SlotBack, &mut ImageNode, &mut Visibility),
        (
            Without<SlotIcon>,
            Without<SlotCount>,
            Without<SlotKeybind>,
            Without<SelectedName>,
        ),
    >,
    mut icons: Query<
        (&SlotIcon, &mut Node, &mut ImageNode, &mut Visibility),
        (
            Without<SlotBack>,
            Without<SlotCount>,
            Without<SlotKeybind>,
            Without<SelectedName>,
        ),
    >,
    mut counts: Query<
        (&SlotCount, &mut HudText, &mut Visibility),
        (Without<SlotKeybind>, Without<SelectedName>),
    >,
    mut keybinds: Query<
        &mut Visibility,
        (
            With<SlotKeybind>,
            Without<SlotBack>,
            Without<SlotIcon>,
            Without<SlotCount>,
            Without<SelectedName>,
        ),
    >,
    mut name: Query<
        (
            Entity,
            &mut Node,
            &mut HudText,
            &mut Visibility,
            Option<&mut HudTextColor>,
        ),
        (With<SelectedName>, Without<SlotIcon>, Without<SlotKeybind>),
    >,
) {
    let Some(inv) = inv else { return };
    let inv = &inv.0;
    let open = open.as_ref().is_some_and(|o| o.0);
    for mut vis in &mut keybinds {
        set_vis(&mut vis, !open);
    }
    for (b, mut img, mut vis) in &mut backs {
        set_vis(&mut vis, !open);
        if open {
            continue;
        }
        let want = if b.0 == inv.selected {
            &assets.back_selected
        } else {
            &assets.back
        };
        if img.image != *want {
            img.image = want.clone();
        }
    }
    for (ic, mut node, mut img, mut vis) in &mut icons {
        if open {
            set_vis(&mut vis, false);
            continue;
        }
        let Some(stack) = inv.slots[ic.0] else {
            set_vis(&mut vis, false);
            continue;
        };
        let (handle, size) = icon_of(&mut assets, &mut image_assets, stack.id);
        let [x, y, s] = slot_rect(ic.0);
        let d = fit_icon(size, SLOT_SCALE);
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
    for (c, mut t, mut vis) in &mut counts {
        set_vis(&mut vis, !open);
        if open {
            continue;
        }
        let want = match inv.slots[c.0] {
            Some(s) if s.count > 1 => s.count.to_string(),
            Some(s)
                if terravibe_sim::item::item_data(s.id).class
                    == terravibe_sim::item::ItemClass::Ranged =>
            {
                let ammo: u32 = inv
                    .slots
                    .iter()
                    .flatten()
                    .filter(|a| terravibe_sim::item::matches_ammo(s.id, a.id))
                    .map(|a| a.count)
                    .sum();
                if ammo > 0 {
                    ammo.to_string()
                } else {
                    String::new()
                }
            }
            _ => String::new(),
        };
        if t.text != want {
            t.text = want;
        }
    }
    let colorblind = settings.as_ref().is_some_and(|s| s.colorblind_palette);
    let (want, want_color) = match inv.selected_stack() {
        Some(s) => (
            stack_name(Some(s)),
            super::tooltip::rarity_color_for(
                terravibe_sim::item::item_data(s.id).rarity,
                colorblind,
            ),
        ),
        None => (String::new(), Color::WHITE),
    };
    for (e, mut node, mut t, mut vis, maybe_color) in &mut name {
        set_vis(&mut vis, !open);
        if open {
            continue;
        }
        if t.text != want {
            t.text = want.clone();
        }
        let x = Val::Px(NAME_CENTRE_X - measure(&fonts.mouse.0, &want) / 2.0);
        if node.left != x {
            node.left = x;
        }
        if let Some(mut color) = maybe_color {
            if color.0 != want_color {
                color.0 = want_color;
            }
        } else {
            commands.entity(e).insert(HudTextColor(want_color));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn hotbar_slot_rects() {
        // Main.DrawInventory: x = 20 + i * 56 * 0.85, y = 20, 52 * 0.85 = 44.2 px.
        let near = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).all(|(p, q)| (p - q).abs() < 1e-3);
        assert!(near(slot_rect(0), [20.0, 20.0, 44.2]));
        assert!(near(slot_rect(9), [448.4, 20.0, 44.2]));
    }

    #[test]
    fn icon_of_uses_cache_and_falls_back_for_missing_textures() {
        let mut images = Assets::<Image>::default();
        let mut assets = HotbarAssets {
            back: Handle::default(),
            back_selected: Handle::default(),
            icons: HashMap::new(),
        };
        // No preloaded icon: a generic 16x16 icon is cached.
        let item = ItemId(65_000);
        let (handle, size) = icon_of(&mut assets, &mut images, item);
        assert_eq!(size, Vec2::splat(16.0));
        assert!(images.get(&handle).is_some());
        assert!(assets.icons.contains_key(&FALLBACK_ICON));
        // Asking again reuses the same cached fallback (no duplicate image).
        let (again, again_size) = icon_of(&mut assets, &mut images, item);
        assert_eq!(handle, again);
        assert_eq!(size, again_size);
        assert_eq!(images.len(), 1);

        // A preloaded icon is returned directly.
        let preloaded = images.add(Image::default());
        assets
            .icons
            .insert(item.0, (preloaded.clone(), Vec2::new(8.0, 8.0)));
        let (cached, cached_size) = icon_of(&mut assets, &mut images, item);
        assert_eq!(cached, preloaded);
        assert_eq!(cached_size, Vec2::new(8.0, 8.0));
    }

    #[test]
    fn hotbar_z_indices_prevent_background_obscuring_icons() {
        let mut app = App::new();
        let handle = Handle::<Image>::default();
        let font = Arc::new(dummy_font());

        app.insert_resource(HotbarSource {
            back: terravibe_assets::RgbaImage {
                width: 52,
                height: 52,
                pixels: vec![[0; 4]; 52 * 52],
            },
            back_selected: terravibe_assets::RgbaImage {
                width: 52,
                height: 52,
                pixels: vec![[0; 4]; 52 * 52],
            },
            icons: HashMap::new(),
        });
        app.init_resource::<Assets<Image>>();
        app.insert_resource(HudFonts {
            mouse: (font.clone(), handle.clone()),
            death: (font.clone(), handle.clone()),
            stack: (font, handle),
        });

        app.add_systems(Startup, setup);
        app.update();

        let mut backs = app.world_mut().query::<(&SlotBack, &GlobalZIndex)>();
        let mut icons = app.world_mut().query::<(&SlotIcon, &GlobalZIndex)>();
        let mut counts = app.world_mut().query::<(&SlotCount, &GlobalZIndex)>();

        for (_, back_z) in backs.iter(app.world()) {
            assert_eq!(back_z.0, 10);
        }
        for (_, icon_z) in icons.iter(app.world()) {
            assert_eq!(icon_z.0, 20);
        }
        for (_, count_z) in counts.iter(app.world()) {
            assert_eq!(count_z.0, 30);
        }
    }

    #[test]
    fn selected_name_includes_prefix() {
        use terravibe_sim::item::{COPPER_PICKAXE, COPPER_SHORTSWORD, Prefix};

        let stack = |id, prefix| ItemStack {
            id,
            count: 1,
            prefix,
        };
        assert_eq!(stack_name(None), "");
        // Item.AffixName falls back to the plain item name for Prefix(0).
        assert_eq!(
            stack_name(Some(&stack(COPPER_PICKAXE, Prefix(0)))),
            "Copper Pickaxe"
        );
        assert_eq!(
            stack_name(Some(&stack(COPPER_SHORTSWORD, Prefix(81)))),
            "Legendary Copper Shortsword"
        );
    }

    fn dummy_font() -> terravibe_assets::SpriteFont {
        terravibe_assets::SpriteFont {
            atlas: RgbaImage {
                width: 1,
                height: 1,
                pixels: vec![[0; 4]],
            },
            glyphs: Vec::new(),
            line_spacing: 10,
            spacing: 0.0,
            default_char: None,
        }
    }

    #[test]
    fn hotbar_hides_when_inventory_open() {
        use std::sync::Arc;
        use terravibe_sim::inventory::Inventory;

        let mut app = App::new();
        let handle = Handle::<Image>::default();
        let font = Arc::new(dummy_font());
        app.insert_resource(LocalInventory(Inventory::empty()));
        app.insert_resource(InventoryOpen(false));
        app.insert_resource(Assets::<Image>::default());
        app.insert_resource(HotbarAssets {
            back: handle.clone(),
            back_selected: handle.clone(),
            icons: HashMap::new(),
        });
        app.insert_resource(HudFonts {
            mouse: (font.clone(), handle.clone()),
            death: (font.clone(), handle.clone()),
            stack: (font, handle.clone()),
        });
        let back = app
            .world_mut()
            .spawn((
                SlotBack(0),
                ImageNode::new(handle.clone()),
                Visibility::Inherited,
            ))
            .id();
        let icon = app
            .world_mut()
            .spawn((
                SlotIcon(0),
                Node::default(),
                ImageNode::new(handle.clone()),
                Visibility::Inherited,
            ))
            .id();
        let count = app
            .world_mut()
            .spawn((
                SlotCount(0),
                HudText {
                    text: String::new(),
                    death: false,
                },
                Visibility::Inherited,
            ))
            .id();
        let name = app
            .world_mut()
            .spawn((
                SelectedName,
                Node::default(),
                HudText {
                    text: String::new(),
                    death: false,
                },
                Visibility::Inherited,
            ))
            .id();
        app.add_systems(Update, update);

        app.update();
        for e in [back, count, name] {
            assert_eq!(
                *app.world().get::<Visibility>(e).unwrap(),
                Visibility::Inherited
            );
        }

        app.world_mut().resource_mut::<InventoryOpen>().0 = true;
        app.update();
        for e in [back, icon, count, name] {
            assert_eq!(
                *app.world().get::<Visibility>(e).unwrap(),
                Visibility::Hidden
            );
        }

        app.world_mut().resource_mut::<InventoryOpen>().0 = false;
        app.update();
        for e in [back, count, name] {
            assert_eq!(
                *app.world().get::<Visibility>(e).unwrap(),
                Visibility::Inherited
            );
        }
        // The empty slot still has no icon.
        assert_eq!(
            *app.world().get::<Visibility>(icon).unwrap(),
            Visibility::Hidden
        );
    }

    #[test]
    fn hotbar_shows_ammo_count_when_holding_bow() {
        use std::sync::Arc;
        use terravibe_sim::inventory::{AMMO0, Inventory, ItemStack};
        use terravibe_sim::item::{FLAMING_ARROW, Prefix, WOODEN_BOW};

        let mut app = App::new();
        let handle = Handle::<Image>::default();
        let font = Arc::new(dummy_font());
        let mut inv = Inventory::empty();
        inv.slots[0] = Some(ItemStack {
            id: WOODEN_BOW,
            count: 1,
            prefix: Prefix(0),
        });
        inv.slots[AMMO0] = Some(ItemStack {
            id: FLAMING_ARROW,
            count: 73,
            prefix: Prefix(0),
        });
        app.insert_resource(LocalInventory(inv));
        app.insert_resource(InventoryOpen(false));
        app.insert_resource(Assets::<Image>::default());
        app.insert_resource(HotbarAssets {
            back: handle.clone(),
            back_selected: handle.clone(),
            icons: HashMap::new(),
        });
        app.insert_resource(HudFonts {
            mouse: (font.clone(), handle.clone()),
            death: (font.clone(), handle.clone()),
            stack: (font, handle.clone()),
        });
        let count = app
            .world_mut()
            .spawn((
                SlotCount(0),
                HudText {
                    text: String::new(),
                    death: false,
                },
                Visibility::Inherited,
            ))
            .id();
        app.add_systems(Update, update);
        app.update();
        assert_eq!(app.world().get::<HudText>(count).unwrap().text, "73");
    }

    #[test]
    fn selected_name_uses_item_rarity_color() {
        use std::sync::Arc;
        use terravibe_sim::inventory::{Inventory, ItemStack};
        use terravibe_sim::item::{DEMONITE_ORE_ITEM, Prefix, Rarity};

        let mut app = App::new();
        let handle = Handle::<Image>::default();
        let font = Arc::new(dummy_font());
        let mut inv = Inventory::empty();
        inv.slots[0] = Some(ItemStack {
            id: DEMONITE_ORE_ITEM,
            count: 1,
            prefix: Prefix(0),
        });
        inv.selected = 0;
        app.insert_resource(LocalInventory(inv));
        app.insert_resource(InventoryOpen(false));
        app.insert_resource(Assets::<Image>::default());
        app.insert_resource(HotbarAssets {
            back: handle.clone(),
            back_selected: handle.clone(),
            icons: HashMap::new(),
        });
        app.insert_resource(HudFonts {
            mouse: (font.clone(), handle.clone()),
            death: (font.clone(), handle.clone()),
            stack: (font, handle.clone()),
        });
        let name = app
            .world_mut()
            .spawn((
                SelectedName,
                HudTextColor(Color::WHITE),
                Node::default(),
                HudText {
                    text: String::new(),
                    death: false,
                },
                Visibility::Inherited,
            ))
            .id();
        app.add_systems(Update, update);
        app.update();

        let color = app.world().get::<HudTextColor>(name).unwrap();
        assert_eq!(color.0, super::super::tooltip::rarity_color(Rarity::Blue));
    }

    #[test]
    fn gamepad_hotbar_delta_and_cycle_slot() {
        assert_eq!(cycle_slot(0, -1), 9);
        assert_eq!(cycle_slot(9, 1), 0);
        assert_eq!(cycle_slot(3, 1), 4);
        assert_eq!(cycle_slot(3, -1), 2);

        let mut gp = Gamepad::default();
        assert_eq!(gamepad_hotbar_delta(&gp), 0);

        gp.digital_mut().press(GamepadButton::RightTrigger);
        assert_eq!(gamepad_hotbar_delta(&gp), 1);

        let mut gp_lb = Gamepad::default();
        gp_lb.digital_mut().press(GamepadButton::LeftTrigger);
        assert_eq!(gamepad_hotbar_delta(&gp_lb), -1);
    }

    #[test]
    fn ranged_hotbar_ammo_count_matches_weapon_type() {
        use std::sync::Arc;
        use terravibe_sim::inventory::{Inventory, ItemStack};
        use terravibe_sim::item::{MUSKET, MUSKET_BALL, Prefix, WOODEN_ARROW, WOODEN_BOW};

        let mut app = App::new();
        let handle = Handle::<Image>::default();
        let font = Arc::new(dummy_font());
        let mut inv = Inventory::empty();
        // Slot 0: Wooden Bow (uses arrows = 15)
        inv.slots[0] = Some(ItemStack {
            id: WOODEN_BOW,
            count: 1,
            prefix: Prefix(0),
        });
        // Slot 1: Musket (uses bullets = 42)
        inv.slots[1] = Some(ItemStack {
            id: MUSKET,
            count: 1,
            prefix: Prefix(0),
        });
        inv.slots[10] = Some(ItemStack {
            id: WOODEN_ARROW,
            count: 15,
            prefix: Prefix(0),
        });
        inv.slots[11] = Some(ItemStack {
            id: MUSKET_BALL,
            count: 42,
            prefix: Prefix(0),
        });
        app.insert_resource(LocalInventory(inv));
        app.insert_resource(InventoryOpen(false));
        app.insert_resource(Assets::<Image>::default());
        app.insert_resource(HotbarAssets {
            back: handle.clone(),
            back_selected: handle.clone(),
            icons: HashMap::new(),
        });
        app.insert_resource(HudFonts {
            mouse: (font.clone(), handle.clone()),
            death: (font.clone(), handle.clone()),
            stack: (font, handle.clone()),
        });
        let bow_count = app
            .world_mut()
            .spawn((
                SlotCount(0),
                StackText,
                Node::default(),
                HudText {
                    text: String::new(),
                    death: false,
                },
                Visibility::Inherited,
            ))
            .id();
        let gun_count = app
            .world_mut()
            .spawn((
                SlotCount(1),
                StackText,
                Node::default(),
                HudText {
                    text: String::new(),
                    death: false,
                },
                Visibility::Inherited,
            ))
            .id();
        app.add_systems(Update, update);
        app.update();
        assert_eq!(app.world().get::<HudText>(bow_count).unwrap().text, "15");
        assert_eq!(app.world().get::<HudText>(gun_count).unwrap().text, "42");
    }
}
