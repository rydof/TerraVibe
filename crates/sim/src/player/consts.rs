//! Terraria movement constants (gameplay spec fields), converted from px/tick to
//! blocks/tick. 1 tile = 16 px = 0.686 blocks; 60 ticks/s unchanged.

/// Blocks per Terraria tile.
pub const TILE_BLOCKS: f32 = 0.686;
pub const PX_TO_BLOCKS: f32 = 0.686 / 16.0;

/// Player.defaultGravity = 0.4
pub const GRAVITY: f32 = 0.4 * PX_TO_BLOCKS;
/// Player.maxFallSpeed = 10
pub const MAX_FALL_SPEED: f32 = 10.0 * PX_TO_BLOCKS;
/// Player.jumpSpeed = 5.01
pub const JUMP_SPEED: f32 = 5.01 * PX_TO_BLOCKS;
/// Player.jumpHeight = 15 (ticks the jump is sustained)
pub const JUMP_HEIGHT_TICKS: i32 = 15;
/// Player.runAcceleration = 0.08
pub const RUN_ACCELERATION: f32 = 0.08 * PX_TO_BLOCKS;
/// Player.runSlowdown = 0.2
pub const RUN_SLOWDOWN: f32 = 0.2 * PX_TO_BLOCKS;
/// Player.maxRunSpeed = Player.originalRunSpeed = 3
pub const MAX_RUN_SPEED: f32 = 3.0 * PX_TO_BLOCKS;

/// Snappy ground acceleration: full speed in 6 ticks.
pub const GROUND_ACCEL: f32 = MAX_RUN_SPEED / 6.0;
/// Snappy ground braking: stop from full speed in 3 ticks.
pub const GROUND_STOP: f32 = MAX_RUN_SPEED / 3.0;
/// Air steering per tick, softer than the ground.
pub const AIR_ACCEL: f32 = MAX_RUN_SPEED / 20.0;

pub const HALF_WIDTH: f32 = 0.3;
pub const HEIGHT: f32 = 1.8;

pub const WATER_GRAVITY_SCALE: f32 = 0.4;
pub const WATER_MOVE_SCALE: f32 = 0.5;
pub const LAVA_MOVE_SCALE: f32 = 0.25;
pub const SWIM_UP_SPEED: f32 = 2.0 * PX_TO_BLOCKS;
pub const BREATH_MAX: i32 = 200;
pub const BREATH_TICKS: u32 = 7;
pub const BREATH_RECOVER: i32 = 3;
pub const DROWN_DAMAGE: i32 = 2;
pub const LAVA_DAMAGE: i32 = 80;
pub const BURN_TICKS: i32 = 420;
pub const LAVA_IMMUNITY_TICKS: u32 = 30;
pub const LAVA_CHARM_TICKS: i32 = 420;
pub const OBSIDIAN_SKIN_TICKS: i32 = 21_600;
pub const HOT_BURN_TICKS: i32 = 20;

pub const IRONSKIN_TICKS: i32 = 28_800;
pub const SWIFTNESS_TICKS: i32 = 28_800;
pub const REGEN_TICKS: i32 = 28_800;
pub const SHINE_TICKS: i32 = 18_000;
pub const WATER_WALKING_TICKS: i32 = 36_000;
pub const HEALING_POTION_HEAL: i32 = 100;
pub const LESSER_MANA_RESTORE: i32 = 50;
pub const MANA_RESTORE: i32 = 100;
