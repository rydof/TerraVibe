//! Town NPCs (Guide, Merchant): kinds, houses and homes, spawn rules and the
//! wander state machine. Pure; the server owns the entities and runs the timers.

use crate::chunk::BlockQuery;
use crate::enemy::{Enemy, EnemyKind, JUMP_SPEED, tick_enemy};
use crate::item::Rng;
use crate::player::consts::PX_TO_BLOCKS as PX;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum TownKind {
    Guide,
    Merchant,
    ArmsDealer,
    Nurse,
    Demolitionist,
    Dryad,
    GoblinTinkerer,
}

impl TownKind {
    pub const ALL: [TownKind; 7] = [
        TownKind::Guide,
        TownKind::Merchant,
        TownKind::ArmsDealer,
        TownKind::Nurse,
        TownKind::Demolitionist,
        TownKind::Dryad,
        TownKind::GoblinTinkerer,
    ];

    /// Terraria NPC type id (gameplay spec Guide, `:11105` Merchant, `:11109` Arms Dealer).
    pub fn npc_id(self) -> i32 {
        match self {
            TownKind::Guide => 22,
            TownKind::Merchant => 17,
            TownKind::ArmsDealer => 19,
            TownKind::Nurse => 18,
            TownKind::Demolitionist => 38,
            TownKind::Dryad => 20,
            TownKind::GoblinTinkerer => 107,
        }
    }
    pub fn enemy_kind(self) -> EnemyKind {
        match self {
            TownKind::Guide => EnemyKind::Guide,
            TownKind::Merchant => EnemyKind::Merchant,
            TownKind::ArmsDealer => EnemyKind::ArmsDealer,
            TownKind::Nurse => EnemyKind::Nurse,
            TownKind::Demolitionist => EnemyKind::Demolitionist,
            TownKind::Dryad => EnemyKind::Dryad,
            TownKind::GoblinTinkerer => EnemyKind::GoblinTinkerer,
        }
    }
    pub fn from_enemy(k: EnemyKind) -> Option<TownKind> {
        match k {
            EnemyKind::Guide => Some(TownKind::Guide),
            EnemyKind::Merchant => Some(TownKind::Merchant),
            EnemyKind::ArmsDealer => Some(TownKind::ArmsDealer),
            EnemyKind::Nurse => Some(TownKind::Nurse),
            EnemyKind::Demolitionist => Some(TownKind::Demolitionist),
            EnemyKind::Dryad => Some(TownKind::Dryad),
            EnemyKind::GoblinTinkerer => Some(TownKind::GoblinTinkerer),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            TownKind::Guide => "Guide",
            TownKind::Merchant => "Merchant",
            TownKind::ArmsDealer => "Arms Dealer",
            TownKind::Nurse => "Nurse",
            TownKind::Demolitionist => "Demolitionist",
            TownKind::Dryad => "Dryad",
            TownKind::GoblinTinkerer => "Goblin Tinkerer",
        }
    }
    pub fn to_u8(self) -> u8 {
        self as u8
    }
    pub fn from_u8(v: u8) -> Option<TownKind> {
        match v {
            0 => Some(TownKind::Guide),
            1 => Some(TownKind::Merchant),
            2 => Some(TownKind::ArmsDealer),
            3 => Some(TownKind::Nurse),
            4 => Some(TownKind::Demolitionist),
            5 => Some(TownKind::Dryad),
            6 => Some(TownKind::GoblinTinkerer),
            _ => None,
        }
    }
}

/// A valid room the server found. `origin` is the room id (`RoomOk::lo`), `home`
/// the chair cell the resident stands at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct House {
    pub origin: [i32; 3],
    pub home: [i32; 3],
    pub npc: Option<TownKind>,
}

/// Move-in scan: each homeless NPC `(kind, pos)` takes the nearest vacant house.
/// Returns the `(kind, house index)` pairs that moved in. A kind that already
/// lives somewhere is never listed as homeless, so homes stay unique.
pub fn assign_homes(
    houses: &mut [House],
    homeless: &[(TownKind, [f32; 3])],
) -> Vec<(TownKind, usize)> {
    let mut moved = Vec::new();
    for &(kind, pos) in homeless {
        if houses.iter().any(|h| h.npc == Some(kind)) {
            continue;
        }
        let d2 = |h: &House| {
            let (dx, dz) = (
                h.home[0] as f32 + 0.5 - pos[0],
                h.home[2] as f32 + 0.5 - pos[2],
            );
            dx * dx + dz * dz
        };
        let best = (0..houses.len())
            .filter(|&i| houses[i].npc.is_none())
            .min_by(|&a, &b| d2(&houses[a]).total_cmp(&d2(&houses[b])));
        if let Some(i) = best {
            houses[i].npc = Some(kind);
            moved.push((kind, i));
        }
    }
    moved
}

/// Copper needed in some player's inventory before the Merchant arrives
/// (`NPC.merchant spawn check`, gameplay spec: 5000 copper).
pub const MERCHANT_COPPER: u32 = 5000;
/// Ticks between arrival checks (`Main.town NPC spawn check`, gameplay spec).
pub const SPAWN_CHECK_TICKS: u32 = 7200;

/// Merchant arrival (gameplay spec: `merchant spawn check` and none alive;
/// vanilla also needs a free house, here a valid vacant one; daytime and no boss
/// are Stage 6 additions).
pub fn merchant_may_spawn(
    copper: u32,
    vacant_house: bool,
    alive: bool,
    day: bool,
    boss: bool,
) -> bool {
    copper >= MERCHANT_COPPER && vacant_house && !alive && day && !boss
}

/// Arms Dealer arrival (gameplay spec): any player carries a gun or bullets,
/// a vacant valid house exists, and none is currently alive.
pub fn arms_dealer_may_spawn(has_gun_or_bullet: bool, alive: bool, free_house: bool) -> bool {
    has_gun_or_bullet && !alive && free_house
}

/// Nurse arrival: any player has > 100 max life, Merchant is alive, free house exists.
pub fn nurse_may_spawn(
    any_life_gt_100: bool,
    merchant_alive: bool,
    alive: bool,
    free_house: bool,
) -> bool {
    any_life_gt_100 && merchant_alive && !alive && free_house
}

/// Demolitionist arrival: any player carries an explosive, Merchant is alive, free house exists.
pub fn demolitionist_may_spawn(
    has_explosive: bool,
    merchant_alive: bool,
    alive: bool,
    free_house: bool,
) -> bool {
    has_explosive && merchant_alive && !alive && free_house
}

/// Dryad arrival: Eye of Cthulhu, Eater of Worlds, or Skeletron defeated, and free house exists.
pub fn dryad_may_spawn(any_boss_downed: bool, alive: bool, free_house: bool) -> bool {
    any_boss_downed && !alive && free_house
}

/// Goblin Tinkerer arrival: Shadow Orb broken or boss defeated, and free house exists.
pub fn goblin_tinkerer_may_spawn(unlocked: bool, alive: bool, free_house: bool) -> bool {
    unlocked && !alive && free_house
}

// ---- wander AI ---------------------------------------------------------------

/// Wander radius around the anchor (home, or the spawn point when homeless).
pub const WANDER_RADIUS: f32 = 12.0;
/// Walk speed, px/tick (`num21 = 1f`, gameplay spec).
pub const WALK_SPEED: f32 = 1.0 * PX;
/// Within this of the home, a night NPC sleeps.
const AT_HOME: f32 = 0.75;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Idle,
    Walk,
    GoHome,
    Sleep,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wander {
    pub mode: Mode,
    /// Seconds left in Idle/Walk.
    pub timer: f32,
    pub dir: [f32; 2],
}

impl Default for Wander {
    fn default() -> Self {
        Wander {
            mode: Mode::Idle,
            timer: 1.0,
            dir: [0.0, 0.0],
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Intent {
    /// Unit direction in x/z (zero = stand).
    pub dir: [f32; 2],
    pub jump: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Ctx {
    pub pos: [f32; 3],
    pub anchor: [f32; 3],
    pub has_home: bool,
    pub is_day: bool,
    /// Probes along the current heading (see `probe`).
    pub ledge_ahead: bool,
    pub wall_ahead: bool,
    pub step_ahead: bool,
}

fn toward(from: [f32; 3], to: [f32; 3]) -> [f32; 2] {
    let (dx, dz) = (to[0] - from[0], to[2] - from[2]);
    let l = dx.hypot(dz);
    if l < 1e-4 {
        [0.0, 0.0]
    } else {
        [dx / l, dz / l]
    }
}

fn dist_xz(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).hypot(a[2] - b[2])
}

/// One step of the town NPC schedule state machine. `dt` in seconds.
/// Enforces deterministic day wander radius, night return-home, and wall/cliff safety.
pub fn step_town_npc_schedule(st: &mut Wander, rng: &mut Rng, dt: f32, ctx: &Ctx) -> Intent {
    let d = dist_xz(ctx.pos, ctx.anchor);
    if ctx.has_home && !ctx.is_day {
        // Night: walk home, then stand still (sleep).
        if d > AT_HOME {
            st.mode = Mode::GoHome;
            st.dir = toward(ctx.pos, ctx.anchor);
        } else {
            st.mode = Mode::Sleep;
            return Intent::default();
        }
    } else if st.mode == Mode::Sleep {
        st.mode = Mode::Idle;
        st.timer = 1.0;
    } else if d > WANDER_RADIUS || (st.mode == Mode::GoHome && d > WANDER_RADIUS * 0.5) {
        st.mode = Mode::GoHome;
        st.dir = toward(ctx.pos, ctx.anchor);
    } else if st.mode == Mode::GoHome {
        st.mode = Mode::Idle;
        st.timer = 1.0;
    }
    match st.mode {
        Mode::Idle => {
            st.timer -= dt;
            if st.timer <= 0.0 {
                let ang = rng.below(3600) as f32 / 3600.0 * std::f32::consts::TAU;
                let mut dir = [ang.cos(), ang.sin()];
                let secs = 1.0 + rng.below(2001) as f32 / 1000.0; // 1-3 s
                let reach = WALK_SPEED * 60.0 * secs;
                let end = [
                    ctx.pos[0] + dir[0] * reach,
                    ctx.pos[1],
                    ctx.pos[2] + dir[1] * reach,
                ];
                if dist_xz(end, ctx.anchor) > WANDER_RADIUS {
                    dir = toward(ctx.pos, ctx.anchor);
                }
                *st = Wander {
                    mode: Mode::Walk,
                    timer: secs,
                    dir,
                };
            }
            Intent::default()
        }
        Mode::Walk => {
            st.timer -= dt;
            if st.timer <= 0.0 || ctx.ledge_ahead || (ctx.wall_ahead && !ctx.step_ahead) {
                *st = Wander {
                    mode: Mode::Idle,
                    timer: 1.0 + rng.below(3001) as f32 / 1000.0, // 1-4 s
                    dir: [0.0, 0.0],
                };
                return Intent::default();
            }
            Intent {
                dir: st.dir,
                jump: ctx.step_ahead,
            }
        }
        Mode::GoHome => {
            if ctx.ledge_ahead || (ctx.wall_ahead && !ctx.step_ahead) {
                return Intent::default();
            }
            Intent {
                dir: st.dir,
                jump: ctx.step_ahead,
            }
        }
        Mode::Sleep => Intent::default(),
    }
}

/// Alias for [`step_town_npc_schedule`].
pub fn step(st: &mut Wander, rng: &mut Rng, dt: f32, ctx: &Ctx) -> Intent {
    step_town_npc_schedule(st, rng, dt, ctx)
}

/// Looks one body-width ahead along `dir`: `(ledge, wall, step)`. A step is a
/// one-block rise with room above; a ledge is a drop of two blocks or more.
pub fn probe(q: &impl BlockQuery, pos: [f32; 3], dir: [f32; 2], half_w: f32) -> (bool, bool, bool) {
    if dir == [0.0, 0.0] {
        return (false, false, false);
    }
    let ax = pos[0] + dir[0] * (half_w + 0.35);
    let az = pos[2] + dir[1] * (half_w + 0.35);
    let (x, z) = (ax.floor() as i32, az.floor() as i32);
    let y = (pos[1] + 0.05).floor() as i32;
    let solid = |dy: i32| q.block(x, y + dy, z).is_solid();
    let wall = solid(0) || solid(1);
    let step = solid(0) && !solid(1) && !solid(2);
    let ledge = !solid(-1) && !solid(-2) && !solid(0);
    (ledge, wall, step)
}

/// Steers a town NPC for one 60 Hz tick, then runs the shared enemy physics.
pub fn tick_town(
    q: &impl BlockQuery,
    e: &mut Enemy,
    st: &mut Wander,
    rng: &mut Rng,
    anchor: [f32; 3],
    has_home: bool,
    is_day: bool,
) {
    let half = e.kind.stats().half[0];
    let (ledge, wall, stp) = probe(q, e.pos, st.dir, half);
    let ctx = Ctx {
        pos: e.pos,
        anchor,
        has_home,
        is_day,
        ledge_ahead: ledge && e.on_ground,
        wall_ahead: wall,
        step_ahead: stp,
    };
    let it = step(st, rng, 1.0 / 60.0, &ctx);
    if e.hurt_ticks == 0 {
        e.vel[0] = it.dir[0] * WALK_SPEED;
        e.vel[2] = it.dir[1] * WALK_SPEED;
        if it.jump && e.on_ground {
            e.vel[1] = JUMP_SPEED;
        }
    }
    let p = e.pos;
    tick_enemy(q, e, p);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{AIR, BlockId, DIRT};

    fn h(o: i32, npc: Option<TownKind>) -> House {
        House {
            origin: [o, 0, 0],
            home: [o, 0, 0],
            npc,
        }
    }

    #[test]
    fn town_ids_match_terraria() {
        assert_eq!(TownKind::Guide.npc_id(), 22);
        assert_eq!(TownKind::Merchant.npc_id(), 17);
        assert_eq!(TownKind::ArmsDealer.npc_id(), 19);
        assert_eq!(EnemyKind::Guide.npc_id(), 22);
        assert_eq!(EnemyKind::ArmsDealer.npc_id(), 19);
        assert_eq!(EnemyKind::Merchant.stats().hp, 250);
        assert_eq!(EnemyKind::ArmsDealer.stats().hp, 250);
        assert!(
            EnemyKind::Guide.is_friendly()
                && EnemyKind::ArmsDealer.is_friendly()
                && !EnemyKind::Zombie.is_friendly()
        );
        assert_eq!(TownKind::from_u8(2), Some(TownKind::ArmsDealer));
        assert!(arms_dealer_may_spawn(true, false, true));
        assert!(!arms_dealer_may_spawn(false, false, true));
        assert!(!arms_dealer_may_spawn(true, true, true));
        assert!(!arms_dealer_may_spawn(true, false, false));
    }

    #[test]
    fn merchant_needs_5000_copper() {
        assert!(!merchant_may_spawn(4999, true, false, true, false));
        assert!(merchant_may_spawn(5000, true, false, true, false));
    }

    #[test]
    fn merchant_needs_valid_house() {
        assert!(!merchant_may_spawn(9999, false, false, true, false));
        assert!(!merchant_may_spawn(9999, true, true, true, false));
        assert!(!merchant_may_spawn(9999, true, false, false, false));
        assert!(!merchant_may_spawn(9999, true, false, true, true));
    }

    #[test]
    fn npc_moves_into_valid_house() {
        let mut hs = [h(0, None), h(50, None)];
        let m = assign_homes(&mut hs, &[(TownKind::Guide, [48.0, 0.0, 0.0])]);
        assert_eq!(m, vec![(TownKind::Guide, 1)]);
        assert_eq!(hs[1].npc, Some(TownKind::Guide));
        assert!(assign_homes(&mut [], &[(TownKind::Guide, [0.0; 3])]).is_empty());
    }

    #[test]
    fn npc_homes_are_unique_per_house() {
        let mut hs = [h(0, None)];
        let both = [(TownKind::Guide, [0.0; 3]), (TownKind::Merchant, [0.0; 3])];
        assert_eq!(assign_homes(&mut hs, &both).len(), 1);
        assert_eq!(hs[0].npc, Some(TownKind::Guide));
        // A resident is not moved again, and a second kind needs a second house.
        assert!(assign_homes(&mut hs, &both).is_empty());
    }

    #[test]
    fn wander_stays_within_radius() {
        let anchor = [0.5, 0.0, 0.5];
        let mut pos = anchor;
        let (mut st, mut rng) = (Wander::default(), Rng::new(7));
        let mut far: f32 = 0.0;
        for _ in 0..60 * 600 {
            let ctx = Ctx {
                pos,
                anchor,
                has_home: true,
                is_day: true,
                ledge_ahead: false,
                wall_ahead: false,
                step_ahead: false,
            };
            let it = step(&mut st, &mut rng, 1.0 / 60.0, &ctx);
            pos[0] += it.dir[0] * WALK_SPEED;
            pos[2] += it.dir[1] * WALK_SPEED;
            far = far.max(dist_xz(pos, anchor));
        }
        assert!(far > 3.0, "never wandered: {far}");
        assert!(far <= WANDER_RADIUS, "left the radius: {far}");
    }

    #[test]
    fn night_sends_home_and_sleeps() {
        let anchor = [0.5, 0.0, 0.5];
        let (mut st, mut rng) = (Wander::default(), Rng::new(1));
        let mut ctx = Ctx {
            pos: [8.5, 0.0, 0.5],
            anchor,
            has_home: true,
            is_day: false,
            ledge_ahead: false,
            wall_ahead: false,
            step_ahead: false,
        };
        let it = step(&mut st, &mut rng, 1.0 / 60.0, &ctx);
        assert!(it.dir[0] < -0.9);
        ctx.pos = anchor;
        assert_eq!(step(&mut st, &mut rng, 1.0 / 60.0, &ctx), Intent::default());
        assert_eq!(st.mode, Mode::Sleep);
    }

    struct Floor(Vec<[i32; 3]>);
    impl BlockQuery for Floor {
        fn block(&self, x: i32, y: i32, z: i32) -> BlockId {
            if y < 0 || self.0.contains(&[x, y, z]) {
                DIRT
            } else {
                AIR
            }
        }
    }

    #[test]
    fn probe_sees_step_wall_and_ledge() {
        let step_q = Floor(vec![[1, 0, 0]]);
        let (l, w, s) = probe(&step_q, [0.5, 0.0, 0.5], [1.0, 0.0], 0.4);
        assert!(!l && w && s);
        let wall_q = Floor(vec![[1, 0, 0], [1, 1, 0], [1, 2, 0]]);
        let (_, w, s) = probe(&wall_q, [0.5, 0.0, 0.5], [1.0, 0.0], 0.4);
        assert!(w && !s);
        let ledge_q = Floor(vec![]);
        let (l, ..) = probe(&ledge_q, [0.5, 0.0, 0.5], [0.0, -1.0], 0.4);
        assert!(!l, "flat ground is not a ledge");
        struct Pit;
        impl BlockQuery for Pit {
            fn block(&self, x: i32, y: i32, _z: i32) -> BlockId {
                if y < 0 && x < 1 { DIRT } else { AIR }
            }
        }
        assert!(probe(&Pit, [0.5, 0.0, 0.5], [1.0, 0.0], 0.4).0);
    }

    #[test]
    fn tick_town_walks_and_stays_on_floor() {
        let q = Floor(vec![]);
        let mut e = Enemy::new(crate::enemy::EnemyId(1), EnemyKind::Guide, [0.5, 0.0, 0.5]);
        let (mut st, mut rng) = (Wander::default(), Rng::new(3));
        for _ in 0..600 {
            tick_town(&q, &mut e, &mut st, &mut rng, [0.5, 0.0, 0.5], false, true);
        }
        assert!(e.pos[1] >= -0.01 && e.pos[1] < 0.5);
        assert!(dist_xz(e.pos, [0.5, 0.0, 0.5]) <= WANDER_RADIUS);
    }

    #[test]
    fn town_npc_wanders_by_day_returns_home_at_night_without_wall_clip() {
        let anchor = [0.5, 0.0, 0.5];
        let mut pos = anchor;
        let (mut st, mut rng) = (Wander::default(), Rng::new(42));

        // Day: wanders deterministically within WANDER_RADIUS.
        let mut max_d: f32 = 0.0;
        for _ in 0..60 * 120 {
            let ctx = Ctx {
                pos,
                anchor,
                has_home: true,
                is_day: true,
                ledge_ahead: false,
                wall_ahead: false,
                step_ahead: false,
            };
            let it = step_town_npc_schedule(&mut st, &mut rng, 1.0 / 60.0, &ctx);
            pos[0] += it.dir[0] * WALK_SPEED;
            pos[2] += it.dir[1] * WALK_SPEED;
            max_d = max_d.max(dist_xz(pos, anchor));
        }
        assert!(max_d > 1.0 && max_d <= WANDER_RADIUS);

        // Night: returns home, but stops safely before a wall or cliff.
        let blocked_wall = Ctx {
            pos: [6.5, 0.0, 0.5],
            anchor,
            has_home: true,
            is_day: false,
            ledge_ahead: false,
            wall_ahead: true,
            step_ahead: false,
        };
        assert_eq!(
            step_town_npc_schedule(&mut st, &mut rng, 1.0 / 60.0, &blocked_wall),
            Intent::default()
        );

        let blocked_cliff = Ctx {
            pos: [6.5, 0.0, 0.5],
            anchor,
            has_home: true,
            is_day: false,
            ledge_ahead: true,
            wall_ahead: false,
            step_ahead: false,
        };
        assert_eq!(
            step_town_npc_schedule(&mut st, &mut rng, 1.0 / 60.0, &blocked_cliff),
            Intent::default()
        );

        // With a physical wall at x = 3, tick_town at night never clips through it.
        let wall_q = Floor(vec![
            [3, 0, 0],
            [3, 1, 0],
            [3, 2, 0],
            [3, 0, 1],
            [3, 1, 1],
            [3, 2, 1],
        ]);
        let mut e = Enemy::new(crate::enemy::EnemyId(2), EnemyKind::Guide, [6.5, 0.0, 0.5]);
        let (mut st2, mut rng2) = (Wander::default(), Rng::new(9));
        for _ in 0..300 {
            tick_town(&wall_q, &mut e, &mut st2, &mut rng2, anchor, true, false);
        }
        assert!(e.pos[0] >= 4.0, "clipped through wall: {:?}", e.pos);

        // With a cliff at x = 4 (no floor for x <= 4), tick_town at night never falls off.
        struct Cliff;
        impl BlockQuery for Cliff {
            fn block(&self, x: i32, y: i32, _z: i32) -> BlockId {
                if y < 0 && x >= 5 { DIRT } else { AIR }
            }
        }
        let mut e2 = Enemy::new(crate::enemy::EnemyId(3), EnemyKind::Guide, [6.5, 0.0, 0.5]);
        let (mut st3, mut rng3) = (Wander::default(), Rng::new(11));
        for _ in 0..300 {
            tick_town(&Cliff, &mut e2, &mut st3, &mut rng3, anchor, true, false);
        }
        assert!(e2.pos[1] >= -0.01, "fell off cliff: {:?}", e2.pos);
    }
}
