//! Arrow projectiles (Terraria `Projectile` type 1 Wooden Arrow, type 2 Flaming
//! Arrow, `aiStyle` 1). Pure: the server owns and steps them.

use crate::chunk::BlockQuery;
use crate::combat::{Hit, apply_hit, apply_hit_with_immunity};
use crate::enemy::{Enemy, EnemyId};
use crate::item::{
    AMAZON, FALLEN_STAR, FLAMARANG, FLAMING_ARROW, ItemId, MAGIC_MISSILE, MOLTEN_FURY, Prefix, Rng,
    THORN_CHAKRAM, VALOR, WOODEN_ARROW, WOODEN_BOW, item_data, prefix_stats,
};
use crate::player::consts::PX_TO_BLOCKS as PX;

/// Live projectiles per server (Stage 5 plan cap).
pub const MAX_PROJECTILES: usize = 64;
/// `Projectile.item defaults` type 1 and 2: `timeLeft = 1200` (gameplay spec, :830).
pub const ARROW_TIME_LEFT: i32 = 1200;
/// `AI_001`: `ai[0]` counts up every tick (`flag3`, gameplay spec) and gravity
/// starts once it reaches 15 (gameplay spec).
pub const GRAVITY_DELAY: i32 = 15;
/// `AI_001` gravity per tick once the delay passed: `velocity.Y += 0.1f` (gameplay spec).
pub const ARROW_GRAVITY: f32 = 0.1 * PX;
/// `AI_001` terminal fall speed: `velocity.Y > 16f` clamp (gameplay spec).
pub const ARROW_MAX_FALL: f32 = 16.0 * PX;
/// Hitbox is 10 x 10 px (`Projectile.item defaults` type 1: width/height 10).
const HALF: f32 = 5.0 * PX;
/// Arrows are never recovered: `Projectile.Kill` has no ammo item drop for types
/// 1 and 2 (gameplay spec onward; arrow cases at :75466 only spawn dust).
pub const ARROW_RECOVERY_CHANCE: Option<(u32, u32)> = None;
/// gameplay spec: type 954 / 504 spark dies after 60 ticks.
pub const SPARK_TIME_LEFT: i32 = 60;
pub const BOOMERANG_TIME_LEFT: i32 = 1200;
pub const BOOMERANG_MAX_DIST: f32 = 20.0;
pub const BOOMERANG_CATCH_DIST: f32 = 1.2;
pub const SPEAR_REACH: f32 = 3.5;
pub const FLAIL_TIME_LEFT: i32 = 1200;
pub const FLAIL_MAX_DIST: f32 = 12.0;
pub const FLAIL_CATCH_DIST: f32 = 1.2;
/// Valor's yoyo reaches a little less far than a flail.
pub const YOYO_MAX_DIST: f32 = 10.0;
pub const VILETHORN_REACH: f32 = 10.0;
pub const BULLET_TIME_LEFT: i32 = 600;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjKind {
    Arrow,
    FlamingArrow,
    Spark,
    Boomerang,
    /// Flamarang: bounces off the first outward hit, then pierces on the return.
    Flamarang,
    Spear,
    Bullet,
    Flail,
    /// Valor's yoyo: flies out, then returns while piercing.
    Yoyo,
    Vilethorn,
    MagicMissile,
    /// Aqua Scepter's water jet: pierces a handful of enemies.
    WaterStream,
    Fireball,
    DemonScythe,
    Laser,
    EyeLaser,
    Grenade,
    Bomb,
    Dynamite,
    GrappleHook,
    StarfuryStar,
    ThornChakram,
    WaterBolt,
    SpaceLaser,
    FallenStarShot,
    FlowerOfFire,
    Flamelash,
    BeeShot,
}

impl ProjKind {
    /// The ammo item that fires this projectile.
    pub fn of_ammo(ammo: ItemId) -> Option<ProjKind> {
        match ammo {
            WOODEN_ARROW => Some(ProjKind::Arrow),
            FLAMING_ARROW => Some(ProjKind::FlamingArrow),
            crate::item::MUSKET_BALL => Some(ProjKind::Bullet),
            FALLEN_STAR => Some(ProjKind::FallenStarShot),
            _ => None,
        }
    }

    /// How many enemies this projectile can damage before it is spent.
    /// `None` means unlimited (`Spear`, `Vilethorn`, `Flail`, `Yoyo`, `Flamarang`).
    /// `Some(1)` is a single-hit projectile: it ignores `NPC_HIT_COOLDOWN`, so
    /// rapid fire (Clockwork burst, Phoenix Blaster) still lands every shot.
    pub fn pierce_cap(self) -> Option<u32> {
        match self {
            ProjKind::Spark | ProjKind::StarfuryStar => Some(2),
            ProjKind::Laser | ProjKind::SpaceLaser => Some(3),
            ProjKind::WaterStream => Some(5),
            ProjKind::FallenStarShot => Some(6),
            ProjKind::WaterBolt => Some(10),
            ProjKind::Spear
            | ProjKind::Vilethorn
            | ProjKind::Flail
            | ProjKind::Yoyo
            | ProjKind::Flamarang
            | ProjKind::ThornChakram
            | ProjKind::Bomb
            | ProjKind::Dynamite
            | ProjKind::GrappleHook => None,
            ProjKind::Arrow
            | ProjKind::FlamingArrow
            | ProjKind::Boomerang
            | ProjKind::Bullet
            | ProjKind::MagicMissile
            | ProjKind::Flamelash
            | ProjKind::Fireball
            | ProjKind::FlowerOfFire
            | ProjKind::DemonScythe
            | ProjKind::EyeLaser
            | ProjKind::Grenade
            | ProjKind::BeeShot => Some(1),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Projectile {
    pub id: u32,
    /// Centre, blocks.
    pub pos: [f32; 3],
    /// Blocks per tick.
    pub vel: [f32; 3],
    pub kind: ProjKind,
    pub owner: u32,
    /// Ticks left (`timeLeft`).
    pub ttl: i32,
    pub damage: i32,
    pub knock_back: f32,
    pub crit: i32,
    /// `ai[0]`: ticks flown, capped at `GRAVITY_DELAY` (also the flight
    /// phase for boomerangs/flails: 0 = outward, 1 = returning).
    pub ai0: i32,
    /// Enemies damaged so far, for [`ProjKind::pierce_cap`].
    pub pierced: u32,
    /// Spawn origin point.
    pub origin: [f32; 3],
    /// Armor penetration (e.g. Shark Tooth Necklace +5).
    pub armor_penetration: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Flying,
    HitBlock,
    Expired,
}

impl Projectile {
    /// Fires `ammo` from `bow` along unit `dir`. Damage, knockback and speed add the
    /// bow's and the ammo's (Player.PickAmmo: `speed += item.shootSpeed`, damage and
    /// `KnockBack += item.knockBack`, gameplay spec). The bow's own
    /// contribution is scaled by its `prefix`. `None` for non-ammo.
    pub fn fire(
        id: u32,
        owner: u32,
        bow: ItemId,
        ammo: ItemId,
        prefix: Prefix,
        origin: [f32; 3],
        dir: [f32; 3],
    ) -> Option<Projectile> {
        let kind = if bow == MOLTEN_FURY && ammo == WOODEN_ARROW {
            // Molten Fury turns Wooden Arrows into Flaming Arrows for free
            // (the arrow is still consumed once); other arrows keep their kind.
            ProjKind::FlamingArrow
        } else {
            ProjKind::of_ammo(ammo)?
        };
        let (b, a) = (item_data(bow), item_data(ammo));
        let ps = prefix_stats(prefix);
        // Player.PickAmmo: speed = gun.shootSpeed + ammo.shootSpeed, for bullets too.
        let speed = (b.shoot_speed * ps.velocity + a.shoot_speed) * PX;
        let ttl = if kind == ProjKind::Bullet {
            BULLET_TIME_LEFT
        } else if kind == ProjKind::FallenStarShot {
            240
        } else {
            ARROW_TIME_LEFT
        };
        Some(Projectile {
            id,
            pos: origin,
            vel: [dir[0] * speed, dir[1] * speed, dir[2] * speed],
            kind,
            owner,
            ttl,
            damage: (b.damage as f32 * ps.damage).round_ties_even() as i32 + a.damage,
            knock_back: b.knock_back * ps.knock_back + a.knock_back,
            crit: b.crit + ps.crit,
            ai0: 0,
            pierced: 0,
            origin,
            armor_penetration: 0,
        })
    }

    /// Convenience for the wooden bow with no prefix.
    pub fn fire_from_bow(
        id: u32,
        owner: u32,
        ammo: ItemId,
        origin: [f32; 3],
        dir: [f32; 3],
    ) -> Option<Projectile> {
        Self::fire(id, owner, WOODEN_BOW, ammo, Prefix(0), origin, dir)
    }

    fn cast_magic_proj(
        id: u32,
        owner: u32,
        wand: ItemId,
        prefix: Prefix,
        origin: [f32; 3],
        dir: [f32; 3],
        vitals: &mut crate::player::Vitals,
        kind: ProjKind,
        ttl: i32,
    ) -> Option<Projectile> {
        Self::cast_magic_proj_with_mana(
            id, owner, wand, prefix, origin, dir, vitals, kind, ttl, None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn cast_magic_proj_with_mana(
        id: u32,
        owner: u32,
        wand: ItemId,
        prefix: Prefix,
        origin: [f32; 3],
        dir: [f32; 3],
        vitals: &mut crate::player::Vitals,
        kind: ProjKind,
        ttl: i32,
        mana_override: Option<i32>,
    ) -> Option<Projectile> {
        let data = item_data(wand);
        let ps = prefix_stats(prefix);
        let mana = mana_override.unwrap_or_else(|| crate::item::mana_cost(data.mana, prefix));
        if data.class != crate::item::ItemClass::Magic || !vitals.spend_mana(mana) {
            return None;
        }
        let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
        let norm = if len > 1e-4 {
            [dir[0] / len, dir[1] / len, dir[2] / len]
        } else {
            [1.0, 0.0, 0.0]
        };
        let speed = data.shoot_speed * ps.velocity * PX;
        Some(Projectile {
            id,
            pos: origin,
            vel: [norm[0] * speed, norm[1] * speed, norm[2] * speed],
            kind,
            owner,
            ttl,
            damage: (data.damage as f32 * ps.damage).round_ties_even() as i32,
            knock_back: data.knock_back * ps.knock_back,
            crit: data.crit + ps.crit,
            ai0: 0,
            pierced: 0,
            origin,
            armor_penetration: 0,
        })
    }

    pub fn hit_enemies_detailed_guard(kind: ProjKind) -> bool {
        matches!(
            kind,
            ProjKind::Bomb | ProjKind::Dynamite | ProjKind::GrappleHook
        )
    }

    /// Throws a Bomb, Dynamite, or Grenade along `dir`.
    pub fn throw_explosive(
        id: u32,
        owner: u32,
        item: ItemId,
        origin: [f32; 3],
        dir: [f32; 3],
    ) -> Projectile {
        let data = item_data(item);
        let speed = data.shoot_speed * PX;
        let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2])
            .sqrt()
            .max(1e-6);
        let nd = [dir[0] / len, dir[1] / len, dir[2] / len];
        let kind = match item {
            crate::item::DYNAMITE => ProjKind::Dynamite,
            crate::item::GRENADE => ProjKind::Grenade,
            _ => ProjKind::Bomb,
        };
        Projectile {
            id,
            pos: origin,
            vel: [nd[0] * speed, nd[1] * speed + 0.14, nd[2] * speed],
            kind,
            owner,
            ttl: 180,
            damage: data.damage,
            knock_back: data.knock_back,
            crit: data.crit,
            ai0: 0,
            pierced: 0,
            origin,
            armor_penetration: 0,
        }
    }

    /// Fires a Grappling Hook along `dir`.
    pub fn fire_grapple(id: u32, owner: u32, origin: [f32; 3], dir: [f32; 3]) -> Projectile {
        let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2])
            .sqrt()
            .max(1e-6);
        let speed = 1.35;
        Projectile {
            id,
            pos: origin,
            vel: [
                dir[0] / len * speed,
                dir[1] / len * speed,
                dir[2] / len * speed,
            ],
            kind: ProjKind::GrappleHook,
            owner,
            ttl: 60,
            damage: 0,
            knock_back: 0.0,
            crit: 0,
            ai0: 0,
            pierced: 0,
            origin,
            armor_penetration: 0,
        }
    }

    /// Spawns a Starfury falling star 22 blocks above `target_pos`.
    pub fn spawn_starfury_star(
        id: u32,
        owner: u32,
        damage: i32,
        knock_back: f32,
        crit: i32,
        target_pos: [f32; 3],
    ) -> Projectile {
        Projectile {
            id,
            pos: [target_pos[0], target_pos[1] + 22.0, target_pos[2]],
            vel: [0.0, -1.25, 0.0],
            kind: ProjKind::StarfuryStar,
            owner,
            ttl: 45,
            damage,
            knock_back,
            crit,
            ai0: 0,
            pierced: 0,
            origin: target_pos,
            armor_penetration: 0,
        }
    }

    /// Cast a Spark projectile from a wand.
    pub fn cast_spark(
        id: u32,
        owner: u32,
        wand: ItemId,
        prefix: Prefix,
        origin: [f32; 3],
        dir: [f32; 3],
        vitals: &mut crate::player::Vitals,
    ) -> Option<Projectile> {
        Self::cast_magic_proj(
            id,
            owner,
            wand,
            prefix,
            origin,
            dir,
            vitals,
            ProjKind::Spark,
            SPARK_TIME_LEFT,
        )
    }

    /// Cast a Magic Missile projectile.
    pub fn cast_magic_missile(
        id: u32,
        owner: u32,
        wand: ItemId,
        prefix: Prefix,
        origin: [f32; 3],
        dir: [f32; 3],
        vitals: &mut crate::player::Vitals,
    ) -> Option<Projectile> {
        Self::cast_magic_proj(
            id,
            owner,
            wand,
            prefix,
            origin,
            dir,
            vitals,
            ProjKind::MagicMissile,
            300,
        )
    }

    /// Cast an Aqua Scepter water stream projectile: a short, gently arcing jet
    /// that pierces up to five enemies.
    pub fn cast_aqua_scepter(
        id: u32,
        owner: u32,
        wand: ItemId,
        prefix: Prefix,
        origin: [f32; 3],
        dir: [f32; 3],
        vitals: &mut crate::player::Vitals,
    ) -> Option<Projectile> {
        let mut p = Self::cast_magic_proj(
            id,
            owner,
            wand,
            prefix,
            origin,
            dir,
            vitals,
            ProjKind::WaterStream,
            45,
        )?;
        p.ai0 = -1;
        Some(p)
    }

    /// Cast a Laser Rifle projectile: fast straight line, `extraUpdates = 2`
    /// approximated by doubling its velocity, piercing three enemies.
    pub fn cast_laser(
        id: u32,
        owner: u32,
        wand: ItemId,
        prefix: Prefix,
        origin: [f32; 3],
        dir: [f32; 3],
        vitals: &mut crate::player::Vitals,
    ) -> Option<Projectile> {
        let mut p = Self::cast_magic_proj(
            id,
            owner,
            wand,
            prefix,
            origin,
            dir,
            vitals,
            ProjKind::Laser,
            180,
        )?;
        for v in &mut p.vel {
            *v *= 2.0;
        }
        Some(p)
    }

    /// Cast a Water Bolt projectile: bounces off solid walls up to 5 times.
    pub fn cast_water_bolt(
        id: u32,
        owner: u32,
        wand: ItemId,
        prefix: Prefix,
        origin: [f32; 3],
        dir: [f32; 3],
        vitals: &mut crate::player::Vitals,
        mana_override: Option<i32>,
    ) -> Option<Projectile> {
        Self::cast_magic_proj_with_mana(
            id,
            owner,
            wand,
            prefix,
            origin,
            dir,
            vitals,
            ProjKind::WaterBolt,
            600,
            mana_override,
        )
    }

    /// Cast a Space Gun laser projectile.
    pub fn cast_space_gun(
        id: u32,
        owner: u32,
        wand: ItemId,
        prefix: Prefix,
        origin: [f32; 3],
        dir: [f32; 3],
        vitals: &mut crate::player::Vitals,
        mana_override: Option<i32>,
    ) -> Option<Projectile> {
        let mut p = Self::cast_magic_proj_with_mana(
            id,
            owner,
            wand,
            prefix,
            origin,
            dir,
            vitals,
            ProjKind::SpaceLaser,
            180,
            mana_override,
        )?;
        for v in &mut p.vel {
            *v *= 2.0;
        }
        Some(p)
    }

    /// Cast a Flamelash controllable fire missile.
    pub fn cast_flamelash(
        id: u32,
        owner: u32,
        wand: ItemId,
        prefix: Prefix,
        origin: [f32; 3],
        dir: [f32; 3],
        vitals: &mut crate::player::Vitals,
        mana_override: Option<i32>,
    ) -> Option<Projectile> {
        Self::cast_magic_proj_with_mana(
            id,
            owner,
            wand,
            prefix,
            origin,
            dir,
            vitals,
            ProjKind::Flamelash,
            300,
            mana_override,
        )
    }

    /// Cast a Flower of Fire bouncing fireball.
    pub fn cast_flower_of_fire(
        id: u32,
        owner: u32,
        wand: ItemId,
        prefix: Prefix,
        origin: [f32; 3],
        dir: [f32; 3],
        vitals: &mut crate::player::Vitals,
        mana_override: Option<i32>,
    ) -> Option<Projectile> {
        Self::cast_magic_proj_with_mana(
            id,
            owner,
            wand,
            prefix,
            origin,
            dir,
            vitals,
            ProjKind::FlowerOfFire,
            300,
            mana_override,
        )
    }

    /// Cast a Bee Gun homing bee projectile.
    pub fn cast_bee_gun(
        id: u32,
        owner: u32,
        wand: ItemId,
        prefix: Prefix,
        origin: [f32; 3],
        dir: [f32; 3],
        vitals: &mut crate::player::Vitals,
        mana_override: Option<i32>,
    ) -> Option<Projectile> {
        Self::cast_magic_proj_with_mana(
            id,
            owner,
            wand,
            prefix,
            origin,
            dir,
            vitals,
            ProjKind::BeeShot,
            240,
            mana_override,
        )
    }

    /// Spawns a friendly homing bee projectile (from Beenade, Bee Keeper, or Honey Comb).
    pub fn spawn_bee(id: u32, owner: u32, damage: i32, origin: [f32; 3], dir: [f32; 3]) -> Self {
        let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2])
            .sqrt()
            .max(1e-4);
        let speed = 8.0 * PX;
        Projectile {
            id,
            pos: origin,
            vel: [
                dir[0] / len * speed,
                dir[1] / len * speed,
                dir[2] / len * speed,
            ],
            kind: ProjKind::BeeShot,
            owner,
            ttl: 240,
            damage,
            knock_back: 0.5,
            crit: 4,
            ai0: 0,
            pierced: 0,
            origin,
            armor_penetration: 0,
        }
    }

    /// Throws an enchanted boomerang.
    pub fn throw_boomerang(
        id: u32,
        owner: u32,
        item: ItemId,
        origin: [f32; 3],
        dir: [f32; 3],
    ) -> Option<Projectile> {
        let data = item_data(item);
        if data.class != crate::item::ItemClass::Melee {
            return None;
        }
        let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
        let norm = if len > 1e-4 {
            [dir[0] / len, dir[1] / len, dir[2] / len]
        } else {
            [1.0, 0.0, 0.0]
        };
        let speed = data.shoot_speed * PX;
        Some(Projectile {
            id,
            pos: origin,
            vel: [norm[0] * speed, norm[1] * speed, norm[2] * speed],
            kind: if item == FLAMARANG {
                ProjKind::Flamarang
            } else if item == THORN_CHAKRAM {
                ProjKind::ThornChakram
            } else {
                ProjKind::Boomerang
            },
            owner,
            ttl: BOOMERANG_TIME_LEFT,
            damage: data.damage,
            knock_back: data.knock_back,
            crit: data.crit,
            ai0: 0,
            pierced: 0,
            origin,
            armor_penetration: 0,
        })
    }

    /// Thrusts a spear.
    pub fn thrust_spear(
        id: u32,
        owner: u32,
        item: ItemId,
        origin: [f32; 3],
        dir: [f32; 3],
    ) -> Option<Projectile> {
        let data = item_data(item);
        if data.class != crate::item::ItemClass::Melee {
            return None;
        }
        let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
        let norm = if len > 1e-4 {
            [dir[0] / len, dir[1] / len, dir[2] / len]
        } else {
            [1.0, 0.0, 0.0]
        };
        let ttl = data.use_animation.max(1);
        Some(Projectile {
            id,
            pos: origin,
            vel: norm,
            kind: ProjKind::Spear,
            owner,
            ttl,
            damage: data.damage,
            knock_back: data.knock_back,
            crit: data.crit,
            ai0: 0,
            pierced: 0,
            origin,
            armor_penetration: 0,
        })
    }

    /// Throws a flail (Ball O' Hurt, Blue Moon, Sunfury) or yoyo (Valor, Amazon).
    pub fn throw_flail(
        id: u32,
        owner: u32,
        item: ItemId,
        origin: [f32; 3],
        dir: [f32; 3],
    ) -> Option<Projectile> {
        let data = item_data(item);
        if data.class != crate::item::ItemClass::Melee {
            return None;
        }
        let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
        let norm = if len > 1e-4 {
            [dir[0] / len, dir[1] / len, dir[2] / len]
        } else {
            [1.0, 0.0, 0.0]
        };
        let speed = data.shoot_speed * PX;
        Some(Projectile {
            id,
            pos: origin,
            vel: [norm[0] * speed, norm[1] * speed, norm[2] * speed],
            kind: if item == VALOR || item == AMAZON {
                ProjKind::Yoyo
            } else {
                ProjKind::Flail
            },
            owner,
            ttl: FLAIL_TIME_LEFT,
            damage: data.damage,
            knock_back: data.knock_back,
            crit: data.crit,
            ai0: 0,
            pierced: 0,
            origin,
            armor_penetration: 0,
        })
    }

    /// Casts Vilethorn: piercing magic spear extending through blocks.
    pub fn cast_vilethorn(
        id: u32,
        owner: u32,
        wand: ItemId,
        prefix: Prefix,
        origin: [f32; 3],
        dir: [f32; 3],
        vitals: &mut crate::player::Vitals,
    ) -> Option<Projectile> {
        let data = item_data(wand);
        let ps = prefix_stats(prefix);
        let mana = crate::item::mana_cost(data.mana, prefix);
        if data.class != crate::item::ItemClass::Magic || !vitals.spend_mana(mana) {
            return None;
        }
        let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
        let norm = if len > 1e-4 {
            [dir[0] / len, dir[1] / len, dir[2] / len]
        } else {
            [1.0, 0.0, 0.0]
        };
        let ttl = data.use_animation.max(1);
        Some(Projectile {
            id,
            pos: origin,
            vel: norm,
            kind: ProjKind::Vilethorn,
            owner,
            ttl,
            damage: (data.damage as f32 * ps.damage).round_ties_even() as i32,
            knock_back: data.knock_back * ps.knock_back,
            crit: data.crit + ps.crit,
            ai0: 0,
            pierced: 0,
            origin,
            armor_penetration: 0,
        })
    }
}

/// Steers a `MagicMissile` or `Flamelash` smoothly toward `aim_point` while preserving its speed.
pub fn steer_magic_missile(p: &mut Projectile, aim_point: [f32; 3]) {
    if !matches!(p.kind, ProjKind::MagicMissile | ProjKind::Flamelash) {
        return;
    }
    let dx = aim_point[0] - p.pos[0];
    let dy = aim_point[1] - p.pos[1];
    let dz = aim_point[2] - p.pos[2];
    let dist = (dx * dx + dy * dy + dz * dz).sqrt();
    if dist <= 0.25 {
        return;
    }
    let cur_speed = (p.vel[0] * p.vel[0] + p.vel[1] * p.vel[1] + p.vel[2] * p.vel[2]).sqrt();
    let speed = if cur_speed > 1e-4 {
        cur_speed
    } else {
        item_data(MAGIC_MISSILE).shoot_speed * PX
    };
    let desired = [dx / dist * speed, dy / dist * speed, dz / dist * speed];
    let blended = [
        p.vel[0] + (desired[0] - p.vel[0]) * 0.2,
        p.vel[1] + (desired[1] - p.vel[1]) * 0.2,
        p.vel[2] + (desired[2] - p.vel[2]) * 0.2,
    ];
    let blen = (blended[0] * blended[0] + blended[1] * blended[1] + blended[2] * blended[2]).sqrt();
    if blen > 1e-4 {
        p.vel = [
            blended[0] / blen * speed,
            blended[1] / blen * speed,
            blended[2] / blen * speed,
        ];
    }
}

/// One tick of `AI_001` plus movement. Solid blocks stop the arrow; the path is
/// sampled in sub-steps so a fast arrow cannot tunnel through a block.
pub fn step(q: &impl BlockQuery, p: &mut Projectile) -> Step {
    step_with_target(q, p, p.origin)
}

/// Stepping with an explicit target position (such as player position for returning boomerangs or anchored spears).
pub fn step_with_target(q: &impl BlockQuery, p: &mut Projectile, target: [f32; 3]) -> Step {
    p.ttl -= 1;
    if p.ttl <= 0 {
        return Step::Expired;
    }

    match p.kind {
        ProjKind::Spear => {
            p.ai0 += 1;
            let total = p.ai0 + p.ttl;
            let half = total as f32 / 2.0;
            let progress = if (p.ai0 as f32) <= half {
                p.ai0 as f32 / half
            } else {
                (total as f32 - p.ai0 as f32) / half
            };
            let ext = progress.max(0.0) * SPEAR_REACH;
            p.origin = target;
            p.pos = [
                target[0] + p.vel[0] * ext,
                target[1] + p.vel[1] * ext,
                target[2] + p.vel[2] * ext,
            ];
            Step::Flying
        }
        ProjKind::Boomerang | ProjKind::Flamarang | ProjKind::ThornChakram => {
            let dx = p.pos[0] - target[0];
            let dy = p.pos[1] - target[1];
            let dz = p.pos[2] - target[2];
            let dist_to_target = (dx * dx + dy * dy + dz * dz).sqrt();

            if p.ai0 == 0 {
                if dist_to_target >= BOOMERANG_MAX_DIST {
                    p.ai0 = 1;
                } else {
                    let len = p.vel.iter().map(|v| v * v).sum::<f32>().sqrt();
                    let n = (len / 0.2).ceil().max(1.0) as i32;
                    for _ in 0..n {
                        let next = [
                            p.pos[0] + p.vel[0] / n as f32,
                            p.pos[1] + p.vel[1] / n as f32,
                            p.pos[2] + p.vel[2] / n as f32,
                        ];
                        if q.block(
                            next[0].floor() as i32,
                            next[1].floor() as i32,
                            next[2].floor() as i32,
                        )
                        .is_solid()
                        {
                            p.ai0 = 1;
                            break;
                        }
                        p.pos = next;
                    }
                }
            }

            if p.ai0 == 1 {
                if dist_to_target <= BOOMERANG_CATCH_DIST {
                    return Step::Expired;
                }
                let to_target = [
                    target[0] - p.pos[0],
                    target[1] - p.pos[1],
                    target[2] - p.pos[2],
                ];
                let d = (to_target[0] * to_target[0]
                    + to_target[1] * to_target[1]
                    + to_target[2] * to_target[2])
                    .sqrt();
                if d > 1e-4 {
                    let return_speed = 10.0 * PX;
                    let desired = [
                        to_target[0] / d * return_speed,
                        to_target[1] / d * return_speed,
                        to_target[2] / d * return_speed,
                    ];
                    p.vel[0] += (desired[0] - p.vel[0]) * 0.25;
                    p.vel[1] += (desired[1] - p.vel[1]) * 0.25;
                    p.vel[2] += (desired[2] - p.vel[2]) * 0.25;
                }
                p.pos[0] += p.vel[0];
                p.pos[1] += p.vel[1];
                p.pos[2] += p.vel[2];

                let new_dx = p.pos[0] - target[0];
                let new_dy = p.pos[1] - target[1];
                let new_dz = p.pos[2] - target[2];
                let new_dist = (new_dx * new_dx + new_dy * new_dy + new_dz * new_dz).sqrt();
                if new_dist <= BOOMERANG_CATCH_DIST {
                    return Step::Expired;
                }
            }
            Step::Flying
        }
        ProjKind::Vilethorn => {
            p.ai0 += 1;
            let total = p.ai0 + p.ttl;
            let progress = (p.ai0 as f32 / total as f32).min(1.0);
            let ext = progress * VILETHORN_REACH;
            p.pos = [
                p.origin[0] + p.vel[0] * ext,
                p.origin[1] + p.vel[1] * ext,
                p.origin[2] + p.vel[2] * ext,
            ];
            Step::Flying
        }
        ProjKind::Flail | ProjKind::Yoyo => {
            let max_dist = if p.kind == ProjKind::Yoyo {
                YOYO_MAX_DIST
            } else {
                FLAIL_MAX_DIST
            };
            let catch_dist = FLAIL_CATCH_DIST;
            let dx = p.pos[0] - target[0];
            let dy = p.pos[1] - target[1];
            let dz = p.pos[2] - target[2];
            let dist_to_target = (dx * dx + dy * dy + dz * dz).sqrt();

            if p.ai0 == 0 {
                if dist_to_target >= max_dist {
                    p.ai0 = 1;
                } else {
                    let len = p.vel.iter().map(|v| v * v).sum::<f32>().sqrt();
                    let n = (len / 0.2).ceil().max(1.0) as i32;
                    for _ in 0..n {
                        let next = [
                            p.pos[0] + p.vel[0] / n as f32,
                            p.pos[1] + p.vel[1] / n as f32,
                            p.pos[2] + p.vel[2] / n as f32,
                        ];
                        if q.block(
                            next[0].floor() as i32,
                            next[1].floor() as i32,
                            next[2].floor() as i32,
                        )
                        .is_solid()
                        {
                            p.ai0 = 1;
                            break;
                        }
                        p.pos = next;
                    }
                }
            }

            if p.ai0 == 1 {
                if dist_to_target <= catch_dist {
                    return Step::Expired;
                }
                let to_target = [
                    target[0] - p.pos[0],
                    target[1] - p.pos[1],
                    target[2] - p.pos[2],
                ];
                let d = (to_target[0] * to_target[0]
                    + to_target[1] * to_target[1]
                    + to_target[2] * to_target[2])
                    .sqrt();
                if d > 1e-4 {
                    let return_speed = 16.0 * PX;
                    let desired = [
                        to_target[0] / d * return_speed,
                        to_target[1] / d * return_speed,
                        to_target[2] / d * return_speed,
                    ];
                    p.vel[0] += (desired[0] - p.vel[0]) * 0.25;
                    p.vel[1] += (desired[1] - p.vel[1]) * 0.25;
                    p.vel[2] += (desired[2] - p.vel[2]) * 0.25;
                }
                p.pos[0] += p.vel[0];
                p.pos[1] += p.vel[1];
                p.pos[2] += p.vel[2];

                let new_dx = p.pos[0] - target[0];
                let new_dy = p.pos[1] - target[1];
                let new_dz = p.pos[2] - target[2];
                let new_dist = (new_dx * new_dx + new_dy * new_dy + new_dz * new_dz).sqrt();
                if new_dist <= catch_dist {
                    return Step::Expired;
                }
            }
            Step::Flying
        }
        ProjKind::GrappleHook => {
            if p.ai0 == 1 {
                p.ttl = 600;
                return Step::Flying;
            }
            let dx = p.pos[0] - target[0];
            let dy = p.pos[1] - target[1];
            let dz = p.pos[2] - target[2];
            if dx * dx + dy * dy + dz * dz >= 24.0 * 24.0 {
                return Step::Expired;
            }
            let len = p.vel.iter().map(|v| v * v).sum::<f32>().sqrt();
            let n = (len / 0.25).ceil().max(1.0) as i32;
            for _ in 0..n {
                let next = [
                    p.pos[0] + p.vel[0] / n as f32,
                    p.pos[1] + p.vel[1] / n as f32,
                    p.pos[2] + p.vel[2] / n as f32,
                ];
                if q.block(
                    next[0].floor() as i32,
                    next[1].floor() as i32,
                    next[2].floor() as i32,
                )
                .is_solid()
                {
                    p.ai0 = 1;
                    p.vel = [0.0; 3];
                    p.ttl = 600;
                    return Step::Flying;
                }
                p.pos = next;
            }
            Step::Flying
        }
        ProjKind::Bomb | ProjKind::Dynamite | ProjKind::Grenade => {
            p.vel[1] = (p.vel[1] - ARROW_GRAVITY * 1.4).max(-ARROW_MAX_FALL);
            for axis in 0..3 {
                let mut next = p.pos;
                next[axis] += p.vel[axis];
                if q.block(
                    next[0].floor() as i32,
                    next[1].floor() as i32,
                    next[2].floor() as i32,
                )
                .is_solid()
                {
                    if axis == 1 {
                        p.vel[1] = -p.vel[1] * 0.35;
                        p.vel[0] *= 0.72;
                        p.vel[2] *= 0.72;
                    } else {
                        p.vel[axis] = -p.vel[axis] * 0.5;
                    }
                } else {
                    p.pos[axis] = next[axis];
                }
            }
            Step::Flying
        }
        ProjKind::StarfuryStar => {
            p.pos[0] += p.vel[0];
            p.pos[1] += p.vel[1];
            p.pos[2] += p.vel[2];
            if p.pos[1] <= p.origin[1] - 2.0
                && q.block(
                    p.pos[0].floor() as i32,
                    p.pos[1].floor() as i32,
                    p.pos[2].floor() as i32,
                )
                .is_solid()
            {
                return Step::HitBlock;
            }
            Step::Flying
        }
        ProjKind::WaterBolt | ProjKind::FlowerOfFire => {
            if p.kind == ProjKind::FlowerOfFire {
                p.vel[1] = (p.vel[1] - 0.65 * ARROW_GRAVITY).max(-ARROW_MAX_FALL);
            }
            let len = p.vel.iter().map(|v| v * v).sum::<f32>().sqrt();
            let n = (len / 0.2).ceil().max(1.0) as i32;
            for _ in 0..n {
                let step_v = [
                    p.vel[0] / n as f32,
                    p.vel[1] / n as f32,
                    p.vel[2] / n as f32,
                ];
                let next = [
                    p.pos[0] + step_v[0],
                    p.pos[1] + step_v[1],
                    p.pos[2] + step_v[2],
                ];
                if q.block(
                    next[0].floor() as i32,
                    next[1].floor() as i32,
                    next[2].floor() as i32,
                )
                .is_solid()
                {
                    if p.ai0 >= 5 {
                        return Step::HitBlock;
                    }
                    p.ai0 += 1;
                    let cx = p.pos[0].floor() as i32;
                    let cy = p.pos[1].floor() as i32;
                    let cz = p.pos[2].floor() as i32;
                    if q.block((p.pos[0] + step_v[0]).floor() as i32, cy, cz)
                        .is_solid()
                    {
                        p.vel[0] = -p.vel[0];
                    }
                    if q.block(cx, (p.pos[1] + step_v[1]).floor() as i32, cz)
                        .is_solid()
                    {
                        p.vel[1] = -p.vel[1];
                    }
                    if q.block(cx, cy, (p.pos[2] + step_v[2]).floor() as i32)
                        .is_solid()
                    {
                        p.vel[2] = -p.vel[2];
                    }
                    break;
                }
                p.pos = next;
            }
            Step::Flying
        }
        ProjKind::Arrow
        | ProjKind::FlamingArrow
        | ProjKind::Spark
        | ProjKind::Bullet
        | ProjKind::MagicMissile
        | ProjKind::Flamelash
        | ProjKind::WaterStream
        | ProjKind::Fireball
        | ProjKind::DemonScythe
        | ProjKind::Laser
        | ProjKind::SpaceLaser
        | ProjKind::FallenStarShot
        | ProjKind::BeeShot
        | ProjKind::EyeLaser => {
            if p.kind == ProjKind::DemonScythe {
                let speed = p.vel.iter().map(|v| v * v).sum::<f32>().sqrt();
                if speed > 1e-5 && speed < 0.55 {
                    let scale = (1.06_f32).min(0.55 / speed);
                    for v in &mut p.vel {
                        *v *= scale;
                    }
                }
            } else if p.kind == ProjKind::Spark {
                // Wand of Sparking: a lobbed spark, so gravity applies at once.
                p.vel[1] = (p.vel[1] - 0.5 * ARROW_GRAVITY).max(-ARROW_MAX_FALL);
            } else if matches!(p.kind, ProjKind::MagicMissile | ProjKind::WaterStream) && p.ai0 < 0
            {
                // Aqua Scepter's jet and the Magic Missile arc gently.
                p.vel[1] = (p.vel[1] - 0.5 * ARROW_GRAVITY).max(-ARROW_MAX_FALL);
            } else if !matches!(
                p.kind,
                ProjKind::Bullet
                    | ProjKind::MagicMissile
                    | ProjKind::Flamelash
                    | ProjKind::WaterStream
                    | ProjKind::Fireball
                    | ProjKind::DemonScythe
                    | ProjKind::Laser
                    | ProjKind::SpaceLaser
                    | ProjKind::FallenStarShot
                    | ProjKind::BeeShot
                    | ProjKind::EyeLaser
            ) {
                p.ai0 += 1;
                if p.ai0 >= GRAVITY_DELAY {
                    p.ai0 = GRAVITY_DELAY;
                    p.vel[1] = (p.vel[1] - ARROW_GRAVITY).max(-ARROW_MAX_FALL);
                }
            }
            let len = p.vel.iter().map(|v| v * v).sum::<f32>().sqrt();
            let n = (len / 0.2).ceil().max(1.0) as i32;
            for _ in 0..n {
                let next = [
                    p.pos[0] + p.vel[0] / n as f32,
                    p.pos[1] + p.vel[1] / n as f32,
                    p.pos[2] + p.vel[2] / n as f32,
                ];
                if q.block(
                    next[0].floor() as i32,
                    next[1].floor() as i32,
                    next[2].floor() as i32,
                )
                .is_solid()
                {
                    return Step::HitBlock;
                }
                p.pos = next;
            }
            Step::Flying
        }
    }
}

fn touches_spear(origin: [f32; 3], tip: [f32; 3], e: &Enemy) -> bool {
    let h = e.kind.stats().half;
    for step in 0..=10 {
        let t = step as f32 / 10.0;
        let pt = [
            origin[0] + (tip[0] - origin[0]) * t,
            origin[1] + (tip[1] - origin[1]) * t,
            origin[2] + (tip[2] - origin[2]) * t,
        ];
        if (pt[0] - e.pos[0]).abs() < h[0] + HALF
            && (pt[2] - e.pos[2]).abs() < h[0] + HALF
            && pt[1] + HALF > e.pos[1]
            && pt[1] - HALF < e.pos[1] + 2.0 * h[1]
        {
            return true;
        }
    }
    false
}

/// Outcome of a projectile hitting one enemy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjHit {
    pub id: EnemyId,
    pub hit: Hit,
}

/// Hits enemies touching the projectile. Piercing weapons (spear) hit all touching enemies.
pub fn hit_enemies(p: &mut Projectile, enemies: &mut [Enemy], rng: &mut Rng) -> Vec<EnemyId> {
    hit_enemies_detailed(p, enemies, rng)
        .into_iter()
        .map(|h| h.id)
        .collect()
}

/// Hits enemies touching the projectile and returns damage/crit/kill outcomes.
///
/// Single-hit kinds (see [`ProjKind::pierce_cap`]) ignore the NPC hit cooldown,
/// so every shot of a burst lands; multi-hit kinds respect it. Piercing kinds
/// keep flying until `pierced` reaches their cap.
pub fn hit_enemies_detailed(
    p: &mut Projectile,
    enemies: &mut [Enemy],
    rng: &mut Rng,
) -> Vec<ProjHit> {
    if Projectile::hit_enemies_detailed_guard(p.kind) {
        return Vec::new();
    }
    let mut hits = Vec::new();
    let cap = p.kind.pierce_cap();
    let ignores_immunity = cap == Some(1);
    for e in enemies.iter_mut() {
        if e.hp <= 0 || e.kind.is_friendly() {
            continue;
        }
        let touched = if p.kind == ProjKind::Spear || p.kind == ProjKind::Vilethorn {
            touches_spear(p.origin, p.pos, e)
        } else {
            touches(p, e)
        };
        if touched {
            let e_def = e.kind.stats().defense;
            let eff_damage = p.damage
                + if p.armor_penetration > 0 {
                    e_def / 2 - (e_def - p.armor_penetration).max(0) / 2
                } else {
                    0
                };
            if let Some(hit) = apply_hit_with_immunity(
                e,
                p.pos,
                eff_damage,
                p.knock_back,
                p.crit,
                rng,
                ignores_immunity,
            ) {
                hits.push(ProjHit { id: e.id, hit });
                p.pierced += 1;
                if matches!(
                    p.kind,
                    ProjKind::FlamingArrow | ProjKind::Flamelash | ProjKind::FlowerOfFire
                ) {
                    e.apply_on_fire(crate::combat::ON_FIRE_TICKS);
                } else if matches!(p.kind, ProjKind::Flamarang | ProjKind::ThornChakram) {
                    if p.kind == ProjKind::Flamarang && rng.below(100) < 50 {
                        e.apply_on_fire(crate::combat::ON_FIRE_TICKS);
                    }
                    if p.ai0 == 0 {
                        p.ai0 = 1;
                        p.vel = [-p.vel[0], -p.vel[1], -p.vel[2]];
                        break;
                    }
                } else if p.kind == ProjKind::Spark && rng.below(100) < 50 {
                    e.apply_on_fire(crate::combat::ON_FIRE_TICKS);
                }
                if let Some(cap) = cap {
                    if p.pierced >= cap {
                        break;
                    }
                }
            }
        }
    }
    hits
}

/// The enemy box (`pos` feet centre) overlaps the arrow's box.
pub fn touches(p: &Projectile, e: &Enemy) -> bool {
    let h = e.kind.stats().half;
    (p.pos[0] - e.pos[0]).abs() < h[0] + HALF
        && (p.pos[2] - e.pos[2]).abs() < h[0] + HALF
        && p.pos[1] + HALF > e.pos[1]
        && p.pos[1] - HALF < e.pos[1] + 2.0 * h[1]
}

/// Damages the first enemy the arrow touches (arrows have `penetrate = 1`, so the
/// arrow is spent on a hit). Returns the enemy hit, if any.
pub fn hit_enemy(p: &Projectile, enemies: &mut [Enemy], rng: &mut Rng) -> Option<EnemyId> {
    for e in enemies.iter_mut() {
        if e.hp > 0
            && !e.kind.is_friendly()
            && touches(p, e)
            && apply_hit(e, p.pos, p.damage, p.knock_back, p.crit, rng).is_some()
        {
            if p.kind == ProjKind::FlamingArrow {
                e.apply_on_fire(crate::combat::ON_FIRE_TICKS);
            }
            return Some(e.id);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{AIR, BlockId, STONE};
    use crate::enemy::EnemyKind;

    struct Wall(Option<i32>);
    impl BlockQuery for Wall {
        fn block(&self, x: i32, _y: i32, _z: i32) -> BlockId {
            if self.0 == Some(x) { STONE } else { AIR }
        }
    }

    fn arrow() -> Projectile {
        Projectile::fire_from_bow(1, 0, WOODEN_ARROW, [0.0, 50.0, 0.0], [1.0, 0.0, 0.0]).unwrap()
    }

    #[test]
    fn ranged_ids_match_terraria() {
        assert_eq!(
            [WOODEN_BOW.0, WOODEN_ARROW.0, FLAMING_ARROW.0],
            [39, 40, 41]
        ); // gameplay spec, :1626, :1628
        assert_eq!(item_data(WOODEN_BOW).class, crate::item::ItemClass::Ranged);
        assert_eq!(item_data(WOODEN_ARROW).class, crate::item::ItemClass::Ammo);
    }

    #[test]
    fn bow_and_arrow_stats_match_spec() {
        let (b, a, f) = (
            item_data(WOODEN_BOW),
            item_data(WOODEN_ARROW),
            item_data(FLAMING_ARROW),
        );
        // gameplay spec: useTime/useAnimation 30, damage 4, shootSpeed 6.1
        assert_eq!((b.use_time, b.use_animation, b.damage), (30, 30, 4));
        assert_eq!(b.shoot_speed, 6.1);
        // gameplay spec: damage 5, kb 2, shootSpeed 3; :1858-1869: 7, 2, 3.5
        assert_eq!((a.damage, a.knock_back, a.shoot_speed), (5, 2.0, 3.0));
        assert_eq!((f.damage, f.knock_back, f.shoot_speed), (7, 2.0, 3.5));
        let p = arrow();
        assert_eq!((p.damage, p.knock_back), (9, 2.0));
        assert!((p.vel[0] - 9.1 * PX).abs() < 1e-6);
    }

    #[test]
    fn arrow_falls_after_straight_flight() {
        let q = Wall(None);
        let mut p = arrow();
        let y0 = p.pos[1];
        for _ in 0..GRAVITY_DELAY - 1 {
            assert_eq!(step(&q, &mut p), Step::Flying);
            assert_eq!(p.pos[1], y0, "straight before gravity starts");
        }
        step(&q, &mut p);
        assert!(p.vel[1] < 0.0);
        for _ in 0..400 {
            step(&q, &mut p);
        }
        assert!((p.vel[1] + ARROW_MAX_FALL).abs() < 1e-6);
    }

    #[test]
    fn arrow_stops_on_solid_block() {
        let q = Wall(Some(5));
        let mut p = arrow();
        let mut r = Step::Flying;
        for _ in 0..60 {
            r = step(&q, &mut p);
            if r != Step::Flying {
                break;
            }
        }
        assert_eq!(r, Step::HitBlock);
        assert!(p.pos[0] < 5.0);
        // Expires on its own in open air.
        let mut p = arrow();
        p.ttl = 3;
        assert_eq!(step(&Wall(None), &mut p), Step::Flying);
        assert_eq!(step(&Wall(None), &mut p), Step::Flying);
        assert_eq!(step(&Wall(None), &mut p), Step::Expired);
    }

    #[test]
    fn arrow_damages_enemy_once() {
        let mut e = [Enemy::new(EnemyId(1), EnemyKind::Zombie, [0.0, 49.5, 0.0])];
        let hp = e[0].hp;
        let p = arrow();
        let mut rng = Rng::new(3);
        assert_eq!(hit_enemy(&p, &mut e, &mut rng), Some(EnemyId(1)));
        assert!(e[0].hp < hp);
        // The enemy is now immune for a few ticks: the same arrow cannot hit twice.
        let hp = e[0].hp;
        assert_eq!(hit_enemy(&p, &mut e, &mut rng), None);
        assert_eq!(e[0].hp, hp);
    }

    #[test]
    fn flaming_arrow_adds_damage() {
        let w = Projectile::fire_from_bow(1, 0, WOODEN_ARROW, [0.0; 3], [1.0, 0.0, 0.0]).unwrap();
        let f = Projectile::fire_from_bow(2, 0, FLAMING_ARROW, [0.0; 3], [1.0, 0.0, 0.0]).unwrap();
        assert_eq!((w.damage, f.damage), (9, 11));
        assert!(f.vel[0] > w.vel[0]);
        assert_eq!(
            ProjKind::of_ammo(FLAMING_ARROW),
            Some(ProjKind::FlamingArrow)
        );
        assert_eq!(
            Projectile::fire_from_bow(3, 0, WOODEN_BOW, [0.0; 3], [1.0; 3]),
            None
        );
    }

    #[test]
    fn arrow_drop_chance_matches_spec() {
        // Projectile.Kill (gameplay spec) drops no item for types 1 and 2: arrows
        // are always consumed, there is no recovery roll to port.
        assert_eq!(ARROW_RECOVERY_CHANCE, None);
    }

    #[test]
    fn wand_costs_mana() {
        use crate::item::WAND_OF_SPARKING;
        use crate::player::Vitals;
        let mut v = Vitals::new();
        let p = Projectile::cast_spark(
            1,
            0,
            WAND_OF_SPARKING,
            Prefix(0),
            [0.0; 3],
            [1.0, 0.0, 0.0],
            &mut v,
        )
        .expect("20 mana pays the 2 mana cost");
        assert_eq!((v.mana, v.mana_max), (18, 20));
        assert!(v.mana_regen_delay > 0.0, "spending starts the regen delay");
        assert_eq!(p.kind, ProjKind::Spark);
        assert_eq!((p.damage, p.crit, p.ttl), (14, 10, SPARK_TIME_LEFT));
        assert!((p.vel[0] - 7.0 * PX).abs() < 1e-6);
    }

    #[test]
    fn wand_does_not_fire_without_mana() {
        use crate::item::WAND_OF_SPARKING;
        use crate::player::Vitals;
        let mut v = Vitals::new();
        v.mana = 1;
        assert!(
            Projectile::cast_spark(
                1,
                0,
                WAND_OF_SPARKING,
                Prefix(0),
                [0.0; 3],
                [1.0, 0.0, 0.0],
                &mut v,
            )
            .is_none()
        );
        assert_eq!(v.mana, 1, "a refused cast spends nothing");
    }

    #[test]
    fn spark_damages_enemy() {
        use crate::item::WAND_OF_SPARKING;
        use crate::player::Vitals;
        let mut v = Vitals::new();
        let p = Projectile::cast_spark(
            1,
            0,
            WAND_OF_SPARKING,
            Prefix(0),
            [0.5, 1.0, 0.5],
            [1.0, 0.0, 0.0],
            &mut v,
        )
        .unwrap();
        let mut e = [Enemy::new(EnemyId(1), EnemyKind::Zombie, [0.5, 1.0, 0.5])];
        let hp = e[0].hp;
        assert_eq!(hit_enemy(&p, &mut e, &mut Rng::new(3)), Some(EnemyId(1)));
        assert!(e[0].hp < hp);
    }

    #[test]
    fn spark_lives_sixty_ticks() {
        use crate::item::WAND_OF_SPARKING;
        use crate::player::Vitals;
        let mut v = Vitals::new();
        let mut p = Projectile::cast_spark(
            1,
            0,
            WAND_OF_SPARKING,
            Prefix(0),
            [0.0; 3],
            [1.0, 0.0, 0.0],
            &mut v,
        )
        .unwrap();
        let q = Wall(None);
        for _ in 0..SPARK_TIME_LEFT - 1 {
            assert_eq!(step(&q, &mut p), Step::Flying);
        }
        assert_eq!(step(&q, &mut p), Step::Expired);
    }

    #[test]
    fn boomerang_returns_to_player() {
        let player_pos = [0.0, 50.0, 0.0];
        let dir = [1.0, 0.0, 0.0];
        let mut p =
            Projectile::throw_boomerang(1, 0, crate::item::ENCHANTED_BOOMERANG, player_pos, dir)
                .expect("boomerang should spawn");
        assert_eq!(p.kind, ProjKind::Boomerang);
        let q = Wall(None);
        let mut max_x = p.pos[0];
        let mut returned = false;
        for _ in 0..200 {
            let st = step_with_target(&q, &mut p, player_pos);
            if p.pos[0] > max_x {
                max_x = p.pos[0];
            }
            if st == Step::Expired {
                returned = true;
                break;
            }
        }
        assert!(
            (max_x - player_pos[0] - 20.0).abs() < 3.0,
            "max_x was {max_x}"
        );
        assert!(returned, "boomerang returned and was caught at player pos");
    }

    #[test]
    fn spear_thrusts_and_pierces() {
        let player_pos = [0.0, 50.0, 0.0];
        let dir = [1.0, 0.0, 0.0];
        let mut p = Projectile::thrust_spear(1, 0, crate::item::SPEAR, player_pos, dir)
            .expect("spear should spawn");
        assert_eq!(p.kind, ProjKind::Spear);
        let q = Wall(None);

        let mut max_x = p.pos[0];
        for _ in 0..16 {
            step_with_target(&q, &mut p, player_pos);
            if p.pos[0] > max_x {
                max_x = p.pos[0];
            }
        }
        assert!(
            (max_x - player_pos[0] - 3.5).abs() < 1.0,
            "spear reach was {max_x}"
        );

        let mut e = [
            Enemy::new(EnemyId(1), EnemyKind::Zombie, [1.5, 50.0, 0.0]),
            Enemy::new(EnemyId(2), EnemyKind::Zombie, [2.5, 50.0, 0.0]),
        ];
        e[0].hp = 200;
        e[1].hp = 200;
        let mut rng = Rng::new(42);
        p.pos = [2.0, 50.0, 0.0];
        let hits = hit_enemies(&mut p, &mut e, &mut rng);
        assert_eq!(hits.len(), 2, "spear pierces and hits both enemies");
    }

    #[test]
    fn ball_o_hurt_flail_extends_and_returns() {
        use crate::item::BALL_O_HURT;
        let mut flail =
            Projectile::throw_flail(1, 10, BALL_O_HURT, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0])
                .expect("flail created");
        assert_eq!(flail.kind, ProjKind::Flail);
        assert_eq!(flail.ai0, 0); // extending
        let q = Wall(None);
        step_with_target(&q, &mut flail, [0.0, 0.0, 0.0]);
        assert!(flail.pos[0] > 0.0);
        for _ in 0..100 {
            step_with_target(&q, &mut flail, [0.0, 0.0, 0.0]);
        }
        assert_eq!(
            flail.ai0, 1,
            "flail should switch to retracting at max distance"
        );
        let mut expired = false;
        for _ in 0..200 {
            if step_with_target(&q, &mut flail, [0.0, 0.0, 0.0]) == Step::Expired {
                expired = true;
                break;
            }
        }
        assert!(expired, "flail should return and expire at player");
    }

    #[test]
    fn vilethorn_pierces_blocks() {
        use crate::item::VILETHORN;
        let mut vitals = crate::player::Vitals::new();
        assert_eq!(vitals.mana, 20);
        let mut proj = Projectile::cast_vilethorn(
            1,
            10,
            VILETHORN,
            Prefix(0),
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            &mut vitals,
        )
        .expect("vilethorn cast");
        assert_eq!(vitals.mana, 10, "vilethorn spends 10 mana");
        assert_eq!(proj.kind, ProjKind::Vilethorn);
        let q = Wall(Some(2));
        for _ in 0..30 {
            let res = step_with_target(&q, &mut proj, [0.0, 0.0, 0.0]);
            assert_ne!(res, Step::HitBlock, "vilethorn must ignore solid blocks");
        }
        assert!(
            proj.pos[0] > 2.0,
            "vilethorn must pierce through solid block at x=2"
        );
    }

    #[test]
    fn molten_fury_turns_wooden_arrows_into_flaming_arrows() {
        use crate::item::MOLTEN_FURY;
        let p = Projectile::fire(
            1,
            1,
            MOLTEN_FURY,
            WOODEN_ARROW,
            Prefix(0),
            [0.0; 3],
            [0.0, 0.0, -1.0],
        )
        .expect("molten fury fires wooden arrows");
        assert_eq!(p.kind, ProjKind::FlamingArrow);
        assert_eq!(p.damage, 31 + item_data(WOODEN_ARROW).damage);
    }

    #[test]
    fn molten_fury_leaves_other_arrows_unchanged() {
        use crate::item::MOLTEN_FURY;
        // Flaming Arrows keep their own ammo kind and damage.
        let f = Projectile::fire(
            1,
            1,
            MOLTEN_FURY,
            FLAMING_ARROW,
            Prefix(0),
            [0.0; 3],
            [0.0, 0.0, -1.0],
        )
        .expect("molten fury fires flaming arrows");
        assert_eq!(f.kind, ProjKind::FlamingArrow);
        assert_eq!(f.damage, 31 + item_data(FLAMING_ARROW).damage);
        // Plain bows still shoot plain Wooden Arrows.
        let w = Projectile::fire_from_bow(2, 1, WOODEN_ARROW, [0.0; 3], [0.0, 0.0, -1.0]).unwrap();
        assert_eq!(w.kind, ProjKind::Arrow);
        assert_eq!(
            w.damage,
            item_data(WOODEN_BOW).damage + item_data(WOODEN_ARROW).damage
        );
    }

    #[test]
    fn flamarang_throw_uses_its_own_kind() {
        use crate::item::{ENCHANTED_BOOMERANG, FLAMARANG};
        let f = Projectile::throw_boomerang(1, 1, FLAMARANG, [0.0, 50.0, 0.0], [1.0, 0.0, 0.0])
            .expect("flamarang should spawn");
        assert_eq!(f.kind, ProjKind::Flamarang);
        assert_eq!(f.damage, item_data(FLAMARANG).damage);
        let e = Projectile::throw_boomerang(
            2,
            1,
            ENCHANTED_BOOMERANG,
            [0.0, 50.0, 0.0],
            [1.0, 0.0, 0.0],
        )
        .expect("enchanted boomerang should spawn");
        assert_eq!(e.kind, ProjKind::Boomerang);
    }

    #[test]
    fn flamarang_does_not_stop_on_first_enemy() {
        use crate::item::FLAMARANG;
        let mut p = Projectile::throw_boomerang(1, 1, FLAMARANG, [0.0, 50.0, 0.0], [1.0, 0.0, 0.0])
            .expect("flamarang should spawn");
        p.pos = [2.0, 50.0, 0.0];
        let v0 = p.vel[0];
        let mut e = [
            Enemy::new(EnemyId(1), EnemyKind::Zombie, [1.5, 50.0, 0.0]),
            Enemy::new(EnemyId(2), EnemyKind::Zombie, [2.5, 50.0, 0.0]),
        ];
        e[0].hp = 200;
        e[1].hp = 200;
        let mut rng = Rng::new(7);
        // Outward hit bounces off the first enemy and reverses velocity.
        let hits = hit_enemies(&mut p, &mut e, &mut rng);
        assert_eq!(hits.len(), 1, "outward hit bounces off first enemy");
        assert_eq!(p.ai0, 1, "flamarang reverses to return trip");
        assert!((p.vel[0] + v0).abs() < 1e-6, "velocity inverted on bounce");

        // On the return trip (ai0 == 1), it pierces multiple enemies (respecting hurt_ticks).
        for en in &mut e {
            en.hurt_ticks = 0;
        }
        let return_hits = hit_enemies(&mut p, &mut e, &mut rng);
        assert_eq!(return_hits.len(), 2, "return trip pierces both enemies");
        let hp = e[0].hp;
        assert_eq!(hit_enemies(&mut p, &mut e, &mut rng).len(), 0);
        assert_eq!(e[0].hp, hp);
    }

    #[test]
    fn magic_missile_spawns_and_flies() {
        use crate::item::MAGIC_MISSILE;
        let mut vitals = crate::player::Vitals::new();
        assert_eq!(vitals.mana, 20);
        let mut proj = Projectile::cast_magic_missile(
            1,
            10,
            MAGIC_MISSILE,
            Prefix(0),
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            &mut vitals,
        )
        .expect("magic missile cast");
        assert_eq!(vitals.mana, 6, "magic missile spends 14 mana");
        assert_eq!(proj.kind, ProjKind::MagicMissile);
        assert_eq!(proj.damage, 35);
        let q = Wall(None);
        let res = step(&q, &mut proj);
        assert_eq!(res, Step::Flying);
        assert!(proj.pos[0] > 0.0);
    }

    #[test]
    fn m11_laser_rifle_casts_straight_no_gravity_laser() {
        use crate::item::LASER_RIFLE;
        let mut vitals = crate::player::Vitals::new();
        let start_mana = vitals.mana;
        let mut proj = Projectile::cast_laser(
            1,
            10,
            LASER_RIFLE,
            Prefix(0),
            [0.0, 5.0, 0.0],
            [1.0, 0.0, 0.0],
            &mut vitals,
        )
        .expect("laser rifle cast");
        assert_eq!(vitals.mana, start_mana - 8);
        assert_eq!(proj.kind, ProjKind::Laser);
        assert_eq!(proj.damage, 29);
        let q = Wall(None);
        for _ in 0..10 {
            assert_eq!(step(&q, &mut proj), Step::Flying);
        }
        assert!(proj.pos[0] > 0.0);
        assert!(
            (proj.pos[1] - 5.0).abs() < 1e-5,
            "laser has no gravity drop"
        );
    }

    #[test]
    fn laser_rifle_pierces_three_enemies() {
        use crate::item::LASER_RIFLE;
        let mut vitals = crate::player::Vitals::new();
        let mut proj = Projectile::cast_laser(
            1,
            10,
            LASER_RIFLE,
            Prefix(0),
            [0.0, 0.5, 0.0],
            [1.0, 0.0, 0.0],
            &mut vitals,
        )
        .unwrap();
        let mut enemies = vec![
            Enemy::new(EnemyId(1), EnemyKind::Zombie, [0.0, 0.0, 0.0]),
            Enemy::new(EnemyId(2), EnemyKind::Zombie, [0.0, 0.0, 0.0]),
            Enemy::new(EnemyId(3), EnemyKind::Zombie, [0.0, 0.0, 0.0]),
        ];
        let mut rng = Rng::new(7);
        let hits = hit_enemies_detailed(&mut proj, &mut enemies, &mut rng);
        assert_eq!(hits.len(), 3, "laser must pierce up to 3 enemies");
        assert_eq!(proj.pierced, 3);
        for e in &enemies {
            assert!(e.hp < e.kind.stats().hp, "every enemy takes a hit");
        }
    }

    #[test]
    fn aqua_scepter_casts_piercing_water_stream_with_gravity_arc() {
        use crate::item::AQUA_SCEPTER;
        let mut vitals = crate::player::Vitals::new();
        let mut proj = Projectile::cast_aqua_scepter(
            1,
            10,
            AQUA_SCEPTER,
            Prefix(0),
            [0.0, 5.0, 0.0],
            [1.0, 0.0, 0.0],
            &mut vitals,
        )
        .expect("aqua scepter cast");
        assert_eq!(vitals.mana, 20 - 7);
        assert_eq!(proj.damage, 27);
        assert_eq!(proj.ai0, -1);

        let mut enemies = vec![
            Enemy::new(EnemyId(1), EnemyKind::Zombie, [0.0, 4.5, 0.0]),
            Enemy::new(EnemyId(2), EnemyKind::Zombie, [0.0, 4.5, 0.0]),
        ];
        let mut rng = Rng::new(3);
        let hits = hit_enemies_detailed(&mut proj, &mut enemies, &mut rng);
        assert_eq!(hits.len(), 2, "aqua scepter water stream pierces enemies");

        let q = Wall(None);
        assert_eq!(step(&q, &mut proj), Step::Flying);
        assert!(proj.pos[1] < 5.0, "water stream arcs downward with gravity");
    }

    #[test]
    fn blue_moon_and_valor_spawn_flail_projectiles() {
        use crate::item::{BLUE_MOON, VALOR};
        let bm = Projectile::throw_flail(1, 0, BLUE_MOON, [0.0, 5.0, 0.0], [1.0, 0.0, 0.0])
            .expect("Blue Moon flail");
        assert_eq!(bm.kind, ProjKind::Flail);
        assert_eq!(bm.damage, 54);

        let val = Projectile::throw_flail(2, 0, VALOR, [0.0, 5.0, 0.0], [1.0, 0.0, 0.0])
            .expect("Valor projectile");
        assert_eq!(val.kind, ProjKind::Yoyo);
        assert_eq!(val.damage, 28);
    }

    #[test]
    fn demon_scythe_accelerates_and_hits_blocks() {
        let mut scythe = Projectile {
            id: 99,
            pos: [0.0, 5.0, 0.0],
            vel: [0.06, 0.0, 0.0],
            kind: ProjKind::DemonScythe,
            owner: u32::MAX,
            ttl: 180,
            damage: 22,
            knock_back: 1.0,
            crit: 0,
            ai0: 0,
            pierced: 0,
            origin: [0.0, 5.0, 0.0],
            armor_penetration: 0,
        };
        let empty = Wall(None);
        assert_eq!(step(&empty, &mut scythe), Step::Flying);
        assert!(scythe.vel[0] > 0.06, "DemonScythe must accelerate");
        let wall = Wall(Some(2));
        scythe.pos = [1.8, 5.0, 0.0];
        scythe.vel = [0.5, 0.0, 0.0];
        assert_eq!(step(&wall, &mut scythe), Step::HitBlock);
    }

    #[test]
    fn single_hit_rapid_bullets_ignore_hurt_ticks() {
        use crate::item::{CLOCKWORK_ASSAULT_RIFLE, MUSKET_BALL};
        let mut e = [Enemy::new(EnemyId(1), EnemyKind::Zombie, [0.0, 0.0, 0.0])];
        e[0].hp = 200;
        let mut rng = Rng::new(11);
        for id in 1..=3u32 {
            let mut b = Projectile::fire(
                id,
                0,
                CLOCKWORK_ASSAULT_RIFLE,
                MUSKET_BALL,
                Prefix(0),
                [0.0, 0.5, 0.0],
                [1.0, 0.0, 0.0],
            )
            .unwrap();
            let hits = hit_enemies_detailed(&mut b, &mut e, &mut rng);
            assert_eq!(hits.len(), 1, "shot {id} must land despite rapid fire");
        }
    }

    #[test]
    fn water_stream_pierces_five_and_arcs() {
        use crate::item::AQUA_SCEPTER;
        let mut v = crate::player::Vitals::new();
        let mut p = Projectile::cast_aqua_scepter(
            1,
            0,
            AQUA_SCEPTER,
            Prefix(0),
            [0.0, 5.0, 0.0],
            [1.0, 0.0, 0.0],
            &mut v,
        )
        .unwrap();
        assert_eq!(p.kind, ProjKind::WaterStream);
        let mut enemies: Vec<_> = (1..=6u32)
            .map(|id| Enemy::new(EnemyId(id), EnemyKind::Zombie, [0.0, 4.5, 0.0]))
            .collect();
        let hits = hit_enemies_detailed(&mut p, &mut enemies, &mut Rng::new(7));
        assert_eq!(hits.len(), 5, "WaterStream pierces up to 5 enemies");
        assert_eq!(step(&Wall(None), &mut p), Step::Flying);
        assert!(p.vel[1] < 0.0, "WaterStream arcs with gravity");
    }

    #[test]
    fn spark_drops_with_gravity_pierces_two_and_ignites() {
        use crate::item::WAND_OF_SPARKING;
        let mut v = crate::player::Vitals::new();
        let mut p = Projectile::cast_spark(
            1,
            0,
            WAND_OF_SPARKING,
            Prefix(0),
            [0.0, 5.0, 0.0],
            [1.0, 0.0, 0.0],
            &mut v,
        )
        .unwrap();
        assert_eq!(step(&Wall(None), &mut p), Step::Flying);
        assert!(p.vel[1] < 0.0, "Spark has immediate gravity");

        let mut enemies: Vec<_> = (1..=10u32)
            .map(|id| {
                Enemy::new(
                    EnemyId(id),
                    EnemyKind::Zombie,
                    [p.pos[0], p.pos[1] - 0.5, 0.0],
                )
            })
            .collect();
        let hits = hit_enemies_detailed(&mut p, &mut enemies, &mut Rng::new(5));
        assert_eq!(hits.len(), 2, "Spark pierces 2 enemies");
        // Verify 50% On Fire! rate across multiple casts.
        let mut ignited = 0;
        let mut rng = Rng::new(42);
        for i in 0..200 {
            let mut sp = p.clone();
            sp.pierced = 0;
            let mut z = [Enemy::new(
                EnemyId(i + 20),
                EnemyKind::Zombie,
                [p.pos[0], p.pos[1] - 0.5, 0.0],
            )];
            hit_enemies_detailed(&mut sp, &mut z, &mut rng);
            if z[0].is_on_fire() {
                ignited += 1;
            }
        }
        assert!(
            (70..=130).contains(&ignited),
            "expected ~100/200 burns, got {ignited}"
        );
    }

    #[test]
    fn magic_missile_steers_toward_aim_point() {
        use crate::item::MAGIC_MISSILE;
        let mut v = crate::player::Vitals::new();
        let mut p = Projectile::cast_magic_missile(
            1,
            0,
            MAGIC_MISSILE,
            Prefix(0),
            [0.0, 5.0, 0.0],
            [1.0, 0.0, 0.0],
            &mut v,
        )
        .unwrap();
        steer_magic_missile(&mut p, [0.0, 15.0, 0.0]);
        assert!(p.vel[1] > 0.0, "missile should steer up toward aim point");
    }

    #[test]
    fn prefixes_scale_ranged_and_magic_projectiles() {
        use crate::item::{MUSKET, MUSKET_BALL, WAND_OF_SPARKING};
        // Unreal (82): 1.15 dmg, 1.15 kb, 1.1 vel, +5 crit
        let p = Projectile::fire(
            1,
            0,
            MUSKET,
            MUSKET_BALL,
            Prefix(82),
            [0.0; 3],
            [1.0, 0.0, 0.0],
        )
        .unwrap();
        let base = Projectile::fire(
            2,
            0,
            MUSKET,
            MUSKET_BALL,
            Prefix(0),
            [0.0; 3],
            [1.0, 0.0, 0.0],
        )
        .unwrap();
        assert!(p.damage > base.damage);
        assert!(p.vel[0] > base.vel[0]);
        assert_eq!(p.crit, base.crit + 5);

        // Mythical (83): 1.15 dmg, 0.9 mana, +5 crit
        let mut v = crate::player::Vitals::new();
        let wand = Projectile::cast_spark(
            3,
            0,
            WAND_OF_SPARKING,
            Prefix(83),
            [0.0; 3],
            [1.0, 0.0, 0.0],
            &mut v,
        )
        .unwrap();
        assert_eq!(wand.damage, (14.0_f32 * 1.15).round_ties_even() as i32);
        assert_eq!(wand.crit, 10 + 5);
    }

    #[test]
    fn projectiles_hit_and_kill_critters() {
        let mut proj = Projectile::fire(
            1,
            0,
            WOODEN_BOW,
            WOODEN_ARROW,
            Prefix(0),
            [0.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
        )
        .unwrap();
        proj.pos = [2.0, 1.2, 0.0];
        let mut enemies = vec![Enemy::new(EnemyId(10), EnemyKind::Bunny, [2.0, 1.0, 0.0])];
        let mut rng = Rng::new(3);
        let hits = hit_enemies_detailed(&mut proj, &mut enemies, &mut rng);
        assert_eq!(hits.len(), 1);
        assert!(hits[0].hit.killed);
        assert!(enemies[0].hp <= 0);
    }

    #[test]
    fn grapple_hook_latches_and_explosives_bounce_and_expire() {
        struct WallAndFloor;
        impl BlockQuery for WallAndFloor {
            fn block(&self, x: i32, y: i32, _z: i32) -> BlockId {
                if y < 0 || x >= 5 { STONE } else { AIR }
            }
        }

        let mut hook = Projectile::fire_grapple(1, 7, [0.5, 2.0, 0.5], [1.0, 0.0, 0.0]);
        assert_eq!(hook.kind, ProjKind::GrappleHook);
        for _ in 0..20 {
            let st = step(&WallAndFloor, &mut hook);
            assert_eq!(st, Step::Flying);
            if hook.ai0 == 1 {
                break;
            }
        }
        assert_eq!(hook.ai0, 1, "grapple hook should latch onto solid wall");
        assert_eq!(hook.vel, [0.0, 0.0, 0.0]);

        for item in [
            crate::item::BOMB,
            crate::item::DYNAMITE,
            crate::item::GRENADE,
        ] {
            let mut exp =
                Projectile::throw_explosive(2, 7, item, [0.5, 2.0, 0.5], [1.0, -0.5, 0.0]);
            let mut saw_bounce = false;
            let mut expired = false;
            for _ in 0..220 {
                let prev_vy = exp.vel[1];
                match step(&WallAndFloor, &mut exp) {
                    Step::Flying => {
                        if prev_vy < 0.0 && exp.vel[1] > 0.0 {
                            saw_bounce = true;
                        }
                    }
                    Step::Expired => {
                        expired = true;
                        break;
                    }
                    Step::HitBlock => panic!("explosives must bounce rather than despawn"),
                }
            }
            assert!(saw_bounce, "{item:?} should bounce off floor");
            assert!(expired, "{item:?} should expire at end of fuse");
        }
    }

    #[test]
    fn water_bolt_bounces_off_walls() {
        let wall = Wall(Some(3));
        let mut vitals = crate::player::Vitals::new();
        vitals.mana = 50;
        let mut wb = Projectile::cast_water_bolt(
            1,
            0,
            crate::item::WATER_BOLT,
            Prefix(0),
            [1.0, 5.0, 0.5],
            [1.0, 0.0, 0.0],
            &mut vitals,
            None,
        )
        .expect("water bolt should cast");
        assert_eq!(wb.kind, ProjKind::WaterBolt);
        for _ in 0..20 {
            let st = step(&wall, &mut wb);
            assert_eq!(st, Step::Flying);
            if wb.ai0 > 0 {
                break;
            }
        }
        assert!(wb.ai0 >= 1, "water bolt should bounce off wall");
        assert!(wb.vel[0] < 0.0, "x velocity should reverse on bounce");
    }
}
