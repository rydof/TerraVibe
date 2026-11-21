//! Client audio plugins and situational music.

use bevy::prelude::*;
use bevy::window::WindowFocused;

use crate::settings::Settings;

pub mod ambient;
pub mod music;
pub mod sfx;

pub use ambient::{AmbientBed, AmbientPlugin, select_ambient_bed};
pub use music::{MusicContext, MusicPlugin, Situation, choose, fade_gains, track_for};
pub use sfx::{FootstepMaterial, SfxEvent, SfxKind, SfxPlugin, footstep_material_for_block};

/// Tracks whether the game window currently has OS focus for `mute_on_focus_loss`.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioFocusState {
    pub focused: bool,
}

impl Default for AudioFocusState {
    fn default() -> Self {
        Self { focused: true }
    }
}

/// Updates [`AudioFocusState`] from Bevy [`WindowFocused`] messages.
pub fn update_audio_focus(
    mut focus: ResMut<AudioFocusState>,
    events: Option<MessageReader<WindowFocused>>,
) {
    let Some(mut events) = events else {
        return;
    };
    for ev in events.read() {
        focus.focused = ev.focused;
    }
}

/// Returns `true` when audio should be muted due to focus loss.
pub fn focus_muted(settings: Option<&Settings>, focus: Option<&AudioFocusState>) -> bool {
    settings.is_some_and(|s| s.mute_on_focus_loss) && focus.is_some_and(|f| !f.focused)
}
