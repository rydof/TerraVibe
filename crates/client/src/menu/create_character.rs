//! Character creation screen: name, hair style, color swatches and difficulty notices.

use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, RenderTarget};
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::window::{PrimaryWindow, Window};
use terravibe_server::characters::{characters_dir, load_character, save_character};
use terravibe_sim::character::{
    Appearance, Character, EYE_PRESETS, HAIR_COLOR_PRESETS, HairStyle, PANTS_PRESETS,
    SHIRT_PRESETS, SKIN_PRESETS, validate_name,
};

use super::style::{
    BORDER_W, BUTTON_HEIGHT, COLOR_BACKDROP, COLOR_HEADER_DIVIDER, COLOR_INPUT_ACTIVE_BORDER,
    COLOR_INPUT_BG, COLOR_PANEL_BG, COLOR_PANEL_BORDER, COLOR_PANEL_INNER, MenuSelected,
    RADIUS_BTN, RADIUS_PANEL, spawn_button_sized, viewport_size,
};
use super::text::MenuText;
use super::{Action, MenuFocus};
use crate::art::art_dir;
use crate::art::rig;
use crate::art::rig_view::{self, RigPart};
use crate::session::{AppState, MenuScreen, SelectedCharacter};
use crate::view::player_art::build_player_part_mesh;

const PREVIEW_AVATAR_SCALE: f32 = 0.9;

#[derive(Resource, Debug, Clone)]
pub struct CreateCharDraft {
    pub name: String,
    pub hair_idx: usize,
    pub skin_idx: usize,
    pub hair_color_idx: usize,
    pub eyes_idx: usize,
    pub shirt_idx: usize,
    pub pants_idx: usize,
    pub error: Option<String>,
}

impl Default for CreateCharDraft {
    fn default() -> Self {
        Self {
            name: "Player".into(),
            hair_idx: 0,
            skin_idx: 0,
            hair_color_idx: 0,
            eyes_idx: 0,
            shirt_idx: 0,
            pants_idx: 0,
            error: None,
        }
    }
}

impl CreateCharDraft {
    pub fn build_appearance(&self) -> Appearance {
        Appearance {
            hair: HairStyle::ALL[self.hair_idx % HairStyle::ALL.len()],
            skin: SKIN_PRESETS[self.skin_idx % SKIN_PRESETS.len()],
            hair_color: HAIR_COLOR_PRESETS[self.hair_color_idx % HAIR_COLOR_PRESETS.len()],
            eyes: EYE_PRESETS[self.eyes_idx % EYE_PRESETS.len()],
            shirt: SHIRT_PRESETS[self.shirt_idx % SHIRT_PRESETS.len()],
            pants: PANTS_PRESETS[self.pants_idx % PANTS_PRESETS.len()],
        }
    }
}

#[derive(Component)]
pub struct CreateCharRoot;

#[derive(Component)]
pub struct CharNameText;

#[derive(Component)]
pub struct CharErrorText;

#[derive(Component)]
pub struct HairValueText;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwatchKind {
    Skin,
    HairColor,
    Eyes,
    Shirt,
    Pants,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct CharSwatch(pub SwatchKind);

fn rgb_color(rgb: [u8; 3]) -> Color {
    Color::srgb(
        rgb[0] as f32 / 255.0,
        rgb[1] as f32 / 255.0,
        rgb[2] as f32 / 255.0,
    )
}

#[derive(Resource, Default)]
pub struct CreateCharRebuild(pub bool);

#[derive(Component)]
pub struct CharPreviewRoot;

#[derive(Component)]
pub struct CharPreviewRig;

#[derive(Component)]
pub struct CharPreviewCamera;

#[derive(Component)]
pub struct CharPreviewLight;

#[derive(Resource, Debug, Default)]
pub struct CharPreviewState {
    pub yaw: f32,
    pub last_appearance: Option<Appearance>,
    pub image: Option<Handle<Image>>,
}

fn hair_label(h: HairStyle) -> &'static str {
    match h {
        HairStyle::Bald => "Bald",
        HairStyle::Short => "Short",
        HairStyle::Long => "Long",
        HairStyle::Spiky => "Spiky",
        HairStyle::Mohawk => "Mohawk",
        HairStyle::Ponytail => "Ponytail",
    }
}

fn insert_render_layer_recursive(
    commands: &mut Commands,
    entity: Entity,
    children_q: &Query<&Children>,
    layer: RenderLayers,
) {
    commands.entity(entity).insert(layer.clone());
    if let Ok(children) = children_q.get(entity) {
        for child in children.iter() {
            insert_render_layer_recursive(commands, child, children_q, layer.clone());
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn setup_create_character(
    mut commands: Commands,
    draft: Option<ResMut<CreateCharDraft>>,
    mut focus: Option<ResMut<MenuFocus>>,
    mut preview_state: Option<ResMut<CharPreviewState>>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut materials: Option<ResMut<Assets<StandardMaterial>>>,
    mut images: Option<ResMut<Assets<Image>>>,
    primary_window: Option<Query<&Window, With<PrimaryWindow>>>,
) {
    let appearance = if let Some(ref d) = draft {
        d.build_appearance()
    } else {
        let d = CreateCharDraft::default();
        let app = d.build_appearance();
        commands.insert_resource(d);
        app
    };
    if let Some(ref mut f) = focus {
        f.rank = Some(0);
    }

    let (win_w, win_h) = viewport_size(primary_window.as_ref());
    let center_x = win_w * 0.5;
    let panel_w = 820.0;
    let panel_h = 630.0;
    let panel_left = center_x - panel_w * 0.5;
    let panel_top = ((win_h - panel_h) * 0.5).max(20.0);

    // Spawn offscreen 3D character preview (RenderLayers::layer(1))
    let preview_layer = RenderLayers::layer(1);
    let preview_img_handle = images.as_mut().map(|imgs| {
        let mut img = Image::new_fill(
            Extent3d {
                width: 280,
                height: 350,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[18, 22, 38, 255],
            TextureFormat::Bgra8UnormSrgb,
            default(),
        );
        img.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_DST
            | TextureUsages::RENDER_ATTACHMENT;
        imgs.add(img)
    });

    if let Some(ref mut ps) = preview_state {
        ps.yaw = 0.35;
        ps.last_appearance = None;
        ps.image = preview_img_handle.clone();
    } else {
        commands.insert_resource(CharPreviewState {
            yaw: 0.35,
            last_appearance: None,
            image: preview_img_handle.clone(),
        });
    }

    if let Some(ref img_handle) = preview_img_handle {
        commands.spawn((
            Camera3d::default(),
            Camera {
                order: -1,
                clear_color: ClearColorConfig::Custom(Color::srgba(0.06, 0.08, 0.14, 1.0)),
                ..default()
            },
            RenderTarget::Image(img_handle.clone().into()),
            Transform::from_xyz(0.0, 1.1, 3.1).looking_at(Vec3::new(0.0, 0.95, 0.0), Vec3::Y),
            preview_layer.clone(),
            DespawnOnExit(MenuScreen::CreateCharacter),
            DespawnOnExit(AppState::Menu),
            CharPreviewRoot,
            CharPreviewCamera,
        ));
    } else {
        commands.spawn((
            Transform::from_xyz(0.0, 1.1, 3.1).looking_at(Vec3::new(0.0, 0.95, 0.0), Vec3::Y),
            preview_layer.clone(),
            DespawnOnExit(MenuScreen::CreateCharacter),
            DespawnOnExit(AppState::Menu),
            CharPreviewRoot,
            CharPreviewCamera,
        ));
    }

    commands.spawn((
        DirectionalLight {
            illuminance: 6500.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(2.5, 4.0, 4.0).looking_at(Vec3::new(0.0, 0.9, 0.0), Vec3::Y),
        preview_layer.clone(),
        DespawnOnExit(MenuScreen::CreateCharacter),
        DespawnOnExit(AppState::Menu),
        CharPreviewRoot,
        CharPreviewLight,
    ));

    if let (Some(meshes), Some(materials)) = (meshes.as_mut(), materials.as_mut()) {
        let dir = art_dir().join("models/player");
        if let Ok(r) = rig::try_load(&dir) {
            let rig_root = rig_view::spawn_rig(&mut commands, meshes, materials, &r, &dir);
            commands.entity(rig_root).insert((
                Transform::from_scale(Vec3::splat(PREVIEW_AVATAR_SCALE))
                    .with_rotation(Quat::from_rotation_y(0.35)),
                preview_layer.clone(),
                DespawnOnExit(MenuScreen::CreateCharacter),
                DespawnOnExit(AppState::Menu),
                CharPreviewRoot,
                CharPreviewRig,
            ));
        } else {
            commands.spawn((
                Transform::from_scale(Vec3::splat(PREVIEW_AVATAR_SCALE)),
                preview_layer.clone(),
                DespawnOnExit(MenuScreen::CreateCharacter),
                DespawnOnExit(AppState::Menu),
                CharPreviewRoot,
                CharPreviewRig,
            ));
        }
    } else {
        commands.spawn((
            Transform::from_scale(Vec3::splat(PREVIEW_AVATAR_SCALE)),
            preview_layer.clone(),
            DespawnOnExit(MenuScreen::CreateCharacter),
            DespawnOnExit(AppState::Menu),
            CharPreviewRoot,
            CharPreviewRig,
        ));
    }

    // Backdrop scrim
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Px(win_w),
            height: Val::Px(win_h),
            ..default()
        },
        BackgroundColor(COLOR_BACKDROP),
        DespawnOnExit(MenuScreen::CreateCharacter),
        CreateCharRoot,
    ));

    // Main framed container
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(panel_left),
                top: Val::Px(panel_top),
                width: Val::Px(panel_w),
                height: Val::Px(panel_h),
                border: UiRect::all(Val::Px(BORDER_W)),
                border_radius: BorderRadius::all(Val::Px(RADIUS_PANEL)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(20.0)),
                ..default()
            },
            BackgroundColor(COLOR_PANEL_BG),
            BorderColor::all(COLOR_PANEL_BORDER),
            DespawnOnExit(MenuScreen::CreateCharacter),
            CreateCharRoot,
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    margin: UiRect::bottom(Val::Px(8.0)),
                    ..default()
                },
                MenuText::new("Create Character", 1.4),
            ));
            p.spawn((
                Node {
                    width: Val::Px(panel_w - 56.0),
                    height: Val::Px(2.0),
                    margin: UiRect::bottom(Val::Px(14.0)),
                    ..default()
                },
                BackgroundColor(COLOR_HEADER_DIVIDER),
            ));

            // Name input box (left column)
            p.spawn((Node {
                position_type: PositionType::Absolute,
                left: Val::Px(32.0),
                top: Val::Px(72.0),
                width: Val::Px(420.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },))
                .with_children(|col| {
                    col.spawn((
                        Node {
                            margin: UiRect::bottom(Val::Px(4.0)),
                            ..default()
                        },
                        MenuText::new("Character Name:", 0.9),
                    ));
                    col.spawn((
                        Node {
                            width: Val::Px(420.0),
                            height: Val::Px(38.0),
                            border: UiRect::all(Val::Px(BORDER_W)),
                            border_radius: BorderRadius::all(Val::Px(RADIUS_BTN)),
                            align_items: AlignItems::Center,
                            padding: UiRect::horizontal(Val::Px(12.0)),
                            margin: UiRect::bottom(Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(COLOR_INPUT_BG),
                        BorderColor::all(COLOR_INPUT_ACTIVE_BORDER),
                    ))
                    .with_children(|b| {
                        b.spawn((Node::default(), MenuText::new("Player", 1.05), CharNameText));
                    });
                    col.spawn((
                        Node {
                            height: Val::Px(18.0),
                            ..default()
                        },
                        MenuText::new("", 0.8),
                        CharErrorText,
                    ));
                });

            // Right column: Live 3D Preview Card
            p.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(488.0),
                    top: Val::Px(76.0),
                    width: Val::Px(296.0),
                    height: Val::Px(376.0),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    padding: UiRect::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(COLOR_PANEL_INNER),
                BorderColor::all(COLOR_PANEL_BORDER),
            ))
            .with_children(|card| {
                if let Some(img_handle) = preview_img_handle {
                    card.spawn((
                        Node {
                            width: Val::Px(276.0),
                            height: Val::Px(356.0),
                            ..default()
                        },
                        ImageNode::new(img_handle),
                    ));
                }
            });
        });

    // Left column: Customization buttons (2 columns x 3 rows + Randomize)
    let left_x = panel_left + 32.0;
    let col2_x = left_x + 216.0;
    let opt_w = 204.0;
    let opt_y0 = panel_top + 162.0;
    let opt_step = 48.0;

    let hair_text = format!("Hair: {}", hair_label(appearance.hair));
    let buttons: [(
        u16,
        &str,
        Action,
        (f32, f32),
        bool,
        Option<(SwatchKind, Color)>,
    ); 6] = [
        (
            0u16,
            hair_text.as_str(),
            Action::CycleHair,
            (left_x, opt_y0),
            true,
            None,
        ),
        (
            1,
            "Skin",
            Action::CycleSkin,
            (col2_x, opt_y0),
            false,
            Some((SwatchKind::Skin, rgb_color(appearance.skin))),
        ),
        (
            2,
            "Hair Color",
            Action::CycleHairColor,
            (left_x, opt_y0 + opt_step),
            false,
            Some((SwatchKind::HairColor, rgb_color(appearance.hair_color))),
        ),
        (
            3,
            "Eyes",
            Action::CycleEyes,
            (col2_x, opt_y0 + opt_step),
            false,
            Some((SwatchKind::Eyes, rgb_color(appearance.eyes))),
        ),
        (
            4,
            "Shirt",
            Action::CycleShirt,
            (left_x, opt_y0 + opt_step * 2.0),
            false,
            Some((SwatchKind::Shirt, rgb_color(appearance.shirt))),
        ),
        (
            5,
            "Pants",
            Action::CyclePants,
            (col2_x, opt_y0 + opt_step * 2.0),
            false,
            Some((SwatchKind::Pants, rgb_color(appearance.pants))),
        ),
    ];
    for (rank, label, action, pos, is_hair, swatch) in buttons {
        let b = spawn_button_sized(
            &mut commands,
            label,
            action,
            rank,
            pos,
            (opt_w, BUTTON_HEIGHT),
            false,
        );
        commands
            .entity(b)
            .insert((DespawnOnExit(MenuScreen::CreateCharacter), CreateCharRoot));
        if is_hair {
            commands.entity(b).insert(HairValueText);
        }
        if let Some((kind, color)) = swatch {
            commands.entity(b).with_children(|p| {
                p.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(10.0),
                        top: Val::Px((BUTTON_HEIGHT - 20.0) * 0.5 - 2.0),
                        width: Val::Px(20.0),
                        height: Val::Px(20.0),
                        border: UiRect::all(Val::Px(BORDER_W)),
                        border_radius: BorderRadius::all(Val::Px(RADIUS_BTN)),
                        ..default()
                    },
                    BackgroundColor(color),
                    BorderColor::all(COLOR_PANEL_BORDER),
                    CharSwatch(kind),
                ));
            });
        }
    }

    let b_rand = spawn_button_sized(
        &mut commands,
        "Randomize",
        Action::RandomizeCharacter,
        6,
        (left_x, opt_y0 + opt_step * 3.0),
        (420.0, BUTTON_HEIGHT),
        false,
    );
    commands
        .entity(b_rand)
        .insert((DespawnOnExit(MenuScreen::CreateCharacter), CreateCharRoot));

    // Difficulty row below Randomize
    let diff_w = 132.0;
    let diff_gap = 12.0;
    let diff_y = opt_y0 + opt_step * 4.0 + 6.0;
    for (idx, (label, disabled)) in [
        ("Softcore", false),
        ("Medium (Later)", true),
        ("Hard (Later)", true),
    ]
    .into_iter()
    .enumerate()
    {
        let b_diff = spawn_button_sized(
            &mut commands,
            label,
            Action::Noop,
            (7 + idx) as u16,
            (left_x + (idx as f32) * (diff_w + diff_gap), diff_y),
            (diff_w, BUTTON_HEIGHT),
            disabled,
        );
        commands.entity(b_diff).insert((
            DespawnOnExit(MenuScreen::CreateCharacter),
            CreateCharRoot,
            MenuSelected(!disabled),
        ));
    }

    // Preview rotate < / > buttons below the 3D preview box
    let rot_y = panel_top + 464.0;
    let b_rot_l = spawn_button_sized(
        &mut commands,
        "< Rotate",
        Action::RotatePreviewLeft,
        10,
        (panel_left + 488.0, rot_y),
        (140.0, BUTTON_HEIGHT),
        false,
    );
    commands
        .entity(b_rot_l)
        .insert((DespawnOnExit(MenuScreen::CreateCharacter), CreateCharRoot));

    let b_rot_r = spawn_button_sized(
        &mut commands,
        "Rotate >",
        Action::RotatePreviewRight,
        11,
        (panel_left + 644.0, rot_y),
        (140.0, BUTTON_HEIGHT),
        false,
    );
    commands
        .entity(b_rot_r)
        .insert((DespawnOnExit(MenuScreen::CreateCharacter), CreateCharRoot));

    // Bottom Action Buttons cleanly separated
    let foot_y = panel_top + panel_h - 68.0;
    let foot_w = 210.0;
    let b_create = spawn_button_sized(
        &mut commands,
        "Create",
        Action::CreateCharacterConfirm,
        12,
        (center_x - foot_w - 18.0, foot_y),
        (foot_w, BUTTON_HEIGHT),
        false,
    );
    commands
        .entity(b_create)
        .insert((DespawnOnExit(MenuScreen::CreateCharacter), CreateCharRoot));

    let b_cancel = spawn_button_sized(
        &mut commands,
        "Cancel",
        Action::Back,
        13,
        (center_x + 18.0, foot_y),
        (foot_w, BUTTON_HEIGHT),
        false,
    );
    commands
        .entity(b_cancel)
        .insert((DespawnOnExit(MenuScreen::CreateCharacter), CreateCharRoot));
}

pub fn handle_create_char_typing(
    key_events: Option<MessageReader<KeyboardInput>>,
    draft: Option<ResMut<CreateCharDraft>>,
    screen: Option<Res<State<MenuScreen>>>,
) {
    if screen.as_ref().map(|s| s.get()) != Some(&MenuScreen::CreateCharacter) {
        return;
    }
    let Some(mut key_events) = key_events else {
        return;
    };
    let Some(mut d) = draft else { return };

    for ev in key_events.read() {
        if !ev.state.is_pressed() {
            continue;
        }
        match ev.key_code {
            KeyCode::Backspace => {
                d.name.pop();
                d.error = None;
            }
            _ => {
                if let Key::Character(ref s) = ev.logical_key {
                    for c in s.chars() {
                        if (c.is_ascii_alphanumeric() || matches!(c, ' ' | '_' | '-'))
                            && d.name.len() < 32
                        {
                            d.name.push(c);
                            d.error = None;
                        }
                    }
                }
            }
        }
    }
}

pub fn handle_create_char_actions(
    mut actions: MessageReader<Action>,
    draft: Option<ResMut<CreateCharDraft>>,
    mut selected_char: Option<ResMut<SelectedCharacter>>,
    mut next_screen: Option<ResMut<NextState<MenuScreen>>>,
    mut rebuild: Option<ResMut<CreateCharRebuild>>,
    mut preview: Option<ResMut<CharPreviewState>>,
) {
    let Some(mut d) = draft else { return };
    let root = characters_dir();

    for action in actions.read() {
        match action {
            Action::CycleHair => {
                d.hair_idx = (d.hair_idx + 1) % HairStyle::ALL.len();
                if let Some(ref mut rb) = rebuild {
                    rb.0 = true;
                }
            }
            Action::CycleSkin => {
                d.skin_idx = (d.skin_idx + 1) % SKIN_PRESETS.len();
            }
            Action::CycleHairColor => {
                d.hair_color_idx = (d.hair_color_idx + 1) % HAIR_COLOR_PRESETS.len();
            }
            Action::CycleEyes => {
                d.eyes_idx = (d.eyes_idx + 1) % EYE_PRESETS.len();
            }
            Action::CycleShirt => {
                d.shirt_idx = (d.shirt_idx + 1) % SHIRT_PRESETS.len();
            }
            Action::CyclePants => {
                d.pants_idx = (d.pants_idx + 1) % PANTS_PRESETS.len();
            }
            Action::RandomizeCharacter => {
                let nanos = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as usize;
                d.hair_idx =
                    (d.hair_idx + 1 + (nanos % (HairStyle::ALL.len() - 1))) % HairStyle::ALL.len();
                d.skin_idx = (d.skin_idx + 1 + (nanos / 7) % 5) % SKIN_PRESETS.len();
                d.hair_color_idx =
                    (d.hair_color_idx + 1 + (nanos / 13) % 5) % HAIR_COLOR_PRESETS.len();
                d.eyes_idx = (d.eyes_idx + 1 + (nanos / 19) % 5) % EYE_PRESETS.len();
                d.shirt_idx = (d.shirt_idx + 1 + (nanos / 23) % 5) % SHIRT_PRESETS.len();
                d.pants_idx = (d.pants_idx + 1 + (nanos / 29) % 5) % PANTS_PRESETS.len();
                if let Some(ref mut rb) = rebuild {
                    rb.0 = true;
                }
            }
            Action::RotatePreviewLeft => {
                if let Some(ref mut p) = preview {
                    p.yaw -= 0.65;
                }
            }
            Action::RotatePreviewRight => {
                if let Some(ref mut p) = preview {
                    p.yaw += 0.65;
                }
            }
            Action::CreateCharacterConfirm => match validate_name(&d.name) {
                Err(e) => {
                    d.error = Some(format!("Invalid name: {e:?}"));
                }
                Ok(valid_name) => {
                    if load_character(&root, valid_name).is_ok() {
                        d.error = Some(format!("Character \"{valid_name}\" already exists"));
                    } else {
                        let appearance = d.build_appearance();
                        let character = Character::new(valid_name, appearance);
                        if let Err(e) = save_character(&root, &character) {
                            d.error = Some(format!("Failed to save: {e}"));
                        } else {
                            if let Some(ref mut sc) = selected_char {
                                sc.0 = Some(character);
                            }
                            if let Some(ref mut ns) = next_screen {
                                ns.set(MenuScreen::Worlds);
                            }
                        }
                    }
                }
            },
            _ => {}
        }
    }
}

pub fn update_char_preview(
    mut commands: Commands,
    _time: Option<Res<Time>>,
    draft: Option<Res<CreateCharDraft>>,
    mut preview: Option<ResMut<CharPreviewState>>,
    mut rebuild: Option<ResMut<CreateCharRebuild>>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut rig_q: Query<(Entity, &mut Transform), With<CharPreviewRig>>,
    mut parts_q: Query<(Entity, &RigPart, &mut Mesh3d, Option<&RenderLayers>)>,
    children_q: Query<&Children>,
) {
    let Some(ref mut preview) = preview else {
        return;
    };
    preview.yaw = preview.yaw.rem_euclid(std::f32::consts::TAU);

    for (rig_ent, mut tf) in &mut rig_q {
        tf.rotation = Quat::from_rotation_y(preview.yaw);
        if parts_q
            .iter()
            .any(|(_, p, _, layer)| p.root == rig_ent && layer.is_none())
        {
            insert_render_layer_recursive(
                &mut commands,
                rig_ent,
                &children_q,
                RenderLayers::layer(1),
            );
        }
    }

    let Some(draft) = draft else { return };
    let app = draft.build_appearance();
    let force_rebuild = rebuild.as_mut().is_some_and(|r| {
        let was = r.0;
        r.0 = false;
        was
    });
    if preview.last_appearance == Some(app) && !force_rebuild {
        return;
    }
    preview.last_appearance = Some(app);

    let Some(ref mut meshes) = meshes else {
        return;
    };
    let dir = art_dir().join("models/player");
    for (rig_ent, _) in &rig_q {
        for (_, part, mut mesh3d, _) in &mut parts_q {
            if part.root == rig_ent {
                if let Some(data) = build_player_part_mesh(&part.name, &app, &dir) {
                    if let Some(m) = crate::view::to_bevy_mesh(data) {
                        mesh3d.0 = meshes.add(m);
                    }
                }
            }
        }
    }
}

pub fn sync_create_char_ui(
    draft: Option<Res<CreateCharDraft>>,
    mut name_query: Query<&mut MenuText, (With<CharNameText>, Without<CharErrorText>)>,
    mut err_query: Query<&mut MenuText, (With<CharErrorText>, Without<CharNameText>)>,
    hair_btn_q: Query<&Children, With<HairValueText>>,
    mut hair_text_q: Query<&mut MenuText, (Without<CharNameText>, Without<CharErrorText>)>,
    mut swatch_q: Query<(&CharSwatch, &mut BackgroundColor)>,
) {
    let Some(d) = draft else { return };
    for mut text in &mut name_query {
        let display = if d.name.is_empty() { "_" } else { &d.name };
        if text.text != display {
            text.text = display.to_string();
        }
    }
    for mut text in &mut err_query {
        let display = d.error.as_deref().unwrap_or("");
        if text.text != display {
            text.text = display.to_string();
        }
    }
    let app = d.build_appearance();
    let hair_str = format!("Hair: {}", hair_label(app.hair));
    for children in &hair_btn_q {
        for child in children.iter() {
            if let Ok(mut t) = hair_text_q.get_mut(child) {
                if t.text != hair_str {
                    t.text = hair_str.clone();
                }
            }
        }
    }
    for (swatch, mut bg) in &mut swatch_q {
        bg.0 = match swatch.0 {
            SwatchKind::Skin => rgb_color(app.skin),
            SwatchKind::HairColor => rgb_color(app.hair_color),
            SwatchKind::Eyes => rgb_color(app.eyes),
            SwatchKind::Shirt => rgb_color(app.shirt),
            SwatchKind::Pants => rgb_color(app.pants),
        };
    }
}

pub struct CreateCharacterPlugin;

impl Plugin for CreateCharacterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CreateCharDraft>()
            .init_resource::<CreateCharRebuild>()
            .init_resource::<CharPreviewState>()
            .add_systems(OnEnter(MenuScreen::CreateCharacter), setup_create_character)
            .add_systems(
                Update,
                (
                    handle_create_char_typing,
                    handle_create_char_actions,
                    update_char_preview,
                    sync_create_char_ui,
                )
                    .run_if(in_state(MenuScreen::CreateCharacter)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn char_preview_spawns_and_updates_on_appearance_and_despawns_on_exit() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .add_sub_state::<MenuScreen>()
            .init_resource::<MenuFocus>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Image>>()
            .add_message::<Action>()
            .add_plugins(CreateCharacterPlugin);

        app.world_mut()
            .resource_mut::<NextState<MenuScreen>>()
            .set(MenuScreen::CreateCharacter);
        app.update();
        app.update();

        assert_eq!(
            app.world_mut()
                .query::<&CharPreviewCamera>()
                .iter(app.world())
                .count(),
            1,
            "preview camera must spawn in CreateCharacter"
        );
        assert_eq!(
            app.world_mut()
                .query::<&CharPreviewRig>()
                .iter(app.world())
                .count(),
            1,
            "preview rig must spawn in CreateCharacter"
        );

        let rig_ent = app
            .world_mut()
            .query_filtered::<Entity, With<CharPreviewRig>>()
            .single(app.world())
            .unwrap();

        let head_mesh_before = app
            .world_mut()
            .query::<(&RigPart, &Mesh3d)>()
            .iter(app.world())
            .find(|(p, _)| p.root == rig_ent && p.name == "head")
            .map(|(_, m)| m.0.clone());
        assert!(
            head_mesh_before.is_some(),
            "preview rig must have head RigPart"
        );

        // Change hair style & randomize -> head mesh handle must rebuild live
        app.world_mut().write_message(Action::CycleHair);
        app.update();
        app.update();

        let head_mesh_after = app
            .world_mut()
            .query::<(&RigPart, &Mesh3d)>()
            .iter(app.world())
            .find(|(p, _)| p.root == rig_ent && p.name == "head")
            .map(|(_, m)| m.0.clone());
        assert_ne!(
            head_mesh_before, head_mesh_after,
            "changing hair style must rebuild preview head mesh live"
        );

        // Exit CreateCharacter -> all preview entities must despawn cleanly
        app.world_mut()
            .resource_mut::<NextState<MenuScreen>>()
            .set(MenuScreen::Characters);
        app.update();
        app.update();

        assert_eq!(
            app.world_mut()
                .query::<&CharPreviewRoot>()
                .iter(app.world())
                .count(),
            0,
            "all preview entities must despawn on exiting CreateCharacter"
        );
    }
}
