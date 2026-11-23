//! Classic Terraria life hearts, mana stars and text, as UI nodes.
//!
//! Offsets and fade/scale rules follow `ClassicPlayerResourcesDisplaySet.DrawLife` / `DrawMana`
//! (Terraria.GameContent.UI.ResourceSets). Terraria's anchor is `screenWidth - 800`, so every
//! x offset below is relative to an 800 px wide box pinned to the right edge.

pub mod armor;
pub mod boss_bar;
pub mod chest;
pub mod combat_feedback;
pub mod crafting;
pub mod death;
pub mod hotbar;
pub mod inventory;
pub mod map;
pub mod onboarding;
pub mod pickup_toast;
pub mod style;
pub mod text;
pub mod tooltip;
pub mod town;
pub mod victory;

use std::sync::Arc;

use bevy::prelude::*;
use terravibe_assets::{RgbaImage, SpriteFont};

use crate::session::AppState;
use crate::view::player::LocalPlayer;
use crate::view::to_bevy_image;
use text::{HudText, StackText, measure, rebuild_text};

const PER_ICON: i32 = 20;
const MAX_HEARTS: usize = 20;
const MAX_STARS: usize = 10;

/// The Mouse_Text font and its atlas image.
#[derive(Resource)]
#[allow(dead_code)] // reused by the hotbar in a later project
pub struct HudFont(pub Arc<SpriteFont>, pub Handle<Image>);

#[derive(Resource)]
pub struct HudFonts {
    pub mouse: (Arc<SpriteFont>, Handle<Image>),
    pub death: (Arc<SpriteFont>, Handle<Image>),
    pub stack: (Arc<SpriteFont>, Handle<Image>),
}

#[derive(Default, Clone)]
pub struct ExtraUiSource {
    pub coin_ghosts: [Option<RgbaImage>; 4],
    pub ammo_ghost: Option<RgbaImage>,
    pub trash_ghost: Option<RgbaImage>,
    pub armor_head_ghost: Option<RgbaImage>,
    pub armor_body_ghost: Option<RgbaImage>,
    pub armor_legs_ghost: Option<RgbaImage>,
    pub accessory_ghost: Option<RgbaImage>,
    pub hammer_btn: Option<RgbaImage>,
    pub quick_stack_btn: Option<RgbaImage>,
    pub sort_btn: Option<RgbaImage>,
    pub shield_icon: Option<RgbaImage>,
    #[allow(dead_code)]
    pub chat_back: Option<RgbaImage>,
    pub heart_fancy: Option<RgbaImage>,
    pub star_fancy: Option<RgbaImage>,
    pub buff_potion_sickness: Option<RgbaImage>,
}

#[derive(Resource, Default, Clone)]
pub struct ExtraUiAssets {
    pub coin_ghosts: [Option<Handle<Image>>; 4],
    pub ammo_ghost: Option<Handle<Image>>,
    pub trash_ghost: Option<Handle<Image>>,
    pub armor_head_ghost: Option<Handle<Image>>,
    pub armor_body_ghost: Option<Handle<Image>>,
    pub armor_legs_ghost: Option<Handle<Image>>,
    pub accessory_ghost: Option<Handle<Image>>,
    pub hammer_btn: Option<Handle<Image>>,
    pub quick_stack_btn: Option<Handle<Image>>,
    pub sort_btn: Option<Handle<Image>>,
    pub shield_icon: Option<Handle<Image>>,
    #[allow(dead_code)]
    pub chat_back: Option<Handle<Image>>,
    #[allow(dead_code)]
    pub heart_fancy: Option<Handle<Image>>,
    #[allow(dead_code)]
    pub star_fancy: Option<Handle<Image>>,
    pub buff_potion_sickness: Option<Handle<Image>>,
}

/// CPU-side assets handed over by `ViewPlugin`; turned into GPU images at startup.
pub struct HudPlugin {
    pub mouse: Arc<SpriteFont>,
    pub death: Arc<SpriteFont>,
    pub stack: Arc<SpriteFont>,
    pub hotbar: hotbar::HotbarSource,
    pub heart: RgbaImage,
    pub mana: RgbaImage,
    pub extra_ui: ExtraUiSource,
}

#[derive(Resource)]
struct HudSource {
    mouse: Arc<SpriteFont>,
    death: Arc<SpriteFont>,
    stack: Arc<SpriteFont>,
    heart: RgbaImage,
    mana: RgbaImage,
    extra_ui: ExtraUiSource,
}

#[derive(Resource)]
struct IconSize {
    heart: Vec2,
    mana: Vec2,
}

#[derive(Resource)]
pub struct HudImages {
    pub heart: Handle<Image>,
    pub mana: Handle<Image>,
}

#[derive(Component)]
struct HeartIcon(usize);
#[derive(Component)]
struct StarIcon(usize);
#[derive(Component)]
struct LifeLabel;
#[derive(Component)]
struct LifeValue;
#[derive(Component)]
struct ManaLabel;
#[derive(Component)]
struct DeathLabel;
#[derive(Component)]
pub struct PotionSicknessIcon;
#[derive(Component)]
pub struct PotionSicknessText;
#[derive(Component)]
pub struct BuffSlotIcon(pub usize);
#[derive(Component)]
pub struct BuffSlotName(pub usize);
#[derive(Component)]
pub struct BuffSlotText(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuffBadgeKind {
    PotionSickness,
    OnFire,
    ObsidianSkin,
    LavaWard,
    Ironskin,
    Swiftness,
    Regen,
    Shine,
    WaterWalking,
    CozyFire,
    HeartLantern,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BuffBadgeInfo {
    pub kind: BuffBadgeKind,
    pub label: &'static str,
    pub secs: i32,
    pub bg_color: Color,
    pub border_color: Color,
}

/// Active buff/debuff badges derived from the player's `Vitals`.
pub fn active_buff_badges(v: &terravibe_sim::player::Vitals) -> Vec<BuffBadgeInfo> {
    let mut out = Vec::new();
    if v.potion_delay_ticks > 0 {
        out.push(BuffBadgeInfo {
            kind: BuffBadgeKind::PotionSickness,
            label: "Potion Sickness",
            secs: (v.potion_delay_ticks + 59) / 60,
            bg_color: Color::srgba(0.38, 0.10, 0.14, 0.90),
            border_color: Color::srgb(0.90, 0.25, 0.30),
        });
    }
    if v.burn_ticks > 0 {
        out.push(BuffBadgeInfo {
            kind: BuffBadgeKind::OnFire,
            label: "On Fire!",
            secs: (v.burn_ticks + 59) / 60,
            bg_color: Color::srgba(0.45, 0.16, 0.04, 0.92),
            border_color: Color::srgb(1.0, 0.50, 0.10),
        });
    }
    if v.obsidian_skin_ticks > 0 {
        out.push(BuffBadgeInfo {
            kind: BuffBadgeKind::ObsidianSkin,
            label: "Obsidian Skin",
            secs: (v.obsidian_skin_ticks + 59) / 60,
            bg_color: Color::srgba(0.18, 0.12, 0.36, 0.92),
            border_color: Color::srgb(0.62, 0.45, 0.95),
        });
    }
    if v.lava_max > 0 && v.lava_time < v.lava_max {
        out.push(BuffBadgeInfo {
            kind: BuffBadgeKind::LavaWard,
            label: "Lava Ward",
            secs: (v.lava_time + 59).max(0) / 60,
            bg_color: Color::srgba(0.38, 0.22, 0.06, 0.92),
            border_color: Color::srgb(0.95, 0.72, 0.20),
        });
    }
    if v.ironskin_ticks > 0 {
        out.push(BuffBadgeInfo {
            kind: BuffBadgeKind::Ironskin,
            label: "Ironskin",
            secs: (v.ironskin_ticks + 59) / 60,
            bg_color: Color::srgba(0.35, 0.30, 0.12, 0.92),
            border_color: Color::srgb(0.92, 0.80, 0.30),
        });
    }
    if v.swiftness_ticks > 0 {
        out.push(BuffBadgeInfo {
            kind: BuffBadgeKind::Swiftness,
            label: "Swiftness",
            secs: (v.swiftness_ticks + 59) / 60,
            bg_color: Color::srgba(0.12, 0.34, 0.22, 0.92),
            border_color: Color::srgb(0.35, 0.90, 0.58),
        });
    }
    if v.regen_ticks > 0 {
        out.push(BuffBadgeInfo {
            kind: BuffBadgeKind::Regen,
            label: "Regen",
            secs: (v.regen_ticks + 59) / 60,
            bg_color: Color::srgba(0.38, 0.14, 0.28, 0.92),
            border_color: Color::srgb(0.95, 0.42, 0.72),
        });
    }
    if v.shine_ticks > 0 {
        out.push(BuffBadgeInfo {
            kind: BuffBadgeKind::Shine,
            label: "Shine",
            secs: (v.shine_ticks + 59) / 60,
            bg_color: Color::srgba(0.38, 0.36, 0.14, 0.92),
            border_color: Color::srgb(1.0, 0.94, 0.45),
        });
    }
    if v.water_walking_ticks > 0 {
        out.push(BuffBadgeInfo {
            kind: BuffBadgeKind::WaterWalking,
            label: "Water Walk",
            secs: (v.water_walking_ticks + 59) / 60,
            bg_color: Color::srgba(0.10, 0.28, 0.42, 0.92),
            border_color: Color::srgb(0.32, 0.78, 0.98),
        });
    }
    if v.campfire_active {
        out.push(BuffBadgeInfo {
            kind: BuffBadgeKind::CozyFire,
            label: "Cozy Fire",
            secs: 1,
            bg_color: Color::srgba(0.42, 0.20, 0.08, 0.92),
            border_color: Color::srgb(1.0, 0.58, 0.22),
        });
    }
    if v.heart_lantern_active {
        out.push(BuffBadgeInfo {
            kind: BuffBadgeKind::HeartLantern,
            label: "Heart Lamp",
            secs: 1,
            bg_color: Color::srgba(0.42, 0.12, 0.20, 0.92),
            border_color: Color::srgb(1.0, 0.42, 0.56),
        });
    }
    out
}
#[derive(Component)]
pub struct CrosshairRoot;
#[derive(Component)]
pub struct CrosshairDot;
#[derive(Component)]
pub struct CrosshairTick;

/// A save notification, emitted when the server autosaves the character.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveEvent {
    WorldSaved,
}

/// Marker for the transient "World saved" toast, with its remaining lifetime.
#[derive(Component)]
pub struct SaveToastUi {
    pub timer: f32,
}

/// Seconds the save toast stays on screen.
pub const SAVE_TOAST_SECS: f32 = 2.5;

fn spawn_save_toast(mut commands: Commands, mut events: MessageReader<SaveEvent>) {
    for _ in events.read() {
        commands
            .spawn((
                SaveToastUi {
                    timer: SAVE_TOAST_SECS,
                },
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(24.0),
                    top: Val::Px(24.0),
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.06, 0.10, 0.06, 0.92)),
                BorderColor::all(Color::srgba(0.30, 0.65, 0.32, 0.95)),
                GlobalZIndex(1100),
                DespawnOnExit(AppState::InGame),
            ))
            .with_children(|p| {
                p.spawn((
                    Node {
                        width: Val::Px(10.0),
                        height: Val::Px(10.0),
                        margin: UiRect::right(Val::Px(8.0)),
                        border_radius: BorderRadius::all(Val::Px(5.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.30, 0.90, 0.38)),
                ));
                p.spawn((
                    Node::default(),
                    HudText {
                        text: "World saved".into(),
                        death: false,
                    },
                ));
            });
    }
}

fn expire_save_toast(
    time: Res<Time>,
    mut toasts: Query<(Entity, &mut SaveToastUi)>,
    mut commands: Commands,
) {
    for (entity, mut toast) in &mut toasts {
        toast.timer -= time.delta_secs();
        if toast.timer <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

/// Fill fraction (0..=1) of each icon: icon `i` shows `(cur - i*per) / per`, clamped.
pub fn fills(cur: i32, max: i32, per: i32) -> Vec<f32> {
    (0..(max.max(0) / per))
        .map(|i| ((cur - i * per) as f32 / per as f32).clamp(0.0, 1.0))
        .collect()
}

pub fn hearts_for(life: i32, life_max: i32) -> Vec<f32> {
    fills(life, life_max, PER_ICON)
}

pub fn stars_for(mana: i32, mana_max: i32) -> Vec<f32> {
    fills(mana, mana_max, PER_ICON)
}

/// (rgb, alpha, scale) of an icon, as in DrawLife: a partial icon has colour
/// `Color(n, n, n, 0.9 n)` with `n = (30 + 225 f) / 255` (premultiplied, so RGB fades with
/// alpha) and scale `f/4 + 0.75`; a full icon is `(1, 0.9)` at scale 1.
pub fn icon_style(fill: f32) -> (f32, f32, f32) {
    if fill >= 1.0 {
        return (1.0, 0.9, 1.0);
    }
    let n = (30.0 + 225.0 * fill).floor().max(30.0);
    (
        n / 255.0,
        (n * 0.9).floor() / 255.0,
        (fill / 4.0 + 0.75).max(0.75),
    )
}

/// Top-left of heart `i` (0-based) before scaling: 26 px pitch, 10 per row, rows 26 px apart.
pub fn heart_pos(i: usize) -> [f32; 2] {
    let (col, row) = ((i % 10) as f32, (i / 10) as f32);
    [500.0 + 26.0 * col, 32.0 + 26.0 * row]
}

/// Top-left of star `i` before scaling: column at x 775, 28 px pitch.
pub fn star_pos(i: usize) -> [f32; 2] {
    [775.0, 30.0 + 28.0 * i as f32]
}

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(HudSource {
            mouse: self.mouse.clone(),
            death: self.death.clone(),
            stack: self.stack.clone(),
            heart: self.heart.clone(),
            mana: self.mana.clone(),
            extra_ui: self.extra_ui.clone(),
        })
        .insert_resource(hotbar::HotbarSource {
            back: self.hotbar.back.clone(),
            back_selected: self.hotbar.back_selected.clone(),
            icons: self.hotbar.icons.clone(),
        })
        .init_resource::<ExtraUiAssets>()
        .init_resource::<HurtFlash>()
        .init_resource::<inventory::InventoryOpen>()
        .init_resource::<inventory::InventoryUi>()
        .init_resource::<chest::ChestPanel>()
        .init_resource::<town::TownPanel>()
        .init_resource::<tooltip::TooltipOverlay>()
        .add_message::<town::TownMsg>()
        .add_message::<SaveEvent>()
        .init_resource::<boss_bar::BossState>()
        .init_resource::<boss_bar::WofBossState>()
        .init_resource::<boss_bar::BossTitleCardState>()
        .init_resource::<death::DeathModalState>()
        .init_resource::<victory::VictoryModalState>()
        .init_resource::<onboarding::ContextualPromptsState>()
        .init_resource::<pickup_toast::PickupToastQueue>()
        .init_resource::<map::MapUiState>()
        .init_resource::<combat_feedback::DamageNumberPool>()
        .init_resource::<combat_feedback::HitMarkerState>()
        .init_resource::<combat_feedback::DamageDirectionState>()
        .add_message::<combat_feedback::CombatHitEvent>()
        .add_message::<crate::net::ChestMsg>()
        .add_message::<crate::net::CraftResults>()
        .add_message::<bevy::input::mouse::MouseWheel>()
        .add_systems(PreStartup, setup_hud_resources)
        .add_systems(Update, apply_hud_settings)
        .add_systems(
            OnEnter(AppState::InGame),
            (
                setup_hud,
                hotbar::setup,
                setup_vignette,
                inventory::setup.after(hotbar::setup),
                chest::setup,
                town::setup,
                tooltip::setup,
                boss_bar::setup,
                death::setup,
                victory::setup,
                onboarding::setup,
                pickup_toast::setup,
                map::setup_map_ui,
                combat_feedback::setup,
            ),
        )
        .add_systems(
            Update,
            (
                update_hud,
                update_potion_sickness,
                update_vignette,
                boss_bar::update,
                hotbar::update.run_if(resource_exists::<hotbar::HotbarAssets>),
                update_crosshair,
                armor::equip_input
                    .run_if(not(crate::view::camera::fly_mode))
                    .run_if(not(chest::is_open)),
                chest::open_input
                    .run_if(not(crate::view::camera::fly_mode))
                    .run_if(not(chest::is_open)),
                town::talk_input
                    .run_if(not(crate::view::camera::fly_mode))
                    .run_if(not(chest::is_open)),
                town::house_keys.run_if(not(crate::view::camera::fly_mode)),
                town::receive,
                town::handle_input.after(town::receive),
                town::update_view.after(town::handle_input),
                chest::receive,
                chest::handle_input
                    .run_if(chest::is_open)
                    .after(chest::receive),
                chest::update_cells
                    .run_if(chest::is_open)
                    .after(chest::receive),
                inventory::refresh_list.run_if(resource_exists::<inventory::InventoryUi>),
                inventory::click_input.after(inventory::refresh_list),
                inventory::update_view
                    .after(inventory::click_input)
                    .run_if(resource_exists::<hotbar::HotbarAssets>),
                rebuild_text
                    .after(update_hud)
                    .after(hotbar::update)
                    .after(inventory::update_view)
                    .after(tooltip::render_tooltip)
                    .after(death::update)
                    .after(onboarding::update_contextual_prompts)
                    .after(combat_feedback::update_combat_feedback),
            )
                .run_if(in_state(AppState::InGame)),
        )
        .add_systems(
            Update,
            town::draw_room_highlight
                .after(town::update_view)
                .run_if(in_state(AppState::InGame)),
        )
        .add_systems(
            Update,
            (
                combat_feedback::consume_combat_hits,
                combat_feedback::update_combat_feedback.after(combat_feedback::consume_combat_hits),
            )
                .run_if(in_state(AppState::InGame)),
        )
        .add_systems(
            Update,
            pickup_toast::update
                .before(rebuild_text)
                .run_if(resource_exists::<hotbar::HotbarAssets>)
                .run_if(in_state(AppState::InGame)),
        )
        .add_systems(
            Update,
            (
                tooltip::begin_frame
                    .before(town::update_view)
                    .before(chest::update_cells)
                    .before(inventory::update_view),
                tooltip::render_tooltip
                    .after(town::update_view)
                    .after(chest::update_cells)
                    .after(inventory::update_view),
            )
                .run_if(in_state(AppState::InGame)),
        )
        .add_systems(
            Update,
            (spawn_save_toast, expire_save_toast).run_if(in_state(AppState::InGame)),
        )
        .add_systems(
            Update,
            (death::update, death::handle_input).run_if(in_state(AppState::InGame)),
        )
        .add_systems(
            Update,
            (victory::update, victory::handle_input).run_if(in_state(AppState::InGame)),
        )
        .add_systems(
            Update,
            inventory::handle_throw_key.run_if(in_state(AppState::InGame)),
        )
        .add_systems(
            Update,
            (
                onboarding::sync_onboarding_ui,
                onboarding::update_contextual_prompts,
                map::handle_map_input,
                map::update_map_exploration,
                map::update_compass,
                map::update_minimap_rendering,
            )
                .run_if(in_state(AppState::InGame)),
        );
    }
}

/// Red screen-edge flash after the local player is hurt.
#[derive(Resource, Default)]
pub struct HurtFlash {
    pub ticks: f32,
}

const FLASH_TICKS: f32 = 20.0;

/// Vignette alpha for the remaining flash ticks.
pub fn flash_alpha(ticks: f32) -> f32 {
    0.35 * (ticks / FLASH_TICKS).clamp(0.0, 1.0)
}

#[derive(Component)]
struct HurtVignette;

fn setup_vignette(mut commands: Commands) {
    commands.spawn((
        HurtVignette,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.8, 0.0, 0.0, 0.0)),
        GlobalZIndex(1000),
        DespawnOnExit(AppState::InGame),
    ));
}

/// Heartbeat vignette alpha when `life / life_max <= 0.25`.
pub fn low_health_pulse_alpha(
    life: i32,
    life_max: i32,
    elapsed_secs: f32,
    reduced_motion: bool,
) -> f32 {
    if life <= 0 || life_max <= 0 || (life as f32 / life_max as f32) > 0.25 {
        return 0.0;
    }
    if reduced_motion {
        return 0.14;
    }
    let wave = 0.5 + 0.5 * (elapsed_secs * 6.0).sin();
    0.08 + 0.16 * wave
}

fn update_vignette(
    time: Res<Time>,
    mut events: MessageReader<crate::view::player::PlayerEvents>,
    mut flash: ResMut<HurtFlash>,
    player: Option<Res<LocalPlayer>>,
    settings: Option<Res<crate::settings::Settings>>,
    mut q: Query<&mut BackgroundColor, With<HurtVignette>>,
) {
    use terravibe_sim::player::PlayerEvent;
    for e in events.read() {
        if matches!(e.0, PlayerEvent::Hurt { .. }) {
            flash.ticks = FLASH_TICKS;
        }
    }
    flash.ticks = (flash.ticks - time.delta_secs() * 60.0).max(0.0);
    let reduced_motion = settings.as_ref().is_some_and(|s| s.reduced_motion);
    let pulse = player.as_ref().map_or(0.0, |p| {
        low_health_pulse_alpha(
            p.0.vitals.life,
            p.0.vitals.life_max,
            time.elapsed_secs(),
            reduced_motion,
        )
    });
    if let Ok(mut bg) = q.single_mut() {
        bg.0 = Color::srgba(0.8, 0.0, 0.0, flash_alpha(flash.ticks).max(pulse));
    }
}

fn abs_node(pos: [f32; 2]) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(pos[0]),
        top: Val::Px(pos[1]),
        ..default()
    }
}

fn build_extra_ui_assets(src: &ExtraUiSource, images: &mut Assets<Image>) -> ExtraUiAssets {
    let mut opt_img =
        |opt: &Option<RgbaImage>| opt.as_ref().map(|img| images.add(to_bevy_image(img)));
    ExtraUiAssets {
        coin_ghosts: [
            opt_img(&src.coin_ghosts[0]),
            opt_img(&src.coin_ghosts[1]),
            opt_img(&src.coin_ghosts[2]),
            opt_img(&src.coin_ghosts[3]),
        ],
        ammo_ghost: opt_img(&src.ammo_ghost),
        trash_ghost: opt_img(&src.trash_ghost),
        armor_head_ghost: opt_img(&src.armor_head_ghost),
        armor_body_ghost: opt_img(&src.armor_body_ghost),
        armor_legs_ghost: opt_img(&src.armor_legs_ghost),
        accessory_ghost: opt_img(&src.accessory_ghost),
        hammer_btn: opt_img(&src.hammer_btn),
        quick_stack_btn: opt_img(&src.quick_stack_btn),
        sort_btn: opt_img(&src.sort_btn),
        shield_icon: opt_img(&src.shield_icon),
        chat_back: opt_img(&src.chat_back),
        heart_fancy: opt_img(&src.heart_fancy),
        star_fancy: opt_img(&src.star_fancy),
        buff_potion_sickness: opt_img(&src.buff_potion_sickness),
    }
}

fn setup_hud_resources(
    mut commands: Commands,
    src: Option<Res<HudSource>>,
    mut images: ResMut<Assets<Image>>,
    existing: Option<Res<HudFonts>>,
) {
    if existing.is_some() {
        return;
    }
    let Some(src) = src else { return };
    let mouse_img = images.add(to_bevy_image(&src.mouse.atlas));
    let death_img = images.add(to_bevy_image(&src.death.atlas));
    let stack_img = images.add(to_bevy_image(&src.stack.atlas));
    let heart = images.add(to_bevy_image(&src.heart));
    let mana = images.add(to_bevy_image(&src.mana));

    commands.insert_resource(build_extra_ui_assets(&src.extra_ui, &mut images));

    commands.insert_resource(HudFont(src.mouse.clone(), mouse_img.clone()));
    commands.insert_resource(HudFonts {
        mouse: (src.mouse.clone(), mouse_img),
        death: (src.death.clone(), death_img),
        stack: (src.stack.clone(), stack_img),
    });
    commands.insert_resource(IconSize {
        heart: Vec2::new(src.heart.width as f32, src.heart.height as f32),
        mana: Vec2::new(src.mana.width as f32, src.mana.height as f32),
    });
    commands.insert_resource(HudImages { heart, mana });
}

fn setup_hud(
    mut commands: Commands,
    src: Option<Res<HudSource>>,
    mut assets: ResMut<Assets<Image>>,
    fonts: Option<Res<HudFonts>>,
    hud_images: Option<Res<HudImages>>,
    extra_assets: Option<Res<ExtraUiAssets>>,
) {
    let (heart, mana) = if let Some(hi) = hud_images {
        (hi.heart.clone(), hi.mana.clone())
    } else if let Some(src) = src.as_ref() {
        let mouse_img = assets.add(to_bevy_image(&src.mouse.atlas));
        let death_img = assets.add(to_bevy_image(&src.death.atlas));
        let stack_img = assets.add(to_bevy_image(&src.stack.atlas));
        let heart = assets.add(to_bevy_image(&src.heart));
        let mana = assets.add(to_bevy_image(&src.mana));
        if fonts.is_none() {
            commands.insert_resource(HudFont(src.mouse.clone(), mouse_img.clone()));
            commands.insert_resource(HudFonts {
                mouse: (src.mouse.clone(), mouse_img),
                death: (src.death.clone(), death_img),
                stack: (src.stack.clone(), stack_img),
            });
            commands.insert_resource(IconSize {
                heart: Vec2::new(src.heart.width as f32, src.heart.height as f32),
                mana: Vec2::new(src.mana.width as f32, src.mana.height as f32),
            });
        }
        if extra_assets.as_ref().is_none_or(|e| {
            e.hammer_btn.is_none() && e.quick_stack_btn.is_none() && e.shield_icon.is_none()
        }) {
            commands.insert_resource(build_extra_ui_assets(&src.extra_ui, &mut assets));
        }
        commands.insert_resource(HudImages {
            heart: heart.clone(),
            mana: mana.clone(),
        });
        (heart, mana)
    } else {
        return;
    };

    let label = |text: &str| HudText {
        text: text.to_string(),
        death: false,
    };
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(800.0),
                height: Val::Px(200.0),
                ..default()
            },
            DespawnOnExit(AppState::InGame),
        ))
        .with_children(|p| {
            for i in 0..MAX_HEARTS {
                let [hx, hy] = heart_pos(i);
                p.spawn((
                    HeartIcon(i),
                    abs_node([hx, hy]),
                    ImageNode::new(heart.clone()),
                    Visibility::Hidden,
                ));
            }
            for i in 0..MAX_STARS {
                let [sx, sy] = star_pos(i);
                p.spawn((
                    StarIcon(i),
                    abs_node([sx, sy]),
                    ImageNode::new(mana.clone()),
                    Visibility::Hidden,
                ));
            }
            p.spawn((LifeLabel, label("Life:"), abs_node([0.0, 6.0])));
            p.spawn((LifeValue, label(""), abs_node([0.0, 6.0])));
            p.spawn((ManaLabel, label("Mana"), abs_node([750.0, 6.0])));
        });

    for i in 0..8 {
        let left_x = 32.0 + i as f32 * 100.0;
        let mut icon_ent = commands.spawn((
            BuffSlotIcon(i),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(left_x),
                top: Val::Px(76.0),
                width: Val::Px(32.0),
                height: Val::Px(32.0),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.2, 0.2, 0.25, 0.85)),
            BorderColor::all(Color::srgb(0.6, 0.6, 0.7)),
            ImageNode::default(),
            Visibility::Hidden,
            DespawnOnExit(AppState::InGame),
        ));
        if i == 0 {
            icon_ent.insert(PotionSicknessIcon);
        }

        let mut text_ent = commands.spawn((
            BuffSlotText(i),
            StackText,
            HudText {
                text: String::new(),
                death: false,
            },
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(left_x),
                top: Val::Px(110.0),
                ..default()
            },
            Visibility::Hidden,
            DespawnOnExit(AppState::InGame),
        ));
        if i == 0 {
            text_ent.insert(PotionSicknessText);
        }

        commands.spawn((
            BuffSlotName(i),
            StackText,
            HudText {
                text: String::new(),
                death: false,
            },
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(left_x),
                top: Val::Px(124.0),
                ..default()
            },
            Visibility::Hidden,
            DespawnOnExit(AppState::InGame),
        ));
    }

    // Crosshair aiming reticle: centered at screen midpoint (50%, 50%)
    commands
        .spawn((
            CrosshairRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Percent(50.0),
                width: Val::Px(0.0),
                height: Val::Px(0.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Visibility::Inherited,
            DespawnOnExit(AppState::InGame),
        ))
        .with_children(|p| {
            // Center dot (4x4) with dark outline
            p.spawn((
                CrosshairDot,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(-2.0),
                    top: Val::Px(-2.0),
                    width: Val::Px(4.0),
                    height: Val::Px(4.0),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.9)),
                BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.85)),
            ));
            // 4 reticle ticks (top, bottom, left, right)
            p.spawn((
                CrosshairTick,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(-1.0),
                    top: Val::Px(-12.0),
                    width: Val::Px(2.0),
                    height: Val::Px(6.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.85)),
            ));
            p.spawn((
                CrosshairTick,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(-1.0),
                    top: Val::Px(6.0),
                    width: Val::Px(2.0),
                    height: Val::Px(6.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.85)),
            ));
            p.spawn((
                CrosshairTick,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(-12.0),
                    top: Val::Px(-1.0),
                    width: Val::Px(6.0),
                    height: Val::Px(2.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.85)),
            ));
            p.spawn((
                CrosshairTick,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(6.0),
                    top: Val::Px(-1.0),
                    width: Val::Px(6.0),
                    height: Val::Px(2.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.85)),
            ));
        });
    commands.spawn((
        DeathLabel,
        HudText {
            text: String::new(),
            death: true,
        },
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(50.0),
            left: Val::Percent(50.0),
            margin: UiRect::top(Val::Px(-60.0)),
            ..default()
        },
        DespawnOnExit(AppState::InGame),
    ));
}

/// Places one heart or star by its fill fraction (or hides it when out of range).
fn place_icon(
    fill: Option<f32>,
    pos: [f32; 2],
    size: Vec2,
    opacity: f32,
    node: &mut Mut<Node>,
    img: &mut Mut<ImageNode>,
    vis: &mut Mut<Visibility>,
) {
    let Some(f) = fill else {
        set(vis, Visibility::Hidden);
        return;
    };
    set(vis, Visibility::Inherited);
    let (n, a, s) = icon_style(f);
    let (l, t) = (
        Val::Px(pos[0] + size.x * (1.0 - s) / 2.0),
        Val::Px(pos[1] + size.y * (1.0 - s) / 2.0),
    );
    let (w, h) = (Val::Px(size.x * s), Val::Px(size.y * s));
    if (node.left, node.top, node.width, node.height) != (l, t, w, h) {
        node.left = l;
        node.top = t;
        node.width = w;
        node.height = h;
    }
    let c = Color::srgba(n, n, n, a * opacity);
    if img.color != c {
        img.color = c;
    }
}

/// Writes through change detection only when the value really differs.
fn set<T: PartialEq, M: std::ops::DerefMut<Target = T>>(m: &mut M, v: T) {
    if **m != v {
        **m = v;
    }
}

fn set_text(t: &mut Mut<HudText>, s: &str) {
    if t.text != s {
        t.text = s.to_string();
    }
}

#[allow(clippy::type_complexity)]
fn update_potion_sickness(
    player: Option<Res<LocalPlayer>>,
    extra: Option<Res<ExtraUiAssets>>,
    mut icons: Query<
        (
            &BuffSlotIcon,
            &mut ImageNode,
            &mut BackgroundColor,
            &mut BorderColor,
            &mut Visibility,
        ),
        (Without<BuffSlotText>, Without<BuffSlotName>),
    >,
    mut texts: Query<
        (&BuffSlotText, &mut HudText, &mut Visibility),
        (Without<BuffSlotIcon>, Without<BuffSlotName>),
    >,
    mut names: Query<
        (&BuffSlotName, &mut HudText, &mut Visibility),
        (Without<BuffSlotIcon>, Without<BuffSlotText>),
    >,
) {
    let Some(player) = player else { return };
    let v = &player.0.vitals;
    let badges = active_buff_badges(v);
    for (slot, mut img, mut bg, mut border, mut vis) in &mut icons {
        if let Some(badge) = badges.get(slot.0) {
            if badge.kind == BuffBadgeKind::PotionSickness
                && let Some(ref ex) = extra
                && let Some(ref h) = ex.buff_potion_sickness
            {
                if img.image != *h {
                    img.image = h.clone();
                }
            } else if img.image != Handle::default() {
                img.image = Handle::default();
            }
            if bg.0 != badge.bg_color {
                bg.0 = badge.bg_color;
            }
            let want_border = BorderColor::all(badge.border_color);
            if *border != want_border {
                *border = want_border;
            }
            set(&mut vis, Visibility::Inherited);
        } else {
            set(&mut vis, Visibility::Hidden);
        }
    }
    for (slot, mut t, mut vis) in &mut texts {
        if let Some(badge) = badges.get(slot.0) {
            let str_val = format!("{}s", badge.secs);
            set_text(&mut t, &str_val);
            set(&mut vis, Visibility::Inherited);
        } else {
            set(&mut vis, Visibility::Hidden);
        }
    }
    for (slot, mut t, mut vis) in &mut names {
        if let Some(badge) = badges.get(slot.0) {
            set_text(&mut t, badge.label);
            set(&mut vis, Visibility::Inherited);
        } else {
            set(&mut vis, Visibility::Hidden);
        }
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn update_hud(
    player: Option<Res<LocalPlayer>>,
    fonts: Res<HudFonts>,
    size: Res<IconSize>,
    settings: Option<Res<crate::settings::Settings>>,
    death_modal: Option<Res<death::DeathModalState>>,
    mut hearts: Query<(&HeartIcon, &mut Node, &mut ImageNode, &mut Visibility), Without<StarIcon>>,
    mut stars: Query<(&StarIcon, &mut Node, &mut ImageNode, &mut Visibility), Without<HeartIcon>>,
    mut texts: ParamSet<(
        Query<(&mut Node, &mut HudText), (With<LifeLabel>, Without<HeartIcon>, Without<StarIcon>)>,
        Query<(&mut Node, &mut HudText), (With<LifeValue>, Without<HeartIcon>, Without<StarIcon>)>,
        Query<
            (&mut Node, &mut Visibility),
            (With<ManaLabel>, Without<HeartIcon>, Without<StarIcon>),
        >,
        Query<(&mut Node, &mut HudText), (With<DeathLabel>, Without<HeartIcon>, Without<StarIcon>)>,
    )>,
) {
    let Some(player) = player else { return };
    let p = &player.0;
    let v = &p.vitals;
    let font = &fonts.mouse.0;
    let opacity = settings
        .as_ref()
        .map_or(1.0, |s| s.hud_opacity.clamp(0.25, 1.0));

    let hf = hearts_for(v.life, v.life_max);
    for (h, mut node, mut img, mut vis) in &mut hearts {
        let (f, pos) = (hf.get(h.0).copied(), heart_pos(h.0));
        place_icon(f, pos, size.heart, opacity, &mut node, &mut img, &mut vis);
    }
    let sf = stars_for(v.mana, v.mana_max);
    for (s, mut node, mut img, mut vis) in &mut stars {
        let (f, pos) = (sf.get(s.0).copied(), star_pos(s.0));
        place_icon(f, pos, size.mana, opacity, &mut node, &mut img, &mut vis);
    }

    // Life text: label left of centre, value right-aligned to the same box (DrawLife).
    let centre = 500.0 + 13.0 * hf.len().min(10) as f32;
    let value = format!("{}/{}", v.life, v.life_max);
    let w = measure(font, &format!("Life: {}/{}", v.life_max, v.life_max));
    for (mut node, mut t) in &mut texts.p0() {
        let x = Val::Px(centre - w / 2.0);
        if node.left != x {
            node.left = x;
        }
        set_text(&mut t, "Life:");
    }
    for (mut node, mut t) in &mut texts.p1() {
        let x = Val::Px(centre + w / 2.0 - measure(font, &value));
        if node.left != x {
            node.left = x;
        }
        set_text(&mut t, &value);
    }
    // Mana label left of the star column: x = 800 - max(50, width + 5) (DrawMana);
    // hidden together with the stars when there is no mana.
    let mw = measure(font, "Mana");
    let off = if mw >= 45.0 { mw + 5.0 } else { 50.0 };
    for (mut node, mut vis) in &mut texts.p2() {
        let x = Val::Px(800.0 - off);
        if node.left != x {
            node.left = x;
        }
        let want = if v.mana_max > 0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        set(&mut vis, want);
    }
    // Death text: centred, screenHeight/2 - 60 (Main.DrawInterface_35_YouDied). Suppressed
    // when the DeathModalRoot overlay is active so they never overlap.
    let modal_active = death_modal.as_ref().is_some_and(|d| d.active);
    let want = if p.dead && !modal_active {
        "You were slain..."
    } else {
        ""
    };
    for (mut node, mut t) in &mut texts.p3() {
        set_text(&mut t, want);
        let m = Val::Px(-measure(&fonts.death.0, want) / 2.0);
        if node.margin.left != m {
            node.margin.left = m;
        }
    }
}

/// Applies `Settings.ui_scale` to Bevy's `UiScale` and `Settings.hud_opacity` to HUD icons.
pub fn apply_hud_settings(
    settings: Option<Res<crate::settings::Settings>>,
    mut ui_scale: Option<ResMut<UiScale>>,
    mut hud_icons: Query<&mut ImageNode, With<PotionSicknessIcon>>,
) {
    let Some(settings) = settings else {
        return;
    };
    if let Some(ref mut scale) = ui_scale {
        let target = settings.ui_scale.clamp(0.75, 1.5);
        if (scale.0 - target).abs() > f32::EPSILON {
            scale.0 = target;
        }
    }
    let alpha = settings.hud_opacity.clamp(0.25, 1.0);
    for mut img in &mut hud_icons {
        if (img.color.alpha() - alpha).abs() > f32::EPSILON {
            img.color.set_alpha(alpha);
        }
    }
}

/// Returns `(dot_bg, dot_border, tick_bg)` for the aiming crosshair.
pub fn crosshair_colors(in_reach: bool, high_contrast: bool) -> (Color, Color, Color) {
    if high_contrast {
        let dot = if in_reach {
            Color::srgb(0.0, 1.0, 1.0)
        } else {
            Color::srgb(1.0, 1.0, 0.0)
        };
        (dot, Color::BLACK, Color::srgb(1.0, 1.0, 0.0))
    } else {
        let dot = if in_reach {
            Color::srgb(1.0, 0.85, 0.25) // golden reach highlight
        } else {
            Color::srgba(1.0, 1.0, 1.0, 0.9)
        };
        (
            dot,
            Color::srgba(0.0, 0.0, 0.0, 0.85),
            Color::srgba(1.0, 1.0, 1.0, 0.85),
        )
    }
}

pub fn update_crosshair(
    opts: Option<Single<&bevy::window::CursorOptions, With<bevy::window::PrimaryWindow>>>,
    mut root: Query<&mut Visibility, With<CrosshairRoot>>,
    mut dot: Query<(&mut BackgroundColor, &mut BorderColor), With<CrosshairDot>>,
    mut ticks: Query<&mut BackgroundColor, (With<CrosshairTick>, Without<CrosshairDot>)>,
    settings: Option<Res<crate::settings::Settings>>,
    cam: Option<Single<&Transform, With<crate::view::camera::FlyCamera>>>,
    stream: Option<Res<crate::view::chunks::ChunkStream>>,
    player: Option<Res<LocalPlayer>>,
    mode: Option<Res<crate::view::camera::CameraMode>>,
    orbit: Option<Res<crate::view::camera::OrbitDist>>,
    inv: Option<Res<crate::net::LocalInventory>>,
) {
    let locked = opts
        .as_ref()
        .is_some_and(|o| o.grab_mode == bevy::window::CursorGrabMode::Locked);
    let want_vis = if locked {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut vis in &mut root {
        if *vis != want_vis {
            *vis = want_vis;
        }
    }
    if !locked {
        return;
    }
    // Check if aiming at a block within reach
    let in_reach = if let (Some(cam), Some(stream), Some(player), Some(mode), Some(orbit)) =
        (cam, stream, player, mode, orbit)
    {
        let q = crate::view::player::LoadedBlocks(stream.generated());
        let held = inv
            .as_ref()
            .and_then(|i| i.0.selected_stack())
            .map(|s| s.id);
        let hit = crate::view::interact::target_block(
            &q,
            &cam,
            &player.0.body,
            crate::view::interact::cam_mode(&orbit, &mode),
            held,
        );
        hit.is_some()
    } else {
        false
    };

    let high_contrast = settings.as_ref().is_some_and(|s| s.high_contrast_crosshair);
    let (dot_col, dot_border_col, tick_col) = crosshair_colors(in_reach, high_contrast);
    for (mut bg, mut border) in &mut dot {
        if bg.0 != dot_col {
            bg.0 = dot_col;
        }
        let want_border = BorderColor::all(dot_border_col);
        if *border != want_border {
            *border = want_border;
        }
    }
    for mut bg in &mut ticks {
        if bg.0 != tick_col {
            bg.0 = tick_col;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn save_toast_spawns_and_expires() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .init_resource::<Time>()
            .add_message::<SaveEvent>()
            .add_systems(Update, (spawn_save_toast, expire_save_toast));
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();

        app.world_mut().write_message(SaveEvent::WorldSaved);
        app.update();
        assert!(
            app.world_mut()
                .query::<&SaveToastUi>()
                .iter(app.world())
                .next()
                .is_some(),
            "SaveToastUi must spawn on WorldSaved"
        );
        let texts: Vec<String> = app
            .world_mut()
            .query::<&HudText>()
            .iter(app.world())
            .map(|t| t.text.clone())
            .collect();
        assert!(
            texts.iter().any(|t| t == "World saved"),
            "toast text missing, found: {texts:?}"
        );

        // Advance time past the 2.5s toast duration.
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(3.0));
        app.update();
        assert!(
            app.world_mut()
                .query::<&SaveToastUi>()
                .iter(app.world())
                .next()
                .is_none(),
            "SaveToastUi must expire after its duration"
        );
    }

    #[test]
    fn hurt_flash_alpha_fades() {
        assert!((flash_alpha(20.0) - 0.35).abs() < 1e-6);
        assert!((flash_alpha(10.0) - 0.175).abs() < 1e-6);
        assert_eq!(flash_alpha(0.0), 0.0);
    }

    #[test]
    fn hearts_for_life() {
        assert_eq!(hearts_for(100, 100), vec![1.0; 5]);
        assert_eq!(hearts_for(50, 100), vec![1.0, 1.0, 0.5, 0.0, 0.0]);
        assert_eq!(hearts_for(0, 100), vec![0.0; 5]);
    }

    #[test]
    fn style_and_positions() {
        assert_eq!(icon_style(1.0), (1.0, 0.9, 1.0));
        let (n, a, s) = icon_style(0.0);
        assert!((n - 30.0 / 255.0).abs() < 1e-6);
        assert!((a - 27.0 / 255.0).abs() < 1e-6 && s == 0.75);
        assert_eq!(heart_pos(10), [500.0, 58.0]);
        assert_eq!(star_pos(1), [775.0, 58.0]);
        assert_eq!(stars_for(20, 40), vec![1.0, 0.0]);
    }

    #[derive(Resource, Default)]
    struct Changes(usize);

    fn count(q: Query<(), Changed<HudText>>, mut n: ResMut<Changes>) {
        n.0 += q.iter().count();
    }

    #[test]
    fn steady_state_does_not_mark_text_changed() {
        let img = || RgbaImage {
            width: 1,
            height: 1,
            pixels: vec![[0; 4]],
        };
        let font = Arc::new(SpriteFont {
            atlas: img(),
            glyphs: vec![],
            line_spacing: 10,
            spacing: 0.0,
            default_char: None,
        });
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .insert_resource(Assets::<Image>::default())
            .init_resource::<Time>()
            .add_message::<crate::view::player::PlayerEvents>()
            .insert_resource(LocalPlayer(terravibe_sim::player::Player::new([0.0; 3])))
            .insert_resource(Changes::default())
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<crate::session::GameMenu>()
            .insert_resource(crate::view::camera::ControlMode::Player)
            .add_plugins(HudPlugin {
                mouse: font.clone(),
                death: font.clone(),
                stack: font,
                hotbar: hotbar::HotbarSource {
                    back: img(),
                    back_selected: img(),
                    icons: Default::default(),
                },
                heart: img(),
                mana: img(),
                extra_ui: Default::default(),
            })
            .add_systems(Update, count.after(rebuild_text));
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();
        app.update();
        let settled = app.world().resource::<Changes>().0;
        assert!(settled > 0, "initial text should register as changed");
        app.update();
        app.update();
        assert_eq!(app.world().resource::<Changes>().0, settled);
    }

    #[test]
    fn potion_sickness_icon_and_text_toggle() {
        let img = || RgbaImage {
            width: 1,
            height: 1,
            pixels: vec![[0; 4]],
        };
        let font = Arc::new(SpriteFont {
            atlas: img(),
            glyphs: vec![],
            line_spacing: 10,
            spacing: 0.0,
            default_char: None,
        });
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .insert_resource(Assets::<Image>::default())
            .init_resource::<Time>()
            .add_message::<crate::view::player::PlayerEvents>()
            .insert_resource(LocalPlayer(terravibe_sim::player::Player::new([0.0; 3])))
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<crate::session::GameMenu>()
            .insert_resource(crate::view::camera::ControlMode::Player)
            .add_plugins(HudPlugin {
                mouse: font.clone(),
                death: font.clone(),
                stack: font,
                hotbar: hotbar::HotbarSource {
                    back: img(),
                    back_selected: img(),
                    icons: Default::default(),
                },
                heart: img(),
                mana: img(),
                extra_ui: Default::default(),
            });
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();
        app.update();

        // Initially no potion delay: hidden
        let mut q_vis = app
            .world_mut()
            .query_filtered::<&Visibility, With<PotionSicknessIcon>>();
        assert_eq!(*q_vis.single(app.world()).unwrap(), Visibility::Hidden);

        // Add potion sickness delay: 3600 ticks = 60s
        app.world_mut()
            .resource_mut::<LocalPlayer>()
            .0
            .vitals
            .potion_delay_ticks = 3600;
        app.update();

        let mut q_vis = app
            .world_mut()
            .query_filtered::<&Visibility, With<PotionSicknessIcon>>();
        assert_eq!(*q_vis.single(app.world()).unwrap(), Visibility::Inherited);

        let mut q_text = app
            .world_mut()
            .query_filtered::<&HudText, With<PotionSicknessText>>();
        assert_eq!(q_text.single(app.world()).unwrap().text, "60s");
    }

    #[test]
    fn active_buff_badges_renders_all_four_status_effects() {
        let mut v = terravibe_sim::player::Vitals::new();
        assert!(active_buff_badges(&v).is_empty());

        v.potion_delay_ticks = 1800; // 30s
        v.burn_ticks = 420; // 7s
        v.obsidian_skin_ticks = 3600; // 60s
        v.lava_max = 420;
        v.lava_time = 240; // 4s
        let badges = active_buff_badges(&v);
        assert_eq!(badges.len(), 4);
        assert_eq!(
            (badges[0].kind, badges[0].label, badges[0].secs),
            (BuffBadgeKind::PotionSickness, "Potion Sickness", 30)
        );
        assert_eq!(
            (badges[1].kind, badges[1].label, badges[1].secs),
            (BuffBadgeKind::OnFire, "On Fire!", 7)
        );
        assert_eq!(
            (badges[2].kind, badges[2].label, badges[2].secs),
            (BuffBadgeKind::ObsidianSkin, "Obsidian Skin", 60)
        );
        assert_eq!(
            (badges[3].kind, badges[3].label, badges[3].secs),
            (BuffBadgeKind::LavaWard, "Lava Ward", 4)
        );
    }

    #[test]
    fn potion_sickness_expiration_clears_slot_image_on_reuse() {
        let mut app = App::new();
        let mut images = Assets::<Image>::default();
        let bottle_handle = images.add(Image::default());
        app.insert_resource(images);
        app.insert_resource(ExtraUiAssets {
            buff_potion_sickness: Some(bottle_handle.clone()),
            ..Default::default()
        });
        let mut player = terravibe_sim::player::Player::new([0.0; 3]);
        player.vitals.potion_delay_ticks = 300;
        player.vitals.burn_ticks = 420;
        app.insert_resource(LocalPlayer(player));

        let icon_ent = app
            .world_mut()
            .spawn((
                BuffSlotIcon(0),
                ImageNode::default(),
                BackgroundColor(Color::BLACK),
                BorderColor::all(Color::BLACK),
                Visibility::Hidden,
            ))
            .id();
        let _text_ent = app
            .world_mut()
            .spawn((
                BuffSlotText(0),
                HudText {
                    text: String::new(),
                    death: false,
                },
                Visibility::Hidden,
            ))
            .id();
        let _name_ent = app
            .world_mut()
            .spawn((
                BuffSlotName(0),
                HudText {
                    text: String::new(),
                    death: false,
                },
                Visibility::Hidden,
            ))
            .id();

        app.add_systems(Update, update_potion_sickness);
        app.update();

        // Slot 0 has Potion Sickness -> bottle sprite is set
        assert_eq!(
            app.world().get::<ImageNode>(icon_ent).unwrap().image,
            bottle_handle
        );

        // Potion sickness expires; On Fire shifts to slot 0
        app.world_mut()
            .resource_mut::<LocalPlayer>()
            .0
            .vitals
            .potion_delay_ticks = 0;
        app.update();

        // Slot 0 reused for On Fire -> bottle sprite must be cleared
        assert_eq!(
            app.world().get::<ImageNode>(icon_ent).unwrap().image,
            Handle::<Image>::default()
        );
    }

    #[test]
    fn low_health_vignette_pulses_at_or_below_quarter_hp() {
        assert_eq!(low_health_pulse_alpha(50, 100, 0.5, false), 0.0);
        assert!(low_health_pulse_alpha(25, 100, 0.5, false) > 0.0);
        assert_eq!(low_health_pulse_alpha(0, 100, 0.5, false), 0.0);
        let a1 = low_health_pulse_alpha(20, 100, 0.1, true);
        let a2 = low_health_pulse_alpha(20, 100, 1.0, true);
        assert_eq!(a1, a2);
        assert!(a1 > 0.0);
    }

    #[test]
    fn hud_opacity_scales_heart_alpha_without_steady_state_fighting() {
        let img = || RgbaImage {
            width: 1,
            height: 1,
            pixels: vec![[0; 4]],
        };
        let font = Arc::new(SpriteFont {
            atlas: img(),
            glyphs: vec![],
            line_spacing: 10,
            spacing: 0.0,
            default_char: None,
        });
        let mut app = App::new();
        let mut settings = crate::settings::Settings::default();
        settings.hud_opacity = 0.5;
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .insert_resource(Assets::<Image>::default())
            .init_resource::<Time>()
            .add_message::<crate::view::player::PlayerEvents>()
            .insert_resource(LocalPlayer(terravibe_sim::player::Player::new([0.0; 3])))
            .insert_resource(Changes::default())
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<crate::session::GameMenu>()
            .insert_resource(crate::view::camera::ControlMode::Player)
            .insert_resource(settings)
            .add_plugins(HudPlugin {
                mouse: font.clone(),
                death: font.clone(),
                stack: font,
                hotbar: hotbar::HotbarSource {
                    back: img(),
                    back_selected: img(),
                    icons: Default::default(),
                },
                heart: img(),
                mana: img(),
                extra_ui: Default::default(),
            });

        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();
        app.update();

        let mut q_hearts = app
            .world_mut()
            .query_filtered::<&ImageNode, With<HeartIcon>>();
        let heart_img = q_hearts.iter(app.world()).next().expect("heart exists");
        assert!(
            (heart_img.color.alpha() - 0.45).abs() < 1e-4,
            "expected alpha ~0.45, got {}",
            heart_img.color.alpha()
        );

        fn count_img(q: Query<(), (With<HeartIcon>, Changed<ImageNode>)>, mut n: ResMut<Changes>) {
            n.0 += q.iter().count();
        }
        app.add_systems(Update, count_img);
        app.update(); // initial tick for count_img
        let settled = app.world().resource::<Changes>().0;
        app.update();
        app.update();
        assert_eq!(
            app.world().resource::<Changes>().0,
            settled,
            "no change detection triggers on steady state"
        );
    }
}
