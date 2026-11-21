//! Unified sound-effects bus and volume-scaled playback.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::prelude::*;
use terravibe_assets::TerrariaContent;
use terravibe_sim::block::{
    ANVIL, BLUE_DUNGEON_BRICK, BlockId, CHAIR, CHEST, COPPER, DEMON_ALTAR, DEMONITE_ORE,
    DOOR_CLOSED, DOOR_OPEN, EBONSTONE, FURNACE, GLASS_BLOCK, GOLD, GREEN_DUNGEON_BRICK,
    HARDENED_SAND, HELLFORGE, HELLSTONE, HELLSTONE_BRICK, ICE, IRON, LIFE_CRYSTAL, OBSIDIAN,
    OBSIDIAN_BRICK, PINK_DUNGEON_BRICK, PLATFORM, POT, SANDSTONE, SHADOW_ORB, SILVER, SNOW, SPIKES,
    STONE, TABLE, TRUNK, WOOD_BLOCK, WORKBENCH,
};
use terravibe_sim::net::DamageKind;

use crate::settings::{Settings, volume_gain};

/// Surface material category for material-aware footsteps and landing sounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FootstepMaterial {
    Wood,
    Stone,
    Dirt,
    Snow,
    Liquid,
}

/// Maps a [`BlockId`] to its corresponding [`FootstepMaterial`].
pub fn footstep_material_for_block(block: BlockId) -> FootstepMaterial {
    match block {
        WOOD_BLOCK | TRUNK | PLATFORM | TABLE | CHAIR | WORKBENCH | CHEST | DOOR_CLOSED
        | DOOR_OPEN => FootstepMaterial::Wood,
        SNOW | ICE => FootstepMaterial::Snow,
        STONE | IRON | COPPER | GOLD | SILVER | DEMONITE_ORE | EBONSTONE | HELLSTONE
        | SANDSTONE | HARDENED_SAND | OBSIDIAN | OBSIDIAN_BRICK | HELLSTONE_BRICK
        | BLUE_DUNGEON_BRICK | GREEN_DUNGEON_BRICK | PINK_DUNGEON_BRICK | ANVIL | FURNACE
        | HELLFORGE | SPIKES | GLASS_BLOCK | DEMON_ALTAR | SHADOW_ORB | LIFE_CRYSTAL | POT => {
            FootstepMaterial::Stone
        }
        _ => FootstepMaterial::Dirt,
    }
}

/// All gameplay and UI sound effect categories routed through the unified SFX bus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SfxKind {
    Equip,
    Draw,
    Release,
    Hit,
    Crit,
    Kill,
    PlayerHurt(DamageKind),
    Death,
    Pickup,
    Coin,
    Craft,
    UiClick,
    Telegraph,
    BossRoar,
    BossTransform,
    BossDash,
    ServantSpawn,
    Footstep(FootstepMaterial),
    Land(FootstepMaterial),
    Thunder,
}

/// Bevy message requesting playback of a sound effect.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SfxEvent(pub SfxKind);

/// Maps an [`SfxKind`] to its Terraria sound asset path (without extension).
pub fn sound_path_for(kind: SfxKind) -> &'static str {
    match kind {
        SfxKind::Equip => "Sounds/Grab",
        SfxKind::Draw => "Sounds/Item_5",
        SfxKind::Release => "Sounds/Item_10",
        SfxKind::Hit => "Sounds/NPC_Hit_1",
        SfxKind::Crit => "Sounds/NPC_Hit_2",
        SfxKind::Kill => "Sounds/NPC_Killed_1",
        SfxKind::PlayerHurt(damage_kind) => match damage_kind {
            DamageKind::Melee | DamageKind::Contact => "Sounds/Player_Hit_0",
            DamageKind::Ranged | DamageKind::Projectile => "Sounds/Player_Hit_1",
            DamageKind::Magic | DamageKind::Environment | DamageKind::Unknown => {
                "Sounds/Player_Hit_2"
            }
        },
        SfxKind::Death => "Sounds/Player_Killed",
        SfxKind::Pickup => "Sounds/Grab",
        SfxKind::Coin => "Sounds/Coins",
        SfxKind::Craft => "Sounds/Research_0",
        SfxKind::UiClick => "Sounds/Menu_Tick",
        SfxKind::Telegraph => "Sounds/Item_8",
        SfxKind::BossRoar => "Sounds/Roar_0",
        SfxKind::BossTransform => "Sounds/NPC_Killed_1",
        SfxKind::BossDash => "Sounds/Roar_0",
        SfxKind::ServantSpawn => "Sounds/NPC_Hit_1",
        SfxKind::Footstep(mat) => match mat {
            FootstepMaterial::Wood => "Sounds/Dig_1",
            FootstepMaterial::Stone => "Sounds/Tink_0",
            FootstepMaterial::Dirt => "Sounds/Dig_0",
            FootstepMaterial::Snow => "Sounds/Dig_2",
            FootstepMaterial::Liquid => "Sounds/Splash_0",
        },
        SfxKind::Land(mat) => match mat {
            FootstepMaterial::Wood => "Sounds/Dig_1",
            FootstepMaterial::Stone => "Sounds/Dig_2",
            FootstepMaterial::Dirt | FootstepMaterial::Snow => "Sounds/Dig_0",
            FootstepMaterial::Liquid => "Sounds/Splash_0",
        },
        SfxKind::Thunder => "Sounds/Thunder_0",
    }
}

/// Returns `true` for UI/inventory/economy sound effects routed through `Settings.ui_volume`.
pub fn is_ui_sfx(kind: SfxKind) -> bool {
    matches!(kind, SfxKind::UiClick | SfxKind::Craft | SfxKind::Coin)
}

/// Computes the linear playback gain for `kind` given a bus volume (`0..=100`).
pub fn sfx_gain(kind: SfxKind, volume: u8) -> f32 {
    let base = match kind {
        SfxKind::UiClick => 0.85,
        SfxKind::Footstep(_) => 0.25,
        SfxKind::Land(_) => 0.45,
        _ => 1.0,
    };
    (volume_gain(volume) * base).clamp(0.0, 1.0)
}

/// Computes the linear playback gain for `kind` using `Settings.ui_volume` for UI/craft/coin
/// sounds, `Settings.sound_volume` for gameplay SFX, and muting when unfocused if configured.
pub fn sfx_gain_for_settings(kind: SfxKind, settings: &Settings, focused: bool) -> f32 {
    if settings.mute_on_focus_loss && !focused {
        return 0.0;
    }
    let volume = if is_ui_sfx(kind) {
        settings.ui_volume
    } else {
        settings.sound_volume
    };
    sfx_gain(kind, volume)
}

#[derive(Resource, Default)]
pub struct SfxManager {
    pub content: Option<Arc<TerrariaContent>>,
    pub cache: HashMap<&'static str, Option<Handle<AudioSource>>>,
}

impl SfxManager {
    fn get_or_load(
        &mut self,
        path: &'static str,
        audio_assets: &mut Assets<AudioSource>,
    ) -> Option<Handle<AudioSource>> {
        if let Some(cached) = self.cache.get(path) {
            return cached.clone();
        }
        let Some(content) = &self.content else {
            self.cache.insert(path, None);
            return None;
        };
        let handle = match content.sound(path) {
            Ok(sfx) => {
                let bytes: Arc<[u8]> = sfx.to_wav().into();
                Some(audio_assets.add(AudioSource { bytes }))
            }
            Err(e) => {
                warn!("sfx {path}: {e}");
                None
            }
        };
        self.cache.insert(path, handle.clone());
        handle
    }
}

/// Plugin registering [`SfxEvent`] and the [`play_sfx`] playback system.
pub struct SfxPlugin {
    pub content: Option<Arc<TerrariaContent>>,
}

impl Plugin for SfxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<crate::audio::AudioFocusState>()
            .insert_resource(SfxManager {
                content: self.content.clone(),
                cache: HashMap::new(),
            })
            .add_message::<SfxEvent>()
            .add_systems(Update, (crate::audio::update_audio_focus, play_sfx).chain());
    }
}

/// Consumes [`SfxEvent`] messages and spawns volume-scaled one-shot audio players.
pub fn play_sfx(
    mut commands: Commands,
    mut events: MessageReader<SfxEvent>,
    mut manager: ResMut<SfxManager>,
    settings: Option<Res<Settings>>,
    focus: Option<Res<crate::audio::AudioFocusState>>,
    mut audio_assets: Option<ResMut<Assets<AudioSource>>>,
) {
    let focused = focus.as_ref().is_none_or(|f| f.focused);
    for SfxEvent(kind) in events.read() {
        let gain = settings.as_deref().map_or_else(
            || sfx_gain(*kind, 75),
            |s| sfx_gain_for_settings(*kind, s, focused),
        );
        if gain <= 0.0 {
            continue;
        }
        let Some(assets) = audio_assets.as_deref_mut() else {
            continue;
        };
        let path = sound_path_for(*kind);
        if let Some(handle) = manager.get_or_load(path, assets) {
            commands.spawn((
                AudioPlayer(handle),
                PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(gain)),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terravibe_sim::net::DamageKind;

    #[test]
    fn sfx_kind_maps_every_event_to_sound_path_and_scales_by_sound_volume() {
        let kinds = [
            SfxKind::Equip,
            SfxKind::Draw,
            SfxKind::Release,
            SfxKind::Hit,
            SfxKind::Crit,
            SfxKind::Kill,
            SfxKind::PlayerHurt(DamageKind::Melee),
            SfxKind::PlayerHurt(DamageKind::Ranged),
            SfxKind::PlayerHurt(DamageKind::Magic),
            SfxKind::PlayerHurt(DamageKind::Contact),
            SfxKind::PlayerHurt(DamageKind::Projectile),
            SfxKind::PlayerHurt(DamageKind::Environment),
            SfxKind::PlayerHurt(DamageKind::Unknown),
            SfxKind::Death,
            SfxKind::Pickup,
            SfxKind::Coin,
            SfxKind::Craft,
            SfxKind::UiClick,
            SfxKind::Telegraph,
            SfxKind::BossRoar,
            SfxKind::BossTransform,
            SfxKind::BossDash,
            SfxKind::ServantSpawn,
            SfxKind::Thunder,
        ];

        for kind in kinds {
            let path = sound_path_for(kind);
            assert!(
                path.starts_with("Sounds/"),
                "expected Sounds/ prefix for {kind:?}, got {path}"
            );
            assert_eq!(sfx_gain(kind, 0), 0.0, "muted volume must yield 0.0");
            let half = sfx_gain(kind, 50);
            let full = sfx_gain(kind, 100);
            assert!(
                half > 0.0 && half < full,
                "expected monotonic gain for {kind:?}"
            );
            assert!(full <= 1.0, "gain must not exceed 1.0 for {kind:?}");
        }
    }

    #[test]
    fn play_sfx_spawns_scaled_audio_and_skips_when_muted() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<AudioSource>()
            .insert_resource(Settings {
                sound_volume: 50,
                ..Settings::default()
            })
            .add_plugins(SfxPlugin { content: None });

        let dummy_handle = app
            .world_mut()
            .resource_mut::<Assets<AudioSource>>()
            .add(AudioSource {
                bytes: vec![0u8; 4].into(),
            });
        app.world_mut()
            .resource_mut::<SfxManager>()
            .cache
            .insert(sound_path_for(SfxKind::Hit), Some(dummy_handle));

        app.world_mut().write_message(SfxEvent(SfxKind::Hit));
        app.update();

        let count = app
            .world_mut()
            .query::<(&AudioPlayer, &PlaybackSettings)>()
            .iter(app.world())
            .count();
        assert_eq!(count, 1);

        // Muting sound_volume must not spawn additional AudioPlayer entities.
        app.world_mut().resource_mut::<Settings>().sound_volume = 0;
        app.world_mut().write_message(SfxEvent(SfxKind::Hit));
        app.update();

        let count_after_mute = app
            .world_mut()
            .query::<(&AudioPlayer, &PlaybackSettings)>()
            .iter(app.world())
            .count();
        assert_eq!(count_after_mute, 1);
    }

    #[test]
    fn footstep_material_differentiates_wood_stone_dirt_snow_liquid() {
        use terravibe_sim::block::{DIRT, GRASS, SAND};
        assert_eq!(
            footstep_material_for_block(WOOD_BLOCK),
            FootstepMaterial::Wood
        );
        assert_eq!(
            footstep_material_for_block(PLATFORM),
            FootstepMaterial::Wood
        );
        assert_eq!(footstep_material_for_block(TRUNK), FootstepMaterial::Wood);
        assert_eq!(footstep_material_for_block(STONE), FootstepMaterial::Stone);
        assert_eq!(
            footstep_material_for_block(BLUE_DUNGEON_BRICK),
            FootstepMaterial::Stone
        );
        assert_eq!(
            footstep_material_for_block(OBSIDIAN),
            FootstepMaterial::Stone
        );
        assert_eq!(footstep_material_for_block(DIRT), FootstepMaterial::Dirt);
        assert_eq!(footstep_material_for_block(GRASS), FootstepMaterial::Dirt);
        assert_eq!(footstep_material_for_block(SAND), FootstepMaterial::Dirt);
        assert_eq!(footstep_material_for_block(SNOW), FootstepMaterial::Snow);
        assert_eq!(footstep_material_for_block(ICE), FootstepMaterial::Snow);
        assert_ne!(FootstepMaterial::Liquid, FootstepMaterial::Dirt);
        assert_ne!(FootstepMaterial::Liquid, FootstepMaterial::Stone);
        assert_ne!(FootstepMaterial::Liquid, FootstepMaterial::Wood);
        assert_ne!(FootstepMaterial::Liquid, FootstepMaterial::Snow);
    }

    #[test]
    fn audio_buses_scale_independently_and_mute_on_focus_loss() {
        use crate::audio::AudioFocusState;
        use crate::audio::ambient::ambient_gain_for_settings;
        use crate::audio::music::music_gain_for_settings;

        let mut s = Settings::default();
        s.sound_volume = 80;
        s.ui_volume = 40;
        s.ambient_volume = 60;
        s.music_volume = 20;
        s.mute_on_focus_loss = true;

        // Focused: each bus scales independently.
        assert!(
            (sfx_gain_for_settings(SfxKind::Hit, &s, true)
                - sfx_gain_for_settings(SfxKind::UiClick, &s, true))
            .abs()
                > 0.1
        );
        assert_eq!(sfx_gain_for_settings(SfxKind::Hit, &s, true), 0.8);
        assert!((sfx_gain_for_settings(SfxKind::UiClick, &s, true) - 0.4 * 0.85).abs() < 1e-5);
        assert!((ambient_gain_for_settings(&s, true) - 0.6 * 0.35).abs() < 1e-5);
        assert!((music_gain_for_settings(&s, true) - 0.2).abs() < 1e-5);

        // Unfocused with mute_on_focus_loss = true: all buses mute to 0.0.
        assert_eq!(sfx_gain_for_settings(SfxKind::Hit, &s, false), 0.0);
        assert_eq!(sfx_gain_for_settings(SfxKind::UiClick, &s, false), 0.0);
        assert_eq!(ambient_gain_for_settings(&s, false), 0.0);
        assert_eq!(music_gain_for_settings(&s, false), 0.0);

        // Unfocused with mute_on_focus_loss = false: buses remain active.
        s.mute_on_focus_loss = false;
        assert_eq!(sfx_gain_for_settings(SfxKind::Hit, &s, false), 0.8);
        assert!((ambient_gain_for_settings(&s, false) - 0.6 * 0.35).abs() < 1e-5);
        assert!((music_gain_for_settings(&s, false) - 0.2).abs() < 1e-5);
        let _focus = AudioFocusState::default();
    }
}
