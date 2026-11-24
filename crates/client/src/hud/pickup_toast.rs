//! Coalesced rarity-colored item pickup toasts.

use bevy::prelude::*;
use terravibe_sim::item::{COPPER_COIN, GOLD_COIN, ItemId, PLATINUM_COIN, SILVER_COIN, item_data};

use super::hotbar::{HotbarAssets, icon_of};
use super::text::{HudText, HudTextColor};
use super::tooltip::rarity_color_for;
use crate::audio::{SfxEvent, SfxKind};
use crate::session::AppState;

/// Total lifetime of a pickup toast before expiration (seconds).
pub const TOAST_DURATION_SECS: f64 = 3.0;
/// Fade-out window at the end of a toast's lifetime (seconds).
pub const TOAST_FADE_SECS: f64 = 0.6;
/// Maximum number of simultaneous pickup toast rows.
pub const MAX_TOASTS: usize = 6;

const ICON_BOX: f32 = 24.0;
const ICON_MAX: f32 = 22.0;

/// Scales `size` down proportionally so its longest side is at most `max_px`,
/// keeping smaller native pixel art unscaled.
pub(crate) fn fit_icon_max(size: Vec2, max_px: f32) -> Vec2 {
    let max = size.max_element();
    if max <= 0.0 || max_px <= 0.0 {
        return Vec2::ZERO;
    }
    let scale = if max > max_px { max_px / max } else { 1.0 };
    (size * scale).round()
}

/// One active pickup toast entry.
#[derive(Clone, Debug, PartialEq)]
pub struct PickupToastEntry {
    pub item: ItemId,
    pub count: u32,
    pub updated_at: f64,
    pub expires_at: f64,
}

/// Queue of active pickup toasts with rapid-pickup coalescing.
#[derive(Resource, Default, Debug)]
pub struct PickupToastQueue {
    pub entries: Vec<PickupToastEntry>,
    pub pending_sfx: Vec<ItemId>,
}

impl PickupToastQueue {
    /// Adds `(item, count)` at timestamp `now`, coalescing into an active entry for `item`.
    pub fn push(&mut self, item: ItemId, count: u32, now: f64) {
        if count == 0 {
            return;
        }
        self.pending_sfx.push(item);
        self.tick(now);
        if let Some(entry) = self.entries.iter_mut().find(|e| e.item == item) {
            entry.count = entry.count.saturating_add(count);
            entry.updated_at = now;
            entry.expires_at = now + TOAST_DURATION_SECS;
            return;
        }
        self.entries.push(PickupToastEntry {
            item,
            count,
            updated_at: now,
            expires_at: now + TOAST_DURATION_SECS,
        });
        if self.entries.len() > MAX_TOASTS {
            let overflow = self.entries.len() - MAX_TOASTS;
            self.entries.drain(0..overflow);
        }
    }

    /// Returns `true` if any entry has expired at `now`.
    pub fn has_expired(&self, now: f64) -> bool {
        self.entries.iter().any(|e| now > e.expires_at)
    }

    /// Removes entries whose lifetime has elapsed at `now`.
    pub fn tick(&mut self, now: f64) {
        self.entries.retain(|e| now <= e.expires_at);
    }

    /// Opacity `[0.0, 1.0]` for an entry at timestamp `now`.
    pub fn alpha_for(entry: &PickupToastEntry, now: f64) -> f32 {
        let remaining = entry.expires_at - now;
        if remaining >= TOAST_FADE_SECS {
            1.0
        } else {
            (remaining / TOAST_FADE_SECS).clamp(0.0, 1.0) as f32
        }
    }
}

#[derive(Component)]
pub struct PickupToastRoot;

#[derive(Component)]
pub struct PickupToastRow {
    expires_at: f64,
    base_color: Color,
    last_alpha: f32,
}

#[derive(Component)]
pub struct PickupToastIcon;

#[derive(Component)]
pub struct PickupToastLabel;

/// Spawns the persistent pickup toast container in the bottom-right HUD corner.
pub fn setup(mut commands: Commands) {
    commands.spawn((
        PickupToastRoot,
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(24.0),
            bottom: Val::Px(24.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexEnd,
            row_gap: Val::Px(6.0),
            ..default()
        },
        GlobalZIndex(1040),
        DespawnOnExit(AppState::InGame),
    ));
}

/// Expires old toasts, rebuilds rows when the queue changes, and animates fade-out.
#[allow(clippy::too_many_arguments)]
pub fn update(
    mut commands: Commands,
    time: Option<Res<Time<Real>>>,
    mut queue: ResMut<PickupToastQueue>,
    roots: Query<(Entity, Option<&Children>), With<PickupToastRoot>>,
    mut rows: Query<(
        &mut PickupToastRow,
        &mut BackgroundColor,
        &mut BorderColor,
        &Children,
    )>,
    box_children: Query<&Children, (Without<PickupToastRow>, Without<PickupToastIcon>)>,
    mut icons: Query<&mut ImageNode, With<PickupToastIcon>>,
    mut labels: Query<&mut HudTextColor, With<PickupToastLabel>>,
    mut assets: ResMut<HotbarAssets>,
    mut images: ResMut<Assets<Image>>,
    settings: Option<Res<crate::settings::Settings>>,
    mut sfx: Option<ResMut<Messages<SfxEvent>>>,
) {
    if !queue.pending_sfx.is_empty() {
        let items: Vec<ItemId> = queue.pending_sfx.drain(..).collect();
        if let Some(ref mut bus) = sfx {
            for item in items {
                let kind = if matches!(item, COPPER_COIN | SILVER_COIN | GOLD_COIN | PLATINUM_COIN)
                {
                    SfxKind::Coin
                } else {
                    SfxKind::Pickup
                };
                bus.write(SfxEvent(kind));
            }
        }
    }
    let Some(time) = time else {
        return;
    };
    let now = time.elapsed_secs_f64();
    if queue.has_expired(now) {
        queue.tick(now);
    }

    if queue.is_changed() {
        for (root, children) in &roots {
            for child in children.into_iter().flatten() {
                commands.entity(*child).despawn();
            }
            let colorblind = settings.as_ref().is_some_and(|s| s.colorblind_palette);
            for entry in &queue.entries {
                let data = item_data(entry.item);
                let base_color = rarity_color_for(data.rarity, colorblind);
                let alpha = PickupToastQueue::alpha_for(entry, now);
                let (icon_handle, native_size) = icon_of(&mut assets, &mut images, entry.item);
                let icon_size = fit_icon_max(native_size, ICON_MAX);

                let row_entity = commands
                    .spawn((
                        PickupToastRow {
                            expires_at: entry.expires_at,
                            base_color,
                            last_alpha: alpha,
                        },
                        Node {
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.06, 0.08, 0.14, 0.88 * alpha)),
                        BorderColor::all(base_color.with_alpha(0.65 * alpha)),
                    ))
                    .with_children(|p| {
                        p.spawn((
                            Node {
                                width: Val::Px(ICON_BOX),
                                height: Val::Px(ICON_BOX),
                                margin: UiRect::right(Val::Px(8.0)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            children![(
                                PickupToastIcon,
                                Node {
                                    width: Val::Px(icon_size.x),
                                    height: Val::Px(icon_size.y),
                                    ..default()
                                },
                                ImageNode {
                                    image: icon_handle,
                                    color: Color::WHITE.with_alpha(alpha),
                                    ..default()
                                },
                            )],
                        ));
                        p.spawn((
                            PickupToastLabel,
                            Node::default(),
                            HudText {
                                text: format!("{} +{}", data.name, entry.count),
                                death: false,
                            },
                            HudTextColor(base_color.with_alpha(alpha)),
                        ));
                    })
                    .id();
                commands.entity(root).add_child(row_entity);
            }
        }
        return;
    }

    for (mut row, mut bg, mut border, children) in &mut rows {
        let remaining = row.expires_at - now;
        let alpha = if remaining >= TOAST_FADE_SECS {
            1.0
        } else {
            (remaining / TOAST_FADE_SECS).clamp(0.0, 1.0) as f32
        };
        if (alpha - row.last_alpha).abs() < 0.02 {
            continue;
        }
        row.last_alpha = alpha;
        *bg = BackgroundColor(Color::srgba(0.06, 0.08, 0.14, 0.88 * alpha));
        *border = BorderColor::all(row.base_color.with_alpha(0.65 * alpha));
        for child in children.iter() {
            if let Ok(mut label_color) = labels.get_mut(child) {
                label_color.0 = row.base_color.with_alpha(alpha);
            } else if let Ok(grand_children) = box_children.get(child) {
                for gc in grand_children.iter() {
                    if let Ok(mut icon) = icons.get_mut(gc) {
                        icon.color = Color::WHITE.with_alpha(alpha);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terravibe_sim::item::{GEL, TORCH};

    #[test]
    fn pickup_toast_coalesces_rapid_identical_items() {
        let mut q = PickupToastQueue::default();
        q.push(GEL, 2, 1.0);
        q.push(GEL, 3, 1.5);
        assert_eq!(q.entries.len(), 1);
        assert_eq!(q.entries[0].item, GEL);
        assert_eq!(q.entries[0].count, 5);

        q.push(TORCH, 1, 1.5);
        assert_eq!(q.entries.len(), 2);
        assert_eq!(q.entries[1].item, TORCH);
        assert_eq!(q.entries[1].count, 1);

        q.tick(4.0);
        assert_eq!(q.entries.len(), 2);

        q.tick(4.6);
        assert!(q.entries.is_empty());
    }

    #[test]
    fn pickup_toast_fades_near_expiry() {
        let mut q = PickupToastQueue::default();
        q.push(GEL, 1, 1.0);
        let entry = &q.entries[0];
        assert!((PickupToastQueue::alpha_for(entry, 2.0) - 1.0).abs() < 1e-5);
        let fade_alpha = PickupToastQueue::alpha_for(entry, 3.7);
        assert!(fade_alpha > 0.0 && fade_alpha < 1.0);
    }

    #[test]
    fn fit_icon_max_clamps_large_and_non_square_icons_to_icon_max() {
        let s16 = fit_icon_max(Vec2::new(16.0, 16.0), ICON_MAX);
        assert_eq!(s16, Vec2::new(16.0, 16.0));
        assert!(s16.max_element() <= ICON_MAX);

        let s32 = fit_icon_max(Vec2::new(32.0, 32.0), ICON_MAX);
        assert_eq!(s32, Vec2::new(22.0, 22.0));
        assert!(s32.max_element() <= ICON_MAX);

        let s64 = fit_icon_max(Vec2::new(64.0, 64.0), ICON_MAX);
        assert_eq!(s64, Vec2::new(22.0, 22.0));
        assert!(s64.max_element() <= ICON_MAX);

        let rect = fit_icon_max(Vec2::new(64.0, 32.0), ICON_MAX);
        assert_eq!(rect, Vec2::new(22.0, 11.0));
        assert!(rect.max_element() <= ICON_MAX);

        let tall = fit_icon_max(Vec2::new(16.0, 44.0), ICON_MAX);
        assert_eq!(tall, Vec2::new(8.0, 22.0));
        assert!(tall.max_element() <= ICON_MAX);
    }
}
