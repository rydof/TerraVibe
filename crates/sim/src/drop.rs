//! World item entities: enemy loot, tile drops and pickup.

use crate::chunk::BlockQuery;
use crate::enemy::EnemyKind;
use crate::inventory::{Inventory, ItemStack};
use crate::item::{
    BONE, COPPER_COIN, DEMONITE_ORE_ITEM, GEL, GOLD_COIN, GOLDEN_KEY, HEART, ItemId, LENS,
    PLATINUM_COIN, Prefix, Rng, SHADOW_SCALE, SILVER_COIN,
};
use crate::player::Vitals;
use crate::player::consts::{MAX_FALL_SPEED, PX_TO_BLOCKS};
use crate::player::vitals::HEART_HEAL;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct DropId(pub u32);

#[derive(Clone, Debug, PartialEq)]
pub struct Drop {
    pub id: DropId,
    pub stack: ItemStack,
    /// Bottom-centre of the item.
    pub pos: [f32; 3],
    pub vel: [f32; 3],
    /// Ticks before the item can be picked up.
    pub pickup_delay: i32,
    pub on_ground: bool,
}

/// Item.gravity-like fall acceleration, per tick.
pub const DROP_GRAVITY: f32 = 0.4 * PX_TO_BLOCKS;
/// Reach (blocks) from the player's centre.
pub const PICKUP_RADIUS: f32 = 1.5;
/// Magnetic pull-in radius (blocks) from the player's centre.
pub const MAGNET_RADIUS: f32 = 3.5;
/// Ticks between spawning and being collectable (also after a failed pickup).
pub const PICKUP_DELAY: i32 = 15;

impl Drop {
    pub fn new(id: DropId, stack: ItemStack, pos: [f32; 3]) -> Drop {
        Drop {
            id,
            stack,
            pos,
            vel: [0.0, 4.0 * PX_TO_BLOCKS, 0.0],
            pickup_delay: PICKUP_DELAY,
            on_ground: false,
        }
    }
}

fn stack(id: ItemId, count: u32) -> ItemStack {
    ItemStack {
        id,
        count,
        prefix: Prefix(0),
    }
}

/// Copper value split greedily into platinum/gold/silver/copper stacks.
pub fn coin_stacks(copper: i32) -> Vec<ItemStack> {
    let mut left = copper.max(0) as u32;
    let mut out = Vec::new();
    for (id, unit) in [
        (PLATINUM_COIN, 1_000_000),
        (GOLD_COIN, 10_000),
        (SILVER_COIN, 100),
        (COPPER_COIN, 1),
    ] {
        if left >= unit {
            out.push(stack(id, left / unit));
            left %= unit;
        }
    }
    out
}

/// NPC.NPCLoot: slimes drop 1-2 gel; every kill drops `value * (80..=175)%`
/// copper (gameplay spec).
pub fn loot_for(kind: EnemyKind, rng: &mut Rng) -> Vec<ItemStack> {
    let mut out = Vec::new();
    if kind == EnemyKind::Slime {
        out.push(stack(GEL, 1 + rng.below(2)));
    }
    if kind == EnemyKind::DemonEye {
        // gameplay spec: Common(236, 100) (Black Lens, not in this game) with
        // OnFailedRoll Common(38, 3): lens on a 1/3 roll when the 1/100 roll fails.
        if rng.below(100) != 0 && rng.below(3) == 0 {
            out.push(stack(LENS, 1));
        }
    }
    if kind == EnemyKind::EaterOfSouls {
        // Eater of Souls drops coins (already rolled from NPC value); Shadow Scales and
        // Demonite Ore come from Eater of Worlds / Eye of Cthulhu / mining instead.
    }
    if matches!(
        kind,
        EnemyKind::EaterOfWorldsHead | EnemyKind::EaterOfWorldsBody | EnemyKind::EaterOfWorldsTail
    ) {
        out.push(stack(DEMONITE_ORE_ITEM, 1 + rng.below(3)));
        out.push(stack(SHADOW_SCALE, 1 + rng.below(2)));
    }
    if kind == EnemyKind::AngryBones {
        out.push(stack(BONE, 1 + rng.below(3)));
        if rng.below(65) == 0 {
            out.push(stack(GOLDEN_KEY, 1));
        }
    }
    if kind == EnemyKind::CursedSkull {
        out.push(stack(BONE, 1 + rng.below(2)));
        if rng.below(65) == 0 {
            out.push(stack(GOLDEN_KEY, 1));
        }
    }
    if kind == EnemyKind::Skeleton || kind == EnemyKind::AngryBones {
        if rng.below(10) == 0 {
            out.push(stack(crate::item::HOOK, 1));
        }
    }
    if kind == EnemyKind::DarkCaster {
        if rng.below(65) == 0 {
            out.push(stack(GOLDEN_KEY, 1));
        }
    }
    if kind == EnemyKind::VoodooDemon {
        out.push(stack(crate::item::GUIDE_VOODOO_DOLL, 1));
    }
    if kind == EnemyKind::Demon || kind == EnemyKind::VoodooDemon {
        // Vanilla gameplay spec ItemDropRule.Common(272, 35) (Demon Scythe)
        // Item 272 is not in this game yet; consume the roll without dropping.
        let _ = rng.below(35);
    }
    if kind == EnemyKind::BoneSerpentBody || kind == EnemyKind::BoneSerpentTail {
        return out;
    }
    let pct = 80 + rng.below(96) as i32;
    out.extend(coin_stacks(kind.stats().value * pct / 100));
    out
}

/// NPC.NPCLoot_DropCommonLifeAndMana (gameplay spec): 1 in 6 kills roll a pickup, and a
/// Heart drops on a further 1 in 2 while the nearest player is hurt (`statLife < statLifeMax2`).
/// Deviation: vanilla tries a Mana Star first; this game has no mana pickup, so that branch is
/// skipped (as it is vanilla when mana is full).
pub fn heart_drop(rng: &mut Rng, player_hurt: bool) -> Option<ItemStack> {
    (rng.below(6) == 0 && rng.below(2) == 0 && player_hurt).then(|| stack(HEART, 1))
}

fn roll_item_prefixes(items: &mut [ItemStack], rng: &mut Rng) {
    for st in items {
        if crate::item::item_data(st.id).max_stack == 1 {
            st.prefix = crate::item::roll_prefix(st.id, &mut || rng.next_u32());
        }
    }
}

/// Shadow orb loot: 1st orb broken guarantees Musket + 100 Musket Balls.
/// 2nd+ orbs roll 1 of 5 choices (gameplay spec).
pub fn roll_shadow_orb_loot(orbs_broken_before: u32, rng: &mut Rng) -> Vec<ItemStack> {
    let mut out = if orbs_broken_before == 0 {
        vec![
            stack(crate::item::MUSKET, 1),
            stack(crate::item::MUSKET_BALL, 100),
        ]
    } else {
        match rng.below(5) {
            0 => vec![
                stack(crate::item::MUSKET, 1),
                stack(crate::item::MUSKET_BALL, 100),
            ],
            1 => vec![stack(crate::item::VILETHORN, 1)],
            2 => vec![stack(crate::item::BALL_O_HURT, 1)],
            3 => vec![stack(crate::item::SHADOW_ORB_ITEM, 1)],
            _ => vec![stack(crate::item::BAND_OF_STARPOWER, 1)],
        }
    };
    roll_item_prefixes(&mut out, rng);
    out
}

/// Wall of Flesh Classic Mode loot: guaranteed Pwnhammer, 1 of 3 Emblems, and
/// 1 of 4 Wall of Flesh weapons.
pub fn roll_wof_loot(rng: &mut Rng) -> Vec<ItemStack> {
    use crate::item::{
        BREAKER_BLADE, CLOCKWORK_ASSAULT_RIFLE, FIRECRACKER, LASER_RIFLE, PWNHAMMER, RANGER_EMBLEM,
        SORCERER_EMBLEM, WARRIOR_EMBLEM,
    };
    let emblem = match rng.below(3) {
        0 => WARRIOR_EMBLEM,
        1 => RANGER_EMBLEM,
        _ => SORCERER_EMBLEM,
    };
    let weapon = match rng.below(4) {
        0 => BREAKER_BLADE,
        1 => CLOCKWORK_ASSAULT_RIFLE,
        2 => LASER_RIFLE,
        _ => FIRECRACKER,
    };
    let mut out = vec![stack(PWNHAMMER, 1), stack(emblem, 1), stack(weapon, 1)];
    roll_item_prefixes(&mut out, rng);
    out
}

/// King Slime guaranteed drops: Slime Hook or Slimy Saddle, 2 Ninja pieces, coins, potions.
pub fn roll_king_slime_loot(rng: &mut Rng) -> Vec<ItemStack> {
    use crate::item::{
        GOLD_COIN, LESSER_HEALING_POTION, NINJA_HOOD, NINJA_PANTS, NINJA_SHIRT, SLIME_HOOK,
        SLIMY_SADDLE,
    };
    let main = if rng.below(2) == 0 {
        SLIME_HOOK
    } else {
        SLIMY_SADDLE
    };
    let ninja = match rng.below(3) {
        0 => (NINJA_HOOD, NINJA_SHIRT),
        1 => (NINJA_SHIRT, NINJA_PANTS),
        _ => (NINJA_HOOD, NINJA_PANTS),
    };
    let mut out = vec![
        stack(main, 1),
        stack(ninja.0, 1),
        stack(ninja.1, 1),
        stack(GOLD_COIN, 1 + rng.below(2)),
        stack(LESSER_HEALING_POTION, 5 + rng.below(6)),
    ];
    roll_item_prefixes(&mut out, rng);
    out
}

/// Queen Bee guaranteed drops: Bee Wax, Beenades, Bee Gun/Bee Keeper, Honey Comb, coins, potions.
pub fn roll_queen_bee_loot(rng: &mut Rng) -> Vec<ItemStack> {
    use crate::item::{
        BEE_GUN, BEE_KEEPER, BEE_WAX, BEENADE, GOLD_COIN, HONEY_COMB, LESSER_HEALING_POTION,
    };
    let weapon = if rng.below(2) == 0 {
        BEE_GUN
    } else {
        BEE_KEEPER
    };
    let mut out = vec![
        stack(weapon, 1),
        stack(HONEY_COMB, 1),
        stack(BEE_WAX, 16 + rng.below(11)),
        stack(BEENADE, 15 + rng.below(16)),
        stack(GOLD_COIN, 5),
        stack(LESSER_HEALING_POTION, 5 + rng.below(6)),
    ];
    roll_item_prefixes(&mut out, rng);
    out
}

/// Breakable pot loot: always drops coins, with a chance for Lesser Healing
/// Potions and Torches.
pub fn roll_pot_loot(rng: &mut Rng) -> Vec<ItemStack> {
    let mut out = coin_stacks(25 + rng.below(150) as i32);
    if rng.below(3) == 0 {
        out.push(stack(crate::item::LESSER_HEALING_POTION, 1 + rng.below(2)));
    }
    if rng.below(2) == 0 {
        out.push(stack(crate::item::TORCH, 2 + rng.below(4)));
    }
    out
}

/// A Heart is never stored: within reach it heals 20 and vanishes, even at full life
/// (Player.GetItem, gameplay spec: `Heal(20); TurnToAir()`).
pub fn try_pickup_heart(d: &mut Drop, v: &mut Vitals, player_pos: [f32; 3]) -> bool {
    if d.stack.id != HEART || d.pickup_delay > 0 {
        return false;
    }
    let dist2: f32 = (0..3).map(|i| (d.pos[i] - player_pos[i]).powi(2)).sum();
    if dist2 > PICKUP_RADIUS * PICKUP_RADIUS {
        return false;
    }
    for _ in 0..d.stack.count {
        v.heal(HEART_HEAL);
    }
    d.stack.count = 0;
    true
}

/// Pulls a drop smoothly toward `player_pos` when within [`MAGNET_RADIUS`].
/// Returns `true` if the drop was magnetized.
pub fn magnetize_drop(d: &mut Drop, player_pos: [f32; 3]) -> bool {
    if d.pickup_delay > 0 {
        return false;
    }
    let dx = player_pos[0] - d.pos[0];
    let dy = player_pos[1] - d.pos[1];
    let dz = player_pos[2] - d.pos[2];
    let dist2 = dx * dx + dy * dy + dz * dz;
    if dist2 > MAGNET_RADIUS * MAGNET_RADIUS || dist2 < 1e-4 {
        return false;
    }
    let dist = dist2.sqrt();
    let speed = 0.22;
    d.vel[0] = (dx / dist) * speed;
    d.vel[1] = (dy / dist) * speed;
    d.vel[2] = (dz / dist) * speed;
    d.on_ground = false;
    true
}

/// Gravity, then rest on the first solid block below.
pub fn tick_drop(q: &impl BlockQuery, d: &mut Drop) {
    d.pickup_delay = (d.pickup_delay - 1).max(0);
    d.vel[1] = (d.vel[1] - DROP_GRAVITY).max(-MAX_FALL_SPEED);
    d.vel[0] *= 0.95;
    d.vel[2] *= 0.95;
    d.pos[0] += d.vel[0];
    d.pos[2] += d.vel[2];
    let ny = d.pos[1] + d.vel[1];
    let (bx, bz) = (d.pos[0].floor() as i32, d.pos[2].floor() as i32);
    if d.vel[1] <= 0.0 && q.block(bx, ny.floor() as i32, bz).is_solid() {
        d.pos[1] = ny.floor() + 1.0;
        d.vel = [0.0; 3];
        d.on_ground = true;
    } else {
        d.pos[1] = ny;
        d.on_ground = false;
    }
}

/// True when anything moved into `inv`. A full inventory leaves the drop and
/// restarts its delay so it is not retried every tick.
pub fn try_pickup(d: &mut Drop, inv: &mut Inventory, player_pos: [f32; 3]) -> bool {
    if d.pickup_delay > 0 {
        return false;
    }
    let dist2: f32 = (0..3).map(|i| (d.pos[i] - player_pos[i]).powi(2)).sum();
    if dist2 > PICKUP_RADIUS * PICKUP_RADIUS {
        return false;
    }
    let before = d.stack.count;
    match inv.add(d.stack) {
        None => d.stack.count = 0,
        Some(rest) => d.stack.count = rest.count,
    }
    if d.stack.count == before {
        d.pickup_delay = PICKUP_DELAY;
    }
    d.stack.count < before
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{AIR, STONE};

    struct Floor;
    impl BlockQuery for Floor {
        fn block(&self, _x: i32, y: i32, _z: i32) -> crate::BlockId {
            if y <= 0 { STONE } else { AIR }
        }
    }

    fn copper_total(v: &[ItemStack]) -> u32 {
        v.iter()
            .filter_map(|s| {
                let u = match s.id {
                    COPPER_COIN => 1,
                    SILVER_COIN => 100,
                    GOLD_COIN => 10_000,
                    PLATINUM_COIN => 1_000_000,
                    _ => return None,
                };
                Some(s.count * u)
            })
            .sum()
    }

    #[test]
    fn lens_and_eye_ids_match_terraria() {
        assert_eq!((LENS.0, crate::item::SUSPICIOUS_LOOKING_EYE.0), (38, 43)); // gameplay spec,:1632
    }

    #[test]
    fn eater_drops_shadow_scale_only_per_table() {
        let mut rng = Rng::new(86);
        for _ in 0..2000 {
            let loot = loot_for(EnemyKind::EaterOfSouls, &mut rng);
            assert!((72..=157).contains(&copper_total(&loot)));
            assert!(
                loot.iter()
                    .all(|s| [COPPER_COIN, SILVER_COIN].contains(&s.id))
            );
            assert!(
                loot.iter()
                    .all(|s| s.id != ItemId(86) && s.id != ItemId(56))
            );
        }
        for kind in [
            EnemyKind::Slime,
            EnemyKind::DemonEye,
            EnemyKind::Zombie,
            EnemyKind::EyeOfCthulhu,
            EnemyKind::ServantOfCthulhu,
            EnemyKind::Guide,
            EnemyKind::Merchant,
        ] {
            for _ in 0..1000 {
                assert!(loot_for(kind, &mut rng).iter().all(|s| s.id != ItemId(86)));
            }
        }
        assert!(
            crate::boss::boss_extra_loot(&mut rng)
                .iter()
                .all(|s| s.id != ItemId(86))
        );
    }

    #[test]
    fn demon_eye_drops_lens_at_one_in_three() {
        let mut rng = Rng::new(21);
        let n = (0..6000)
            .filter(|_| {
                loot_for(EnemyKind::DemonEye, &mut rng)
                    .iter()
                    .any(|s| s.id == LENS)
            })
            .count();
        // 99/100 * 1/3 = 33% -> ~1980 of 6000.
        assert!((1800..=2160).contains(&n), "{n}");
        assert!((0..500).all(|_| {
            !loot_for(EnemyKind::Zombie, &mut rng)
                .iter()
                .any(|s| s.id == LENS)
        }));
    }

    #[test]
    fn slime_drops_one_or_two_gel() {
        let mut rng = Rng::new(3);
        let mut seen = [false; 3];
        for _ in 0..100 {
            let l = loot_for(EnemyKind::Slime, &mut rng);
            let g: u32 = l.iter().filter(|s| s.id == GEL).map(|s| s.count).sum();
            assert!((1..=2).contains(&g));
            seen[g as usize] = true;
        }
        assert!(seen[1] && seen[2]);
        let z = loot_for(EnemyKind::Zombie, &mut rng);
        assert!(z.iter().all(|s| s.id != GEL));
    }

    #[test]
    fn dungeon_enemies_drop_bones_and_golden_keys() {
        use crate::item::{BONE, GOLDEN_KEY};
        let mut found_bones_angry = false;
        let mut found_key_angry = false;
        let mut found_bones_skull = false;
        let mut found_key_skull = false;
        let mut found_key_caster = false;

        for s in 0..1000 {
            let mut rng = Rng::new(s);
            let loot_angry = loot_for(EnemyKind::AngryBones, &mut rng);
            for stack in &loot_angry {
                if stack.id == BONE {
                    assert!((1..=3).contains(&stack.count));
                    found_bones_angry = true;
                }
                if stack.id == GOLDEN_KEY {
                    assert_eq!(stack.count, 1);
                    found_key_angry = true;
                }
            }

            let mut rng2 = Rng::new(s + 5000);
            let loot_skull = loot_for(EnemyKind::CursedSkull, &mut rng2);
            for stack in &loot_skull {
                if stack.id == BONE {
                    assert!((1..=2).contains(&stack.count));
                    found_bones_skull = true;
                }
                if stack.id == GOLDEN_KEY {
                    assert_eq!(stack.count, 1);
                    found_key_skull = true;
                }
            }

            let mut rng3 = Rng::new(s + 10000);
            let loot_caster = loot_for(EnemyKind::DarkCaster, &mut rng3);
            for stack in &loot_caster {
                if stack.id == GOLDEN_KEY {
                    assert_eq!(stack.count, 1);
                    found_key_caster = true;
                }
            }
        }

        assert!(found_bones_angry, "AngryBones must drop bones");
        assert!(found_key_angry, "AngryBones must drop golden keys");
        assert!(found_bones_skull, "CursedSkull must drop bones");
        assert!(found_key_skull, "CursedSkull must drop golden keys");
        assert!(found_key_caster, "DarkCaster must drop golden keys");
    }

    #[test]
    fn coin_drop_is_80_to_175_percent() {
        let mut rng = Rng::new(9);
        for _ in 0..200 {
            let v = copper_total(&loot_for(EnemyKind::Zombie, &mut rng));
            assert!((60 * 80 / 100..=60 * 175 / 100).contains(&v), "{v}");
        }
    }

    #[test]
    fn coins_split_into_tiers() {
        let c = coin_stacks(137);
        assert_eq!(
            c.iter().map(|s| (s.id, s.count)).collect::<Vec<_>>(),
            vec![(SILVER_COIN, 1), (COPPER_COIN, 37)]
        );
        let p = coin_stacks(1_000_000);
        assert_eq!(p.len(), 1);
        assert_eq!((p[0].id, p[0].count), (PLATINUM_COIN, 1));
    }

    #[test]
    fn drop_falls_and_rests_on_ground() {
        let mut d = Drop::new(DropId(1), stack(GEL, 1), [0.5, 5.5, 0.5]);
        for _ in 0..300 {
            tick_drop(&Floor, &mut d);
        }
        assert!(d.on_ground);
        assert_eq!(d.pos[1], 1.0);
        assert_eq!(d.pickup_delay, 0);
    }

    #[test]
    fn pickup_merges_and_full_inventory_keeps_drop() {
        let mut inv = Inventory::empty();
        let mut d = Drop::new(DropId(1), stack(GEL, 3), [0.5, 1.0, 0.5]);
        assert!(!try_pickup(&mut d, &mut inv, [0.5, 1.0, 0.5]), "delay");
        d.pickup_delay = 0;
        assert!(!try_pickup(&mut d, &mut inv, [9.0, 1.0, 0.5]), "reach");
        assert!(try_pickup(&mut d, &mut inv, [0.5, 1.0, 0.5]));
        assert_eq!(d.stack.count, 0);
        assert_eq!(inv.slots.iter().flatten().next().unwrap().count, 3);

        // Partial merge: only one more fits.
        let mut inv = Inventory::empty();
        inv.slots[0] = Some(stack(GEL, 9998));
        for s in inv.slots.iter_mut().skip(1) {
            *s = Some(stack(COPPER_COIN, 1));
        }
        let mut d = Drop::new(DropId(2), stack(GEL, 3), [0.5, 1.0, 0.5]);
        d.pickup_delay = 0;
        assert!(try_pickup(&mut d, &mut inv, [0.5, 1.0, 0.5]));
        assert_eq!(d.stack.count, 2);

        // Full: nothing moves, delay restarts.
        assert!(!try_pickup(&mut d, &mut inv, [0.5, 1.0, 0.5]));
        assert_eq!(d.stack.count, 2);
        assert_eq!(d.pickup_delay, PICKUP_DELAY);
    }

    #[test]
    fn heart_pickup_heals_20_and_is_not_stored() {
        let mut v = Vitals::new();
        v.life = 50;
        let mut d = Drop::new(DropId(1), stack(HEART, 1), [0.5, 1.0, 0.5]);
        d.pickup_delay = 0;
        assert!(try_pickup_heart(&mut d, &mut v, [0.5, 1.0, 0.5]));
        assert_eq!((v.life, d.stack.count), (70, 0));
    }

    #[test]
    fn heart_pickup_at_full_life_still_consumed_per_vanilla() {
        // Player.GetItem always runs Heal(20) then TurnToAir, hurt or not.
        let mut v = Vitals::new();
        let mut d = Drop::new(DropId(1), stack(HEART, 1), [0.5, 1.0, 0.5]);
        d.pickup_delay = 0;
        assert!(try_pickup_heart(&mut d, &mut v, [0.5, 1.0, 0.5]));
        assert_eq!((v.life, d.stack.count), (100, 0));
        // Too far or still in delay: untouched.
        let mut d = Drop::new(DropId(2), stack(HEART, 1), [9.0, 1.0, 0.5]);
        d.pickup_delay = 0;
        assert!(!try_pickup_heart(&mut d, &mut v, [0.5, 1.0, 0.5]));
    }

    #[test]
    fn enemy_loot_can_drop_heart() {
        let mut rng = Rng::new(11);
        let n = (0..2400)
            .filter(|_| heart_drop(&mut rng, true).is_some())
            .count();
        // 1/6 * 1/2 = 1/12 -> ~200 of 2400.
        assert!((140..=260).contains(&n), "{n}");
        assert!((0..500).all(|_| heart_drop(&mut rng, false).is_none()));
    }

    #[test]
    fn shadow_orb_loot_rolls_correctly() {
        use crate::item::{
            BALL_O_HURT, BAND_OF_STARPOWER, MUSKET, MUSKET_BALL, SHADOW_ORB_ITEM, VILETHORN,
        };
        let mut rng = Rng::new(12345);
        let first = roll_shadow_orb_loot(0, &mut rng);
        assert_eq!(first.len(), 2);
        assert_eq!(first[0].id, MUSKET);
        assert_eq!(first[0].count, 1);
        assert_eq!(first[1].id, MUSKET_BALL);
        assert_eq!(first[1].count, 100);

        let mut seen = std::collections::HashSet::new();
        for _ in 0..100 {
            let loot = roll_shadow_orb_loot(1, &mut rng);
            for item in loot {
                seen.insert(item.id);
            }
        }
        assert!(seen.contains(&MUSKET));
        assert!(seen.contains(&VILETHORN));
        assert!(seen.contains(&BALL_O_HURT));
        assert!(seen.contains(&SHADOW_ORB_ITEM));
        assert!(seen.contains(&BAND_OF_STARPOWER));
    }

    #[test]
    fn eater_of_worlds_segments_drop_demonite_and_scales() {
        let mut rng = Rng::new(42);
        for kind in [
            EnemyKind::EaterOfWorldsHead,
            EnemyKind::EaterOfWorldsBody,
            EnemyKind::EaterOfWorldsTail,
        ] {
            let mut demonite_counts = std::collections::HashSet::new();
            let mut scale_counts = std::collections::HashSet::new();
            for _ in 0..100 {
                let loot = loot_for(kind, &mut rng);
                let dem = loot.iter().find(|s| s.id == DEMONITE_ORE_ITEM).unwrap();
                let sc = loot.iter().find(|s| s.id == SHADOW_SCALE).unwrap();
                assert!((1..=3).contains(&dem.count));
                assert!((1..=2).contains(&sc.count));
                demonite_counts.insert(dem.count);
                scale_counts.insert(sc.count);
            }
            assert!(demonite_counts.len() >= 2);
            assert_eq!(scale_counts.len(), 2);
        }
    }

    #[test]
    fn voodoo_demon_always_drops_guide_doll() {
        let mut rng = Rng::new(99);
        for _ in 0..100 {
            let loot = loot_for(EnemyKind::VoodooDemon, &mut rng);
            let doll = loot.iter().find(|s| s.id == crate::item::GUIDE_VOODOO_DOLL);
            assert!(
                doll.is_some(),
                "Voodoo Demon must always drop Guide Voodoo Doll"
            );
            assert_eq!(doll.unwrap().count, 1);
        }
    }

    #[test]
    fn other_underworld_enemies_never_drop_guide_doll() {
        let mut rng = Rng::new(1234);
        for kind in [
            EnemyKind::FireImp,
            EnemyKind::Demon,
            EnemyKind::LavaSlime,
            EnemyKind::Hellbat,
            EnemyKind::BoneSerpentHead,
            EnemyKind::BoneSerpentBody,
            EnemyKind::BoneSerpentTail,
        ] {
            for _ in 0..100 {
                let loot = loot_for(kind, &mut rng);
                assert!(
                    !loot.iter().any(|s| s.id == crate::item::GUIDE_VOODOO_DOLL),
                    "{:?} must never drop Guide Voodoo Doll",
                    kind
                );
            }
        }
    }

    #[test]
    fn m11_wof_loot_always_drops_pwnhammer_one_emblem_and_one_weapon() {
        use crate::item::{
            BREAKER_BLADE, CLOCKWORK_ASSAULT_RIFLE, FIRECRACKER, LASER_RIFLE, PWNHAMMER,
            RANGER_EMBLEM, SORCERER_EMBLEM, WARRIOR_EMBLEM,
        };
        let mut rng = Rng::new(42);
        let emblems = [WARRIOR_EMBLEM, RANGER_EMBLEM, SORCERER_EMBLEM];
        let weapons = [
            BREAKER_BLADE,
            CLOCKWORK_ASSAULT_RIFLE,
            LASER_RIFLE,
            FIRECRACKER,
        ];
        let mut seen_emblems = std::collections::HashSet::new();
        let mut seen_weapons = std::collections::HashSet::new();
        for _ in 0..80 {
            let loot = roll_wof_loot(&mut rng);
            assert!(
                loot.iter().any(|s| s.id == PWNHAMMER && s.count == 1),
                "WoF must always drop Pwnhammer"
            );
            let emb_count = loot.iter().filter(|s| emblems.contains(&s.id)).count();
            let wep_count = loot.iter().filter(|s| weapons.contains(&s.id)).count();
            assert_eq!(emb_count, 1, "WoF must drop exactly 1 emblem");
            assert_eq!(wep_count, 1, "WoF must drop exactly 1 weapon");
            for s in &loot {
                if emblems.contains(&s.id) {
                    seen_emblems.insert(s.id);
                }
                if weapons.contains(&s.id) {
                    seen_weapons.insert(s.id);
                }
            }
        }
        assert_eq!(seen_emblems.len(), 3, "all 3 emblems reachable");
        assert_eq!(seen_weapons.len(), 4, "all 4 weapons reachable");
    }
}
