//! Menu sound loading and playback.

use bevy::prelude::*;
use terravibe_assets::TerrariaContent;

#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuSoundEvent {
    Tick,
    Open,
    Close,
}

#[derive(Clone, Default)]
pub struct MenuSounds {
    pub tick: Option<Vec<u8>>,
    pub open: Option<Vec<u8>>,
    pub close: Option<Vec<u8>>,
}

impl MenuSounds {
    pub fn load(c: &TerrariaContent) -> Self {
        let get = |name: &str| match c.sound(name) {
            Ok(s) => Some(s.to_wav()),
            Err(e) => {
                warn!("{name}: {e}");
                None
            }
        };
        Self {
            tick: get("Sounds/Menu_Tick"),
            open: get("Sounds/Menu_Open"),
            close: get("Sounds/Menu_Close"),
        }
    }
}

#[derive(Resource, Clone, Default)]
pub struct MenuAudio {
    pub tick: Option<Handle<AudioSource>>,
    pub open: Option<Handle<AudioSource>>,
    pub close: Option<Handle<AudioSource>>,
}

pub fn play_menu_sounds(
    mut commands: Commands,
    mut events: MessageReader<MenuSoundEvent>,
    audio: Res<MenuAudio>,
    settings: Option<Res<crate::settings::Settings>>,
) {
    let gain = settings
        .as_ref()
        .map_or(0.75, |s| crate::settings::volume_gain(s.sound_volume));

    for event in events.read() {
        let handle = match event {
            MenuSoundEvent::Tick => audio.tick.clone(),
            MenuSoundEvent::Open => audio.open.clone(),
            MenuSoundEvent::Close => audio.close.clone(),
        };
        if let Some(h) = handle {
            commands.spawn((
                AudioPlayer(h),
                PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(gain)),
            ));
        }
    }
}

pub struct SoundsPlugin {
    pub sounds: MenuSounds,
}

impl Plugin for SoundsPlugin {
    fn build(&self, app: &mut App) {
        let mut audio = MenuAudio::default();
        if let Some(mut assets) = app.world_mut().get_resource_mut::<Assets<AudioSource>>() {
            if let Some(bytes) = self.sounds.tick.clone() {
                audio.tick = Some(assets.add(AudioSource {
                    bytes: bytes.into(),
                }));
            }
            if let Some(bytes) = self.sounds.open.clone() {
                audio.open = Some(assets.add(AudioSource {
                    bytes: bytes.into(),
                }));
            }
            if let Some(bytes) = self.sounds.close.clone() {
                audio.close = Some(assets.add(AudioSource {
                    bytes: bytes.into(),
                }));
            }
        }
        app.insert_resource(audio)
            .add_message::<MenuSoundEvent>()
            .add_systems(Update, play_menu_sounds);
    }
}
