//! Enemy kinds, stats and movement AI. Pure and deterministic: the server owns
//! every `Enemy`; clients only render the state they are sent.

use crate::block::BlockId;
use crate::chunk::BlockQuery;
use crate::player::consts::{GRAVITY, MAX_FALL_SPEED, PX_TO_BLOCKS as PX};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EnemyKind {
    Slime,
    DemonEye,
    EaterOfSouls,
    Zombie,
    /// The boss. Its AI lives in `boss::EyeBoss`; `tick_enemy` leaves it alone.
    EyeOfCthulhu,
    /// Servant of Cthulhu spawned during Eye of Cthulhu phase 1.
    ServantOfCthulhu,
    /// Town NPCs (`town_npc`): friendly, never hurt by or hurting anyone.
    Guide,
    Merchant,
    ArmsDealer,
    Nurse,
    Demolitionist,
    Dryad,
    GoblinTinkerer,
    Skeleton,
    CaveBat,
    GiantWorm,
    EaterOfWorldsHead,
    EaterOfWorldsBody,
    EaterOfWorldsTail,
    OldMan,
    DungeonGuardian,
    SkeletronHead,
    SkeletronHand,
    AngryBones,
    DarkCaster,
    CursedSkull,
    FireImp,
    BoneSerpentHead,
    BoneSerpentBody,
    BoneSerpentTail,
    LavaSlime,
    Hellbat,
    Demon,
    VoodooDemon,
    Bunny,
    Bird,
    Squirrel,
    CaveCritter,
    WallOfFleshMouth,
    WallOfFleshEye,
    TheHungry,
    Leech,
    KingSlime,
    QueenBee,
    Bee,
    // Wave 5: Events & Biome Enemies
    BloodZombie,
    Drippler,
    GoblinPeon,
    GoblinThief,
    GoblinWarrior,
    GoblinSorcerer,
    GoblinArcher,
    Antlion,
    Vulture,
    IceSlime,
    SnowFlinx,
    JungleSlime,
    Hornet,
    ManEater,
    Harpy,
    Shark,
    Crab,
    Piranha,
}

pub const MAX_CRITTERS: usize = 12;
pub const CRITTER_DESPAWN_DIST: f32 = 64.0;
pub const CRITTER_FLEE_RADIUS: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnemyStats {
    pub hp: i32,
    pub damage: i32,
    pub defense: i32,
    /// Copper coins dropped (NPC.value).
    pub value: i32,
    /// 0 = full knockback, 1 = immune (NPC.knockBackResist).
    pub knockback_resist: f32,
    /// Half width (x and z) and half height of the hitbox, in blocks.
    pub half: [f32; 2],
}

impl EnemyKind {
    /// Terraria NPC type id.
    pub fn npc_id(self) -> i32 {
        match self {
            EnemyKind::Slime => 1,              // NPCID.BlueSlime
            EnemyKind::DemonEye => 2,           // NPCID.DemonEye
            EnemyKind::EaterOfSouls => 6,       // NPCID.EaterofSouls (gameplay spec)
            EnemyKind::Zombie => 3,             // NPCID.Zombie
            EnemyKind::EyeOfCthulhu => 4,       // NPCID.EyeofCthulhu
            EnemyKind::ServantOfCthulhu => 5,   // NPCID.ServantofCthulhu (gameplay spec)
            EnemyKind::Guide => 22,             // NPCID.Guide (gameplay spec)
            EnemyKind::Merchant => 17,          // NPCID.Merchant (gameplay spec)
            EnemyKind::ArmsDealer => 19,        // NPCID.ArmsDealer (gameplay spec)
            EnemyKind::Nurse => 18,             // NPCID.Nurse
            EnemyKind::Demolitionist => 38,     // NPCID.Demolitionist
            EnemyKind::Dryad => 20,             // NPCID.Dryad
            EnemyKind::GoblinTinkerer => 107,   // NPCID.GoblinTinkerer
            EnemyKind::Skeleton => 21,          // NPCID.Skeleton
            EnemyKind::CaveBat => 49,           // NPCID.CaveBat
            EnemyKind::GiantWorm => 10,         // NPCID.GiantWormHead
            EnemyKind::EaterOfWorldsHead => 13, // NPCID.EaterofWorldsHead
            EnemyKind::EaterOfWorldsBody => 14, // NPCID.EaterofWorldsBody
            EnemyKind::EaterOfWorldsTail => 15, // NPCID.EaterofWorldsTail
            EnemyKind::OldMan => 37,            // NPCID.OldMan
            EnemyKind::DungeonGuardian => 68,   // NPCID.DungeonGuardian
            EnemyKind::SkeletronHead => 35,     // NPCID.SkeletronHead
            EnemyKind::SkeletronHand => 36,     // NPCID.SkeletronHand
            EnemyKind::AngryBones => 31,        // NPCID.AngryBones
            EnemyKind::DarkCaster => 32,        // NPCID.DarkCaster
            EnemyKind::CursedSkull => 34,       // NPCID.CursedSkull
            EnemyKind::FireImp => 24,           // NPCID.FireImp
            EnemyKind::BoneSerpentHead => 39,   // NPCID.BoneSerpentHead
            EnemyKind::BoneSerpentBody => 40,   // NPCID.BoneSerpentBody
            EnemyKind::BoneSerpentTail => 41,   // NPCID.BoneSerpentTail
            EnemyKind::LavaSlime => 59,         // NPCID.LavaSlime
            EnemyKind::Hellbat => 60,           // NPCID.Hellbat
            EnemyKind::Demon => 62,             // NPCID.Demon
            EnemyKind::VoodooDemon => 66,       // NPCID.VoodooDemon
            EnemyKind::Bunny => 46,             // NPCID.Bunny
            EnemyKind::Bird => 74,              // NPCID.Bird
            EnemyKind::Squirrel => 299,         // NPCID.Squirrel
            EnemyKind::CaveCritter => 300,
            EnemyKind::WallOfFleshMouth => 113, // NPCID.WallofFlesh
            EnemyKind::WallOfFleshEye => 114,   // NPCID.WallofFleshEye
            EnemyKind::TheHungry => 115,        // NPCID.TheHungry
            EnemyKind::Leech => 117,            // NPCID.LeechHead
            EnemyKind::KingSlime => 50,         // NPCID.KingSlime
            EnemyKind::QueenBee => 222,         // NPCID.QueenBee
            EnemyKind::Bee => 210,              // NPCID.Bee
            EnemyKind::BloodZombie => 489,      // NPCID.BloodZombie
            EnemyKind::Drippler => 490,         // NPCID.Drippler
            EnemyKind::GoblinPeon => 26,        // NPCID.GoblinPeon
            EnemyKind::GoblinThief => 27,       // NPCID.GoblinThief
            EnemyKind::GoblinWarrior => 28,     // NPCID.GoblinWarrior
            EnemyKind::GoblinSorcerer => 29,    // NPCID.GoblinSorcerer
            EnemyKind::GoblinArcher => 111,     // NPCID.GoblinArcher
            EnemyKind::Antlion => 69,           // NPCID.Antlion
            EnemyKind::Vulture => 61,           // NPCID.Vulture
            EnemyKind::IceSlime => 147,         // NPCID.IceSlime
            EnemyKind::SnowFlinx => 185,        // NPCID.SnowFlinx
            EnemyKind::JungleSlime => 244,      // NPCID.JungleSlime
            EnemyKind::Hornet => 42,            // NPCID.Hornet
            EnemyKind::ManEater => 43,          // NPCID.ManEater
            EnemyKind::Harpy => 48,             // NPCID.Harpy
            EnemyKind::Shark => 65,             // NPCID.Shark
            EnemyKind::Crab => 67,              // NPCID.Crab
            EnemyKind::Piranha => 58,           // NPCID.Piranha
        }
    }

    /// Ambient critters: flee nearby players, no combat, no drops, separate cap/despawn.
    pub fn is_critter(self) -> bool {
        matches!(
            self,
            EnemyKind::Bunny | EnemyKind::Bird | EnemyKind::Squirrel | EnemyKind::CaveCritter
        )
    }

    /// Town NPCs are `friendly` (gameplay spec): no contact damage, not hit by
    /// players, not counted by spawn caps or distance despawn.
    pub fn is_friendly(self) -> bool {
        matches!(
            self,
            EnemyKind::Guide
                | EnemyKind::Merchant
                | EnemyKind::ArmsDealer
                | EnemyKind::Nurse
                | EnemyKind::Demolitionist
                | EnemyKind::Dryad
                | EnemyKind::GoblinTinkerer
                | EnemyKind::OldMan
        )
    }

    /// True for boss and boss-segment enemy kinds.
    pub fn is_boss(self) -> bool {
        matches!(
            self,
            EnemyKind::EyeOfCthulhu
                | EnemyKind::EaterOfWorldsHead
                | EnemyKind::EaterOfWorldsBody
                | EnemyKind::EaterOfWorldsTail
                | EnemyKind::SkeletronHead
                | EnemyKind::SkeletronHand
                | EnemyKind::WallOfFleshMouth
                | EnemyKind::WallOfFleshEye
                | EnemyKind::KingSlime
                | EnemyKind::QueenBee
        )
    }

    /// NPC.item defaults: BlueSlime gameplay spec, DemonEye 8621-8632,
    /// Zombie 8634-8645. Half sizes are half of the hitbox in px.
    pub fn stats(self) -> EnemyStats {
        match self {
            EnemyKind::Slime => EnemyStats {
                hp: 25,
                damage: 7,
                defense: 2,
                value: 25,
                knockback_resist: 0.0,
                half: [12.0 * PX, 9.0 * PX],
            },
            EnemyKind::DemonEye => EnemyStats {
                hp: 60,
                damage: 18,
                defense: 2,
                value: 75,
                knockback_resist: 0.0,
                half: [15.0 * PX, 16.0 * PX],
            },
            // NPC.item defaults type 6 (gameplay spec): 30x30, aiStyle 5,
            // noGravity; noTileCollide is NOT set, so normal block collision applies.
            EnemyKind::EaterOfSouls => EnemyStats {
                hp: 40,
                damage: 22,
                defense: 8,
                value: 90,
                knockback_resist: 0.5,
                half: [15.0 * PX, 15.0 * PX],
            },
            EnemyKind::Zombie => EnemyStats {
                hp: 45,
                damage: 14,
                defense: 6,
                value: 60,
                knockback_resist: 0.0,
                half: [9.0 * PX, 20.0 * PX],
            },
            // NPC.item defaults type 4 (gameplay spec): 100x110, damage 15, defense 12,
            // lifeMax 2800, knockBackResist 0 (immune in Terraria), value 30000.
            EnemyKind::EyeOfCthulhu => EnemyStats {
                hp: 2800,
                damage: 15,
                defense: 12,
                value: 30000,
                knockback_resist: 1.0,
                half: [50.0 * PX, 55.0 * PX],
            },
            // NPC.item defaults type 5 (gameplay spec): 20x20, damage 12, defense 0,
            // lifeMax 8, aiStyle 5, noGravity, noTileCollide. Drops nothing (value 0).
            EnemyKind::ServantOfCthulhu => EnemyStats {
                hp: 8,
                damage: 12,
                defense: 0,
                value: 0,
                knockback_resist: 0.0,
                half: [10.0 * PX, 10.0 * PX],
            },
            // NPC.item defaults type 22 (gameplay spec): 18x40, defense 30, lifeMax 250,
            // knockBackResist 0.5. Contact damage is 0 here: friendly, never hurts.
            EnemyKind::Guide => EnemyStats {
                hp: 250,
                damage: 0,
                defense: 30,
                value: 0,
                knockback_resist: 0.5,
                half: [9.0 * PX, 20.0 * PX],
            },
            // NPC.item defaults type 17 / 19 (gameplay spec): 18x40, defense 15, lifeMax 250.
            EnemyKind::Merchant
            | EnemyKind::ArmsDealer
            | EnemyKind::Nurse
            | EnemyKind::Demolitionist
            | EnemyKind::Dryad
            | EnemyKind::GoblinTinkerer => EnemyStats {
                hp: 250,
                damage: 0,
                defense: 15,
                value: 0,
                knockback_resist: 0.5,
                half: [9.0 * PX, 20.0 * PX],
            },
            // NPC.item defaults type 21 (gameplay spec): 18x40, damage 20, defense 8,
            // lifeMax 60, knockBackResist 0.5, value 100.
            EnemyKind::Skeleton => EnemyStats {
                hp: 60,
                damage: 20,
                defense: 8,
                value: 100,
                knockback_resist: 0.5,
                half: [9.0 * PX, 20.0 * PX],
            },
            // NPC.item defaults type 49 (gameplay spec): 22x18, damage 13, defense 2,
            // lifeMax 16, knockBackResist 0.8, value 90.
            EnemyKind::CaveBat => EnemyStats {
                hp: 16,
                damage: 13,
                defense: 2,
                value: 90,
                knockback_resist: 0.8,
                half: [11.0 * PX, 9.0 * PX],
            },
            // NPC.item defaults type 10 (gameplay spec): 14x14, damage 14 (brief spec / classic digger),
            // defense 4, lifeMax 30, knockBackResist 0, value 40.
            EnemyKind::GiantWorm => EnemyStats {
                hp: 30,
                damage: 14,
                defense: 4,
                value: 40,
                knockback_resist: 0.0,
                half: [7.0 * PX, 7.0 * PX],
            },
            EnemyKind::EaterOfWorldsHead => EnemyStats {
                hp: 150,
                damage: 22,
                defense: 2,
                value: 800,
                knockback_resist: 0.0,
                half: [19.0 * PX, 19.0 * PX],
            },
            EnemyKind::EaterOfWorldsBody => EnemyStats {
                hp: 150,
                damage: 13,
                defense: 4,
                value: 800,
                knockback_resist: 0.0,
                half: [19.0 * PX, 19.0 * PX],
            },
            EnemyKind::EaterOfWorldsTail => EnemyStats {
                hp: 150,
                damage: 11,
                defense: 8,
                value: 800,
                knockback_resist: 0.0,
                half: [19.0 * PX, 19.0 * PX],
            },
            EnemyKind::OldMan => EnemyStats {
                hp: 250,
                damage: 0,
                defense: 15,
                value: 0,
                knockback_resist: 0.0,
                half: [9.0 * PX, 20.0 * PX],
            },
            EnemyKind::DungeonGuardian => EnemyStats {
                hp: 9999,
                damage: 9999,
                defense: 9999,
                value: 0,
                knockback_resist: 0.0,
                half: [15.0 * PX, 15.0 * PX],
            },
            EnemyKind::SkeletronHead => EnemyStats {
                hp: 4400,
                damage: 32,
                defense: 10,
                value: 50000,
                knockback_resist: 1.0,
                half: [20.0 * PX, 20.0 * PX],
            },
            EnemyKind::SkeletronHand => EnemyStats {
                hp: 600,
                damage: 20,
                defense: 14,
                value: 0,
                knockback_resist: 1.0,
                half: [10.0 * PX, 10.0 * PX],
            },
            EnemyKind::AngryBones => EnemyStats {
                hp: 80,
                damage: 26,
                defense: 8,
                value: 150,
                knockback_resist: 0.8,
                half: [9.0 * PX, 21.0 * PX],
            },
            EnemyKind::DarkCaster => EnemyStats {
                hp: 50,
                damage: 20,
                defense: 2,
                value: 140,
                knockback_resist: 0.6,
                half: [9.0 * PX, 20.0 * PX],
            },
            EnemyKind::CursedSkull => EnemyStats {
                hp: 40,
                damage: 35,
                defense: 6,
                value: 120,
                knockback_resist: 0.2,
                half: [13.0 * PX, 13.0 * PX],
            },
            EnemyKind::FireImp => EnemyStats {
                hp: 70,
                damage: 30,
                defense: 16,
                value: 350,
                knockback_resist: 0.5,
                half: [9.0 * PX, 20.0 * PX],
            },
            EnemyKind::BoneSerpentHead => EnemyStats {
                hp: 300,
                damage: 36,
                defense: 12,
                value: 1200,
                knockback_resist: 0.0,
                half: [11.0 * PX, 11.0 * PX],
            },
            EnemyKind::BoneSerpentBody => EnemyStats {
                hp: 300,
                damage: 20,
                defense: 18,
                value: 1200,
                knockback_resist: 0.0,
                half: [11.0 * PX, 11.0 * PX],
            },
            EnemyKind::BoneSerpentTail => EnemyStats {
                hp: 300,
                damage: 16,
                defense: 18,
                value: 1200,
                knockback_resist: 0.0,
                half: [11.0 * PX, 11.0 * PX],
            },
            EnemyKind::LavaSlime => EnemyStats {
                hp: 50,
                damage: 15,
                defense: 10,
                value: 120,
                knockback_resist: 0.0,
                half: [12.0 * PX, 9.0 * PX],
            },
            EnemyKind::Hellbat => EnemyStats {
                hp: 46,
                damage: 35,
                defense: 8,
                value: 120,
                knockback_resist: 0.8,
                half: [11.0 * PX, 9.0 * PX],
            },
            EnemyKind::Demon => EnemyStats {
                hp: 120,
                damage: 32,
                defense: 8,
                value: 300,
                knockback_resist: 0.8,
                half: [14.0 * PX, 24.0 * PX],
            },
            EnemyKind::VoodooDemon => EnemyStats {
                hp: 140,
                damage: 32,
                defense: 8,
                value: 1000,
                knockback_resist: 0.8,
                half: [14.0 * PX, 24.0 * PX],
            },
            EnemyKind::Bunny => EnemyStats {
                hp: 5,
                damage: 0,
                defense: 0,
                value: 0,
                knockback_resist: 0.0,
                half: [6.0 * PX, 7.0 * PX],
            },
            EnemyKind::Bird => EnemyStats {
                hp: 5,
                damage: 0,
                defense: 0,
                value: 0,
                knockback_resist: 0.0,
                half: [5.0 * PX, 5.0 * PX],
            },
            EnemyKind::Squirrel => EnemyStats {
                hp: 5,
                damage: 0,
                defense: 0,
                value: 0,
                knockback_resist: 0.0,
                half: [6.0 * PX, 6.0 * PX],
            },
            EnemyKind::CaveCritter => EnemyStats {
                hp: 5,
                damage: 0,
                defense: 0,
                value: 0,
                knockback_resist: 0.0,
                half: [6.0 * PX, 5.0 * PX],
            },
            EnemyKind::WallOfFleshMouth => EnemyStats {
                hp: 8000,
                damage: 50,
                defense: 12,
                value: 80000,
                knockback_resist: 1.0,
                half: [1.5, 2.2],
            },
            EnemyKind::WallOfFleshEye => EnemyStats {
                hp: 8000,
                damage: 50,
                defense: 0,
                value: 0,
                knockback_resist: 1.0,
                half: [1.5, 1.5],
            },
            EnemyKind::TheHungry => EnemyStats {
                hp: 250,
                damage: 30,
                defense: 10,
                value: 0,
                knockback_resist: 0.5,
                half: [0.7, 0.7],
            },
            EnemyKind::Leech => EnemyStats {
                hp: 60,
                damage: 26,
                defense: 2,
                value: 0,
                knockback_resist: 0.2,
                half: [0.6, 0.6],
            },
            EnemyKind::KingSlime => EnemyStats {
                hp: 2000,
                damage: 40,
                defense: 10,
                value: 10000,
                knockback_resist: 1.0,
                half: [34.0 * PX, 30.0 * PX],
            },
            EnemyKind::QueenBee => EnemyStats {
                hp: 3400,
                damage: 30,
                defense: 8,
                value: 50000,
                knockback_resist: 1.0,
                half: [24.0 * PX, 24.0 * PX],
            },
            EnemyKind::Bee => EnemyStats {
                hp: 18,
                damage: 12,
                defense: 0,
                value: 0,
                knockback_resist: 0.2,
                half: [6.0 * PX, 6.0 * PX],
            },
            EnemyKind::BloodZombie => EnemyStats {
                hp: 75,
                damage: 18,
                defense: 6,
                value: 200,
                knockback_resist: 0.4,
                half: [9.0 * PX, 20.0 * PX],
            },
            EnemyKind::Drippler => EnemyStats {
                hp: 80,
                damage: 20,
                defense: 6,
                value: 250,
                knockback_resist: 0.5,
                half: [12.0 * PX, 12.0 * PX],
            },
            EnemyKind::GoblinPeon => EnemyStats {
                hp: 60,
                damage: 12,
                defense: 4,
                value: 100,
                knockback_resist: 0.5,
                half: [9.0 * PX, 20.0 * PX],
            },
            EnemyKind::GoblinThief => EnemyStats {
                hp: 80,
                damage: 20,
                defense: 6,
                value: 150,
                knockback_resist: 0.4,
                half: [9.0 * PX, 20.0 * PX],
            },
            EnemyKind::GoblinWarrior => EnemyStats {
                hp: 110,
                damage: 25,
                defense: 12,
                value: 250,
                knockback_resist: 0.7,
                half: [10.0 * PX, 21.0 * PX],
            },
            EnemyKind::GoblinSorcerer => EnemyStats {
                hp: 40,
                damage: 20,
                defense: 2,
                value: 200,
                knockback_resist: 0.3,
                half: [9.0 * PX, 20.0 * PX],
            },
            EnemyKind::GoblinArcher => EnemyStats {
                hp: 80,
                damage: 18,
                defense: 4,
                value: 150,
                knockback_resist: 0.4,
                half: [9.0 * PX, 20.0 * PX],
            },
            EnemyKind::Antlion => EnemyStats {
                hp: 45,
                damage: 10,
                defense: 6,
                value: 60,
                knockback_resist: 1.0,
                half: [12.0 * PX, 12.0 * PX],
            },
            EnemyKind::Vulture => EnemyStats {
                hp: 40,
                damage: 15,
                defense: 4,
                value: 90,
                knockback_resist: 0.5,
                half: [12.0 * PX, 12.0 * PX],
            },
            EnemyKind::IceSlime => EnemyStats {
                hp: 30,
                damage: 8,
                defense: 4,
                value: 40,
                knockback_resist: 0.0,
                half: [12.0 * PX, 9.0 * PX],
            },
            EnemyKind::SnowFlinx => EnemyStats {
                hp: 55,
                damage: 14,
                defense: 6,
                value: 100,
                knockback_resist: 0.3,
                half: [11.0 * PX, 12.0 * PX],
            },
            EnemyKind::JungleSlime => EnemyStats {
                hp: 60,
                damage: 18,
                defense: 6,
                value: 110,
                knockback_resist: 0.0,
                half: [13.0 * PX, 10.0 * PX],
            },
            EnemyKind::Hornet => EnemyStats {
                hp: 48,
                damage: 24,
                defense: 10,
                value: 150,
                knockback_resist: 0.5,
                half: [11.0 * PX, 11.0 * PX],
            },
            EnemyKind::ManEater => EnemyStats {
                hp: 110,
                damage: 34,
                defense: 14,
                value: 200,
                knockback_resist: 0.8,
                half: [12.0 * PX, 14.0 * PX],
            },
            EnemyKind::Harpy => EnemyStats {
                hp: 100,
                damage: 25,
                defense: 8,
                value: 300,
                knockback_resist: 0.5,
                half: [12.0 * PX, 16.0 * PX],
            },
            EnemyKind::Shark => EnemyStats {
                hp: 300,
                damage: 40,
                defense: 2,
                value: 500,
                knockback_resist: 0.7,
                half: [20.0 * PX, 10.0 * PX],
            },
            EnemyKind::Crab => EnemyStats {
                hp: 40,
                damage: 20,
                defense: 10,
                value: 100,
                knockback_resist: 0.5,
                half: [11.0 * PX, 9.0 * PX],
            },
            EnemyKind::Piranha => EnemyStats {
                hp: 30,
                damage: 22,
                defense: 2,
                value: 80,
                knockback_resist: 0.3,
                half: [10.0 * PX, 8.0 * PX],
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EnemyId(pub u32);

/// `pos` is the feet centre; the box spans `half[0]` either side in x/z and
/// `2 * half[1]` up from the feet.
#[derive(Clone, Debug, PartialEq)]
pub struct Enemy {
    pub id: EnemyId,
    pub kind: EnemyKind,
    pub pos: [f32; 3],
    pub vel: [f32; 3],
    pub on_ground: bool,
    pub hp: i32,
    /// NPC.ai[0..4]; slime uses [0] as its ground-tick counter.
    pub ai: [i32; 4],
    /// Ticks of knockback left, during which only physics runs.
    pub hurt_ticks: i32,
    /// Buff 24 (On Fire) ticks remaining. Deals 4 hp/s DoT (1 dmg / 15 ticks).
    pub on_fire_ticks: i32,
}

impl Enemy {
    pub fn new(id: EnemyId, kind: EnemyKind, pos: [f32; 3]) -> Self {
        Enemy {
            id,
            kind,
            pos,
            vel: [0.0; 3],
            on_ground: false,
            hp: kind.stats().hp,
            ai: [0; 4],
            hurt_ticks: 0,
            on_fire_ticks: 0,
        }
    }

    pub fn is_on_fire(&self) -> bool {
        self.on_fire_ticks > 0
    }

    pub fn apply_on_fire(&mut self, ticks: i32) {
        self.on_fire_ticks = self.on_fire_ticks.max(ticks);
    }

    /// Ticks On Fire debuff. Deals 4 hp/s DoT (1 damage every 15 ticks).
    /// Returns true if killed by fire.
    pub fn tick_on_fire(&mut self) -> bool {
        if self.on_fire_ticks <= 0 {
            return false;
        }
        self.on_fire_ticks -= 1;
        if self.on_fire_ticks % 15 == 0 {
            self.hp -= 1;
            if self.hp <= 0 {
                self.on_fire_ticks = 0;
                return true;
            }
        }
        false
    }
}

impl Enemy {
    /// Current defense. The Eye of Cthulhu drops to 0 in phase 2 (gameplay spec).
    pub fn defense(&self) -> i32 {
        if self.kind == EnemyKind::EyeOfCthulhu && self.ai[0] == 3 {
            return 0;
        }
        if self.kind == EnemyKind::SkeletronHead {
            if self.ai[2] == 1 {
                return 9999; // Daytime Dungeon Guardian enrage
            }
            if self.ai[1] == 1 {
                return 0;
            }
            return self.kind.stats().defense + 25 * self.ai[3].max(0);
        }
        if self.kind == EnemyKind::TheHungry {
            return 10 + (self.ai[2].clamp(0, 3) * 5);
        }
        self.kind.stats().defense
    }

    /// Current contact damage. Phase 2 of the Eye uses 23 (gameplay spec,20469`, classic mode).
    pub fn damage(&self) -> i32 {
        if self.kind == EnemyKind::EyeOfCthulhu && self.ai[0] == 3 {
            return 23;
        }
        if self.kind == EnemyKind::SkeletronHead {
            if self.ai[2] == 1 {
                return 1000; // Daytime Dungeon Guardian enrage
            }
            if self.ai[1] == 1 {
                return 64;
            }
        }
        if self.kind == EnemyKind::TheHungry {
            return 30 + (self.ai[2].clamp(0, 3) * 10);
        }
        self.kind.stats().damage
    }
}

const EPS: f32 = 1e-5;
const MAX_SUBSTEP: f32 = 0.25;
/// Wind-up duration in ticks (~200 ms at 60 Hz) before ranged enemies fire.
pub const RANGED_WINDUP_TICKS: i32 = 12;

fn start_ranged_windup(e: &mut Enemy) {
    e.ai[2] = RANGED_WINDUP_TICKS;
    e.ai[3] = 1;
}

fn tick_ranged_windup(e: &mut Enemy) {
    if e.ai[2] > 0 {
        e.ai[2] -= 1;
        if e.ai[2] == 0 {
            e.ai[1] = 1;
        }
    }
}
/// Ticks between slime hops while on the ground (gameplay spec interval).
const SLIME_HOP_TICKS: i32 = 120;
/// Slime ground friction while not hopping.
const SLIME_FRICTION: f32 = 0.8;
/// Zombie walk acceleration and top speed, px/tick (AI_003_Fighters).
const ZOMBIE_ACCEL: f32 = 0.1 * PX;
const ZOMBIE_MAX: f32 = 1.0 * PX;
/// Demon eye acceleration and cap (gameplay spec).
const EYE_ACCEL: f32 = 0.08 * PX;
const EYE_MAX: f32 = 4.0 * PX;
/// AI_005_EaterOfSouls, classic (gameplay spec).
const EATER_ACCEL: f32 = 0.02 * PX;
const EATER_MAX: f32 = 4.0 * PX;
/// AI_005_ServantOfCthulhu, classic (gameplay spec).
const SERVANT_ACCEL: f32 = 0.03 * PX;
const SERVANT_MAX: f32 = 5.0 * PX;
/// Initial jump speed shared by slime hops and zombie step jumps (gameplay spec).
pub(crate) const JUMP_SPEED: f32 = 6.0 * PX;

fn overlaps(q: &impl BlockQuery, pos: [f32; 3], half: [f32; 2]) -> bool {
    let lo = [pos[0] - half[0], pos[1], pos[2] - half[0]];
    let hi = [pos[0] + half[0], pos[1] + 2.0 * half[1], pos[2] + half[0]];
    let r = |a: usize| ((lo[a] + EPS).floor() as i32, (hi[a] - EPS).floor() as i32);
    let (x0, x1) = r(0);
    let (y0, y1) = r(1);
    let (z0, z1) = r(2);
    for x in x0..=x1 {
        for y in y0..=y1 {
            for z in z0..=z1 {
                let b: BlockId = q.block(x, y, z);
                if b.is_solid() {
                    return true;
                }
            }
        }
    }
    false
}

/// Sub-stepped axis move that stops flush against solids; true if blocked.
fn move_axis(q: &impl BlockQuery, pos: &mut [f32; 3], half: [f32; 2], axis: usize, d: f32) -> bool {
    if d == 0.0 {
        return false;
    }
    let (lo, hi) = if axis == 1 {
        (0.0, 2.0 * half[1])
    } else {
        (half[0], half[0])
    };
    let n = (d.abs() / MAX_SUBSTEP).ceil().max(1.0) as i32;
    let step = d / n as f32;
    for _ in 0..n {
        let mut next = *pos;
        next[axis] += step;
        if overlaps(q, next, half) {
            pos[axis] = if step > 0.0 {
                (next[axis] + hi - EPS).floor() - hi
            } else {
                (next[axis] - lo + EPS).floor() + 1.0 + lo
            };
            return true;
        }
        *pos = next;
    }
    false
}

/// Unit vector in the x/z plane from `from` toward `to` (zero if coincident).
fn dir_xz(from: [f32; 3], to: [f32; 3]) -> [f32; 2] {
    let (dx, dz) = (to[0] - from[0], to[2] - from[2]);
    let l = (dx * dx + dz * dz).sqrt();
    if l < 1e-4 {
        [0.0, 0.0]
    } else {
        [dx / l, dz / l]
    }
}

/// Authentic Terraria dawn behavior for nocturnal surface enemies:
/// when `is_day` is true, `DemonEye` and `Zombie` retreat away and despawn promptly.
/// Returns `true` when the enemy should despawn.
pub fn tick_dawn_retreat(e: &mut Enemy, target: [f32; 3], is_day: bool) -> bool {
    if !is_day {
        if matches!(e.kind, EnemyKind::DemonEye | EnemyKind::Zombie) {
            e.ai[2] = 0;
        }
        return false;
    }
    if e.kind == EnemyKind::DemonEye {
        let d = dir_xz(target, e.pos);
        e.vel[0] = (e.vel[0] + EYE_ACCEL * 2.0 * d[0]).clamp(-EYE_MAX, EYE_MAX);
        e.vel[2] = (e.vel[2] + EYE_ACCEL * 2.0 * d[1]).clamp(-EYE_MAX, EYE_MAX);
        e.vel[1] = (e.vel[1] + EYE_ACCEL * 3.0).min(EYE_MAX);
        for a in 0..3 {
            e.pos[a] += e.vel[a];
        }
        e.ai[2] += 1;
        return e.ai[2] >= 90 || e.pos[1] > target[1] + 20.0;
    }
    if e.kind == EnemyKind::Zombie {
        let d = dir_xz(target, e.pos);
        e.vel[0] = (e.vel[0] + ZOMBIE_ACCEL * d[0]).clamp(-ZOMBIE_MAX, ZOMBIE_MAX);
        e.vel[2] = (e.vel[2] + ZOMBIE_ACCEL * d[1]).clamp(-ZOMBIE_MAX, ZOMBIE_MAX);
        e.pos[0] += e.vel[0];
        e.pos[2] += e.vel[2];
        e.ai[2] += 1;
        let dist = (e.pos[0] - target[0]).hypot(e.pos[2] - target[2]);
        return e.ai[2] >= 90 || dist > 32.0;
    }
    false
}

/// One 60 Hz step toward `target` (the player's feet).
pub fn tick_enemy(q: &impl BlockQuery, e: &mut Enemy, target: [f32; 3]) {
    if matches!(
        e.kind,
        EnemyKind::EyeOfCthulhu
            | EnemyKind::WallOfFleshMouth
            | EnemyKind::WallOfFleshEye
            | EnemyKind::TheHungry
    ) {
        return; // stepped by dedicated boss AI (`boss::EyeBoss` / `wof::WofBoss`)
    }
    let half = e.kind.stats().half;
    let mut blocked_xz = false;
    if e.hurt_ticks > 0 {
        e.hurt_ticks -= 1;
        if !e.on_ground {
            e.vel[0] *= 0.96;
            e.vel[2] *= 0.96;
        }
    } else {
        match e.kind {
            EnemyKind::Slime | EnemyKind::LavaSlime => {
                if e.on_ground {
                    e.ai[0] += 1;
                    if e.ai[0] >= SLIME_HOP_TICKS {
                        let d = if e.kind == EnemyKind::Slime && e.ai[2] == 1 {
                            // Unprovoked daytime surface slime hops neutrally without chasing target
                            let dir = if e.ai[3] % 2 == 0 { 1.0 } else { -1.0 };
                            e.ai[3] = e.ai[3].wrapping_add(1);
                            [dir * 0.6, 0.0]
                        } else {
                            dir_xz(e.pos, target)
                        };
                        e.vel[1] = JUMP_SPEED;
                        e.vel[0] = 2.0 * PX * d[0];
                        e.vel[2] = 2.0 * PX * d[1];
                        e.ai[0] = 0;
                    } else {
                        e.vel[0] *= SLIME_FRICTION;
                        e.vel[2] *= SLIME_FRICTION;
                    }
                }
            }
            EnemyKind::Zombie => {
                let d = dir_xz(e.pos, target);
                e.vel[0] += ZOMBIE_ACCEL * d[0];
                e.vel[2] += ZOMBIE_ACCEL * d[1];
                let s = (e.vel[0] * e.vel[0] + e.vel[2] * e.vel[2]).sqrt();
                if s > ZOMBIE_MAX {
                    e.vel[0] *= ZOMBIE_MAX / s;
                    e.vel[2] *= ZOMBIE_MAX / s;
                }
            }
            EnemyKind::DemonEye | EnemyKind::EaterOfSouls | EnemyKind::ServantOfCthulhu => {
                let eater = e.kind == EnemyKind::EaterOfSouls;
                let servant = e.kind == EnemyKind::ServantOfCthulhu;
                let accel = if servant {
                    SERVANT_ACCEL
                } else if eater {
                    EATER_ACCEL
                } else {
                    EYE_ACCEL
                };
                let max = if servant {
                    SERVANT_MAX
                } else if eater {
                    EATER_MAX
                } else {
                    EYE_MAX
                };
                let c = [target[0], target[1] + 0.8, target[2]];
                let v = [c[0] - e.pos[0], c[1] - e.pos[1], c[2] - e.pos[2]];
                let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                if l > 1e-4 {
                    if e.kind == EnemyKind::DemonEye {
                        let dir = [v[0] / l, v[1] / l, v[2] / l];
                        let dot = e.vel[0] * dir[0] + e.vel[1] * dir[1] + e.vel[2] * dir[2];
                        // Damp perpendicular orbit and brake harder on overshoot so DemonEyes
                        // swoop into the player instead of flying away past them.
                        for a in 0..3 {
                            let par = dot * dir[a];
                            let perp = e.vel[a] - par;
                            e.vel[a] = par + perp * 0.92;
                        }
                        let steer = if dot < 0.0 { accel * 4.0 } else { accel };
                        for a in 0..3 {
                            e.vel[a] += steer * dir[a];
                        }
                    } else {
                        for a in 0..3 {
                            // 3D adaptation: double downward acceleration for a more
                            // committed dive, while keeping the classic total speed cap.
                            let dive = if eater && a == 1 && v[1] < 0.0 {
                                2.0
                            } else {
                                1.0
                            };
                            e.vel[a] += accel * dive * v[a] / l;
                        }
                    }
                }
                let s = (e.vel[0] * e.vel[0] + e.vel[1] * e.vel[1] + e.vel[2] * e.vel[2]).sqrt();
                if s > max {
                    for a in 0..3 {
                        e.vel[a] *= max / s;
                    }
                }
            }
            // Town NPCs are steered by `town_npc::tick_town`; only physics runs here.
            EnemyKind::EyeOfCthulhu
            | EnemyKind::Guide
            | EnemyKind::Merchant
            | EnemyKind::ArmsDealer
            | EnemyKind::Nurse
            | EnemyKind::Demolitionist
            | EnemyKind::Dryad
            | EnemyKind::GoblinTinkerer => {}
            EnemyKind::Skeleton => {
                let d = dir_xz(e.pos, target);
                e.vel[0] += ZOMBIE_ACCEL * d[0];
                e.vel[2] += ZOMBIE_ACCEL * d[1];
                let s = (e.vel[0] * e.vel[0] + e.vel[2] * e.vel[2]).sqrt();
                let max = 1.5 * PX;
                if s > max {
                    e.vel[0] *= max / s;
                    e.vel[2] *= max / s;
                }
            }
            EnemyKind::CaveBat | EnemyKind::Hellbat => {
                e.ai[0] = (e.ai[0] + 1) % 360;
                let hover_y = (e.ai[0] as f32 * 0.15).sin() * 0.8;
                let c = [target[0], target[1] + 0.8 + hover_y, target[2]];
                let v = [c[0] - e.pos[0], c[1] - e.pos[1], c[2] - e.pos[2]];
                let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                if l > 1e-4 {
                    let accel = 0.08 * PX;
                    for a in 0..3 {
                        e.vel[a] += accel * v[a] / l;
                    }
                }
                let s = (e.vel[0] * e.vel[0] + e.vel[1] * e.vel[1] + e.vel[2] * e.vel[2]).sqrt();
                let max = if e.kind == EnemyKind::Hellbat {
                    5.0 * PX
                } else {
                    4.0 * PX
                };
                if s > max {
                    for a in 0..3 {
                        e.vel[a] *= max / s;
                    }
                }
            }
            EnemyKind::GiantWorm => {
                let c = [target[0], target[1] + 0.5, target[2]];
                let v = [c[0] - e.pos[0], c[1] - e.pos[1], c[2] - e.pos[2]];
                let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                if l > 1e-4 {
                    let accel = 0.05 * PX;
                    for a in 0..3 {
                        e.vel[a] += accel * v[a] / l;
                    }
                }
                let s = (e.vel[0] * e.vel[0] + e.vel[1] * e.vel[1] + e.vel[2] * e.vel[2]).sqrt();
                let max = 6.0 * PX;
                if s > max {
                    for a in 0..3 {
                        e.vel[a] *= max / s;
                    }
                }
            }
            EnemyKind::EaterOfWorldsHead => {
                let c = [target[0], target[1] + 0.5, target[2]];
                let v = [c[0] - e.pos[0], c[1] - e.pos[1], c[2] - e.pos[2]];
                let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                if l > 1e-4 {
                    let accel = 0.008;
                    for a in 0..3 {
                        e.vel[a] += accel * v[a] / l;
                    }
                }
                let s = (e.vel[0] * e.vel[0] + e.vel[1] * e.vel[1] + e.vel[2] * e.vel[2]).sqrt();
                let max = 0.22;
                if s > max {
                    for a in 0..3 {
                        e.vel[a] *= max / s;
                    }
                }
            }
            EnemyKind::DungeonGuardian => {
                let c = [target[0], target[1] + 0.8, target[2]];
                let v = [c[0] - e.pos[0], c[1] - e.pos[1], c[2] - e.pos[2]];
                let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                let speed = 20.0 * PX;
                if l > 1e-4 {
                    for a in 0..3 {
                        e.vel[a] = speed * v[a] / l;
                    }
                } else {
                    e.vel = [0.0; 3];
                }
            }
            EnemyKind::SkeletronHead => {
                tick_skeletron_head(e, target);
            }
            EnemyKind::SkeletronHand => {
                tick_skeletron_hand(e, target, target);
            }
            EnemyKind::AngryBones => {
                let d = dir_xz(e.pos, target);
                e.vel[0] += ZOMBIE_ACCEL * d[0];
                e.vel[2] += ZOMBIE_ACCEL * d[1];
                let s = (e.vel[0] * e.vel[0] + e.vel[2] * e.vel[2]).sqrt();
                let max = 1.8 * PX;
                if s > max {
                    e.vel[0] *= max / s;
                    e.vel[2] *= max / s;
                }
            }
            EnemyKind::DarkCaster => {
                e.vel[0] = 0.0;
                e.vel[2] = 0.0;
                tick_ranged_windup(e);
                e.ai[0] += 1;
                if e.ai[0] % 120 == 60 - RANGED_WINDUP_TICKS {
                    start_ranged_windup(e);
                }
                if e.ai[0] >= 180 {
                    e.ai[0] = 0;
                    let tx = target[0].floor() as i32;
                    let ty = target[1].floor() as i32;
                    let tz = target[2].floor() as i32;
                    let mut teleported = false;
                    for r in [4, 6, 8, 3, 5, 7] {
                        if teleported {
                            break;
                        }
                        for (dx, dz) in [(-r, 0), (r, 0), (0, -r), (0, r), (-r, -r), (r, r)] {
                            let cx = tx + dx;
                            let cz = tz + dz;
                            for dy in -3..=3 {
                                let cy = ty + dy;
                                if q.block(cx, cy - 1, cz).is_solid()
                                    && !q.block(cx, cy, cz).is_solid()
                                    && !q.block(cx, cy + 1, cz).is_solid()
                                {
                                    e.pos = [cx as f32 + 0.5, cy as f32, cz as f32 + 0.5];
                                    teleported = true;
                                    break;
                                }
                            }
                            if teleported {
                                break;
                            }
                        }
                    }
                }
            }
            EnemyKind::CursedSkull => {
                let c = [target[0], target[1] + 0.8, target[2]];
                let v = [c[0] - e.pos[0], c[1] - e.pos[1], c[2] - e.pos[2]];
                let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                let speed = 1.2 * PX;
                if l > 1e-4 {
                    for a in 0..3 {
                        e.vel[a] = speed * v[a] / l;
                    }
                } else {
                    e.vel = [0.0; 3];
                }
            }
            EnemyKind::FireImp => {
                let dx = e.pos[0] - target[0];
                let dz = e.pos[2] - target[2];
                let horiz = (dx * dx + dz * dz).sqrt();
                let standoff_dir = if horiz > 1e-4 {
                    [dx / horiz, dz / horiz]
                } else {
                    [1.0, 0.0]
                };
                let desired = [
                    target[0] + standoff_dir[0] * 5.0,
                    target[1] + 6.0,
                    target[2] + standoff_dir[1] * 5.0,
                ];
                let v = [
                    desired[0] - e.pos[0],
                    desired[1] - e.pos[1],
                    desired[2] - e.pos[2],
                ];
                let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                if l > 1e-4 {
                    let speed = (0.05 * PX * l).min(3.0 * PX);
                    for a in 0..3 {
                        e.vel[a] = speed * v[a] / l;
                    }
                } else {
                    e.vel = [0.0; 3];
                }
                tick_ranged_windup(e);
                e.ai[0] += 1;
                if e.ai[0] == 90 - RANGED_WINDUP_TICKS {
                    start_ranged_windup(e);
                }
                if e.ai[0] == 115 || e.ai[0] == 140 {
                    e.ai[1] = 1;
                }
                if e.ai[0] >= 165 {
                    e.ai[0] = 0;
                    // Teleport to a clear spot near the player after the 3-shot burst
                    let phase = (e.id.0.wrapping_mul(2654435761) ^ (e.pos[0].to_bits())) % 8;
                    for step in 0..8u32 {
                        let ang = ((phase + step) % 8) as f32 * std::f32::consts::FRAC_PI_4;
                        let cand = [
                            target[0] + ang.cos() * 10.0,
                            target[1] + 4.0,
                            target[2] + ang.sin() * 10.0,
                        ];
                        if !overlaps(q, cand, half) {
                            e.pos = cand;
                            e.vel = [0.0; 3];
                            break;
                        }
                    }
                }
            }
            EnemyKind::Demon | EnemyKind::VoodooDemon => {
                let c = [target[0], target[1] + 0.8, target[2]];
                let v = [c[0] - e.pos[0], c[1] - e.pos[1], c[2] - e.pos[2]];
                let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                if l > 1e-4 {
                    let accel = 0.06 * PX;
                    for a in 0..3 {
                        e.vel[a] += accel * v[a] / l;
                    }
                }
                let s = (e.vel[0] * e.vel[0] + e.vel[1] * e.vel[1] + e.vel[2] * e.vel[2]).sqrt();
                let max = 5.0 * PX;
                if s > max {
                    for a in 0..3 {
                        e.vel[a] *= max / s;
                    }
                }
                tick_ranged_windup(e);
                e.ai[0] += 1;
                if e.ai[0] == 90 - RANGED_WINDUP_TICKS {
                    start_ranged_windup(e);
                }
                if e.ai[0] == 115 || e.ai[0] == 140 {
                    e.ai[1] = 1;
                }
                if e.ai[0] >= 165 {
                    e.ai[0] = 0;
                }
            }
            EnemyKind::BoneSerpentHead
            | EnemyKind::BoneSerpentBody
            | EnemyKind::BoneSerpentTail
            | EnemyKind::EaterOfWorldsBody
            | EnemyKind::EaterOfWorldsTail
            | EnemyKind::OldMan
            | EnemyKind::WallOfFleshMouth
            | EnemyKind::WallOfFleshEye
            | EnemyKind::TheHungry => {}
            EnemyKind::Leech => {
                let c = [target[0], target[1] + 0.8, target[2]];
                let v = [c[0] - e.pos[0], c[1] - e.pos[1], c[2] - e.pos[2]];
                let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                let speed = 3.2 * PX;
                if l > 1e-4 {
                    for a in 0..3 {
                        e.vel[a] = speed * v[a] / l;
                    }
                } else {
                    e.vel = [0.0; 3];
                }
            }
            EnemyKind::KingSlime => {
                if e.on_ground {
                    e.vel[0] *= SLIME_FRICTION;
                    e.vel[2] *= SLIME_FRICTION;
                    e.ai[0] += 1;
                    if e.ai[0] >= 40 {
                        e.ai[0] = 0;
                        e.ai[1] += 1;
                        let d = dir_xz(e.pos, target);
                        let big = e.ai[1] % 3 == 0;
                        let hspd = if big { 4.2 * PX } else { 2.8 * PX };
                        e.vel[0] = d[0] * hspd;
                        e.vel[2] = d[1] * hspd;
                        e.vel[1] = if big { 10.5 * PX } else { 7.5 * PX };
                        e.on_ground = false;
                        let dist_sq =
                            (e.pos[0] - target[0]).powi(2) + (e.pos[2] - target[2]).powi(2);
                        if dist_sq > 24.0 * 24.0 {
                            e.pos = [target[0], target[1] + 6.0, target[2]];
                        }
                    }
                }
            }
            EnemyKind::QueenBee => {
                e.ai[0] = (e.ai[0] + 1) % 240;
                if e.ai[0] < 130 {
                    // Hover above player and periodically signal minion/stinger attack
                    let goal = [target[0], target[1] + 8.0, target[2]];
                    let dx = goal[0] - e.pos[0];
                    let dy = goal[1] - e.pos[1];
                    let dz = goal[2] - e.pos[2];
                    let d = (dx * dx + dy * dy + dz * dz).sqrt().max(1e-4);
                    let spd = 5.5 * PX;
                    e.vel = [dx / d * spd, dy / d * spd, dz / d * spd];
                    if e.ai[0] == 40 || e.ai[0] == 80 || e.ai[0] == 120 {
                        e.ai[2] = 1;
                    }
                } else {
                    // Dash horizontally at player's Y
                    if e.ai[0] == 130 {
                        let d = dir_xz(e.pos, target);
                        let dash = 9.0 * PX;
                        e.vel = [
                            d[0] * dash,
                            (target[1] + 1.0 - e.pos[1]) * 0.08,
                            d[1] * dash,
                        ];
                    }
                }
            }
            EnemyKind::Bee => {
                let c = [target[0], target[1] + 1.0, target[2]];
                let v = [c[0] - e.pos[0], c[1] - e.pos[1], c[2] - e.pos[2]];
                let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-4);
                let speed = 4.2 * PX;
                e.vel = [speed * v[0] / l, speed * v[1] / l, speed * v[2] / l];
            }
            EnemyKind::IceSlime | EnemyKind::JungleSlime => {
                if e.on_ground {
                    e.vel[0] *= SLIME_FRICTION;
                    e.vel[2] *= SLIME_FRICTION;
                    e.ai[0] += 1;
                    if e.ai[0] >= SLIME_HOP_TICKS {
                        e.ai[0] = 0;
                        let d = dir_xz(e.pos, target);
                        e.vel[0] = d[0] * 1.8 * PX;
                        e.vel[2] = d[1] * 1.8 * PX;
                        e.vel[1] = 6.5 * PX;
                        e.on_ground = false;
                    }
                }
            }
            EnemyKind::BloodZombie
            | EnemyKind::GoblinPeon
            | EnemyKind::GoblinThief
            | EnemyKind::GoblinWarrior
            | EnemyKind::GoblinArcher
            | EnemyKind::SnowFlinx
            | EnemyKind::Crab => {
                let d = dir_xz(e.pos, target);
                let max = match e.kind {
                    EnemyKind::GoblinThief => 2.5 * PX,
                    EnemyKind::GoblinWarrior => 1.3 * PX,
                    _ => 1.7 * PX,
                };
                e.vel[0] += ZOMBIE_ACCEL * d[0];
                e.vel[2] += ZOMBIE_ACCEL * d[1];
                let s = (e.vel[0] * e.vel[0] + e.vel[2] * e.vel[2]).sqrt();
                if s > max {
                    e.vel[0] *= max / s;
                    e.vel[2] *= max / s;
                }
                if e.on_ground && (e.pos[1] + 0.5 < target[1] || e.ai[0] % 90 == 0) {
                    e.vel[1] = JUMP_SPEED;
                    e.on_ground = false;
                }
                e.ai[0] += 1;
            }
            EnemyKind::GoblinSorcerer | EnemyKind::Antlion => {
                e.vel[0] *= SLIME_FRICTION;
                e.vel[2] *= SLIME_FRICTION;
            }
            EnemyKind::Drippler
            | EnemyKind::Vulture
            | EnemyKind::Hornet
            | EnemyKind::ManEater
            | EnemyKind::Harpy
            | EnemyKind::Shark
            | EnemyKind::Piranha => {
                let c = [target[0], target[1] + 1.2, target[2]];
                let v = [c[0] - e.pos[0], c[1] - e.pos[1], c[2] - e.pos[2]];
                let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-4);
                let speed = match e.kind {
                    EnemyKind::Shark | EnemyKind::Harpy => 3.5 * PX,
                    EnemyKind::Hornet | EnemyKind::Piranha => 2.8 * PX,
                    _ => 2.2 * PX,
                };
                e.vel = [speed * v[0] / l, speed * v[1] / l, speed * v[2] / l];
            }
            EnemyKind::Bunny | EnemyKind::Bird | EnemyKind::Squirrel | EnemyKind::CaveCritter => {
                let dx = e.pos[0] - target[0];
                let dz = e.pos[2] - target[2];
                let horiz = dx.hypot(dz);
                let dist = (horiz * horiz + (e.pos[1] - target[1]).powi(2)).sqrt();
                if e.kind == EnemyKind::Bird && dist <= 3.5 {
                    let away = if horiz > 1e-4 {
                        [dx / horiz, dz / horiz]
                    } else {
                        [1.0, 0.0]
                    };
                    e.vel[0] = away[0] * 2.2 * PX;
                    e.vel[2] = away[1] * 2.2 * PX;
                    e.vel[1] = if e.pos[1] < target[1] + 6.0 {
                        1.0 * PX
                    } else {
                        0.0
                    };
                } else {
                    e.ai[0] = (e.ai[0] + 1) % 240;
                    if e.kind == EnemyKind::Bird {
                        let angle = e.ai[0] as f32 * std::f32::consts::TAU / 240.0;
                        if e.on_ground {
                            e.vel[0] = if e.ai[0] < 60 {
                                angle.cos() * 0.5 * PX
                            } else {
                                0.0
                            };
                            e.vel[2] = 0.0;
                            e.vel[1] = 0.0;
                        } else {
                            e.vel[0] = angle.cos() * 0.8 * PX;
                            e.vel[2] = angle.sin() * 0.8 * PX;
                            // Descend gently to land when safe instead of flying forever
                            e.vel[1] = -0.45 * PX;
                        }
                    } else if e.on_ground {
                        if e.ai[0] < 60 {
                            let dir = if e.ai[1] % 2 == 0 { 1.0 } else { -1.0 };
                            e.vel[0] = dir * 0.8 * PX;
                            e.vel[2] = 0.0;
                        } else {
                            e.vel[0] *= SLIME_FRICTION;
                            e.vel[2] *= SLIME_FRICTION;
                            if e.ai[0] == 239 {
                                e.ai[1] = e.ai[1].wrapping_add(1);
                            }
                        }
                    }
                }
            }
        }
    }

    // Flyers float even during knockback; they still collide with blocks below.
    if !matches!(
        e.kind,
        EnemyKind::DemonEye
            | EnemyKind::EaterOfSouls
            | EnemyKind::ServantOfCthulhu
            | EnemyKind::CaveBat
            | EnemyKind::Hellbat
            | EnemyKind::GiantWorm
            | EnemyKind::EaterOfWorldsHead
            | EnemyKind::EaterOfWorldsBody
            | EnemyKind::EaterOfWorldsTail
            | EnemyKind::DungeonGuardian
            | EnemyKind::SkeletronHead
            | EnemyKind::SkeletronHand
            | EnemyKind::CursedSkull
            | EnemyKind::FireImp
            | EnemyKind::Demon
            | EnemyKind::VoodooDemon
            | EnemyKind::BoneSerpentHead
            | EnemyKind::BoneSerpentBody
            | EnemyKind::BoneSerpentTail
            | EnemyKind::Bird
            | EnemyKind::Leech
            | EnemyKind::QueenBee
            | EnemyKind::Bee
            | EnemyKind::Drippler
            | EnemyKind::Vulture
            | EnemyKind::Hornet
            | EnemyKind::ManEater
            | EnemyKind::Harpy
            | EnemyKind::Shark
            | EnemyKind::Piranha
    ) {
        e.vel[1] = (e.vel[1] - GRAVITY).max(-MAX_FALL_SPEED);
    }

    // Servant of Cthulhu has noTileCollide = true (gameplay spec): flies directly through blocks.
    // Giant Worm burrows freely through solid blocks (gameplay spec).
    // Dungeon Guardian and Cursed Skull fly no-clip directly through blocks.
    if matches!(
        e.kind,
        EnemyKind::ServantOfCthulhu
            | EnemyKind::GiantWorm
            | EnemyKind::EaterOfWorldsHead
            | EnemyKind::EaterOfWorldsBody
            | EnemyKind::EaterOfWorldsTail
            | EnemyKind::DungeonGuardian
            | EnemyKind::SkeletronHead
            | EnemyKind::SkeletronHand
            | EnemyKind::CursedSkull
            | EnemyKind::BoneSerpentHead
            | EnemyKind::BoneSerpentBody
            | EnemyKind::BoneSerpentTail
            | EnemyKind::Leech
            | EnemyKind::QueenBee
            | EnemyKind::Bee
            | EnemyKind::Drippler
            | EnemyKind::Vulture
            | EnemyKind::Hornet
            | EnemyKind::ManEater
            | EnemyKind::Harpy
            | EnemyKind::Shark
            | EnemyKind::Piranha
    ) {
        for a in 0..3 {
            e.pos[a] += e.vel[a];
        }
        return;
    }

    let bounce = matches!(
        e.kind,
        EnemyKind::DemonEye | EnemyKind::CaveBat | EnemyKind::Hellbat
    );
    for axis in [0, 2] {
        let d = e.vel[axis];
        if move_axis(q, &mut e.pos, half, axis, d) {
            e.vel[axis] = if bounce { -d } else { 0.0 };
            blocked_xz = true;
        }
    }
    let dy = e.vel[1];
    let hit_y = move_axis(q, &mut e.pos, half, 1, dy);
    e.on_ground = hit_y && dy < 0.0;
    if hit_y {
        e.vel[1] = if bounce { -dy } else { 0.0 };
    }

    // Zombie, Skeleton, Angry Bones, and ground critters hop a blocked step (gameplay spec,56701).
    if matches!(
        e.kind,
        EnemyKind::Zombie
            | EnemyKind::Skeleton
            | EnemyKind::AngryBones
            | EnemyKind::Bunny
            | EnemyKind::Squirrel
            | EnemyKind::CaveCritter
    ) && blocked_xz
        && e.on_ground
        && e.hurt_ticks == 0
    {
        e.vel[1] = JUMP_SPEED;
    }
}

pub const EATER_SEGMENT_SPACING: f32 = 1.2;
pub const EATER_OF_WORLDS_DEFAULT_SEGMENTS: usize = 12;

pub const BONE_SERPENT_DEFAULT_SEGMENTS: usize = 6;
pub const BONE_SERPENT_SPACING: f32 = 1.2;

pub fn is_bone_serpent(kind: EnemyKind) -> bool {
    matches!(
        kind,
        EnemyKind::BoneSerpentHead | EnemyKind::BoneSerpentBody | EnemyKind::BoneSerpentTail
    )
}

pub fn is_eater_of_worlds(kind: EnemyKind) -> bool {
    matches!(
        kind,
        EnemyKind::EaterOfWorldsHead | EnemyKind::EaterOfWorldsBody | EnemyKind::EaterOfWorldsTail
    )
}

/// Kinds and tuning for a chained worm enemy (Eater of Worlds, Bone Serpent).
#[derive(Clone, Copy)]
struct WormChain {
    head: EnemyKind,
    body: EnemyKind,
    tail: EnemyKind,
    spacing: f32,
    max_speed: f32,
}

const EATER_CHAIN: WormChain = WormChain {
    head: EnemyKind::EaterOfWorldsHead,
    body: EnemyKind::EaterOfWorldsBody,
    tail: EnemyKind::EaterOfWorldsTail,
    spacing: EATER_SEGMENT_SPACING,
    max_speed: 0.22,
};

const BONE_SERPENT_CHAIN: WormChain = WormChain {
    head: EnemyKind::BoneSerpentHead,
    body: EnemyKind::BoneSerpentBody,
    tail: EnemyKind::BoneSerpentTail,
    spacing: BONE_SERPENT_SPACING,
    max_speed: 0.25,
};

/// Spawns a head/body/tail chain with child/parent ids in `ai[0]`/`ai[1]`.
fn spawn_worm_chain(start_id: u32, pos: [f32; 3], count: usize, chain: WormChain) -> Vec<Enemy> {
    if count == 0 {
        return Vec::new();
    }
    let mut segments = Vec::with_capacity(count);
    for i in 0..count {
        let id = EnemyId(start_id + i as u32);
        let kind = if i == 0 {
            chain.head
        } else if i == count - 1 {
            chain.tail
        } else {
            chain.body
        };
        let seg_pos = [pos[0], pos[1] - i as f32 * chain.spacing, pos[2]];
        let mut enemy = Enemy::new(id, kind, seg_pos);
        // ai[0] = child ID (behind, 0 if tail)
        // ai[1] = parent ID (in front, 0 if head)
        enemy.ai[0] = if i + 1 < count {
            (start_id + (i + 1) as u32) as i32
        } else {
            0
        };
        enemy.ai[1] = if i > 0 {
            (start_id + (i - 1) as u32) as i32
        } else {
            0
        };
        segments.push(enemy);
    }
    segments
}

fn tick_worm_chain(enemies: &mut [Enemy], target: [f32; 3], chain: WormChain) {
    // 1. Head segments steer towards target
    for i in 0..enemies.len() {
        if enemies[i].kind == chain.head && enemies[i].hp > 0 {
            let c = [target[0], target[1] + 0.5, target[2]];
            let v = [
                c[0] - enemies[i].pos[0],
                c[1] - enemies[i].pos[1],
                c[2] - enemies[i].pos[2],
            ];
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            if l > 1e-4 {
                let accel = 0.008;
                for a in 0..3 {
                    enemies[i].vel[a] += accel * v[a] / l;
                }
            }
            let s =
                (enemies[i].vel[0].powi(2) + enemies[i].vel[1].powi(2) + enemies[i].vel[2].powi(2))
                    .sqrt();
            if s > chain.max_speed {
                for a in 0..3 {
                    enemies[i].vel[a] *= chain.max_speed / s;
                }
            }
            for a in 0..3 {
                enemies[i].pos[a] += enemies[i].vel[a];
            }
        }
    }

    // 2. Propagate chain movement from each head down to tail
    let head_indices: Vec<usize> = enemies
        .iter()
        .enumerate()
        .filter(|(_, e)| e.kind == chain.head && e.hp > 0)
        .map(|(i, _)| i)
        .collect();

    for head_idx in head_indices {
        let mut leader_pos = enemies[head_idx].pos;
        let mut child_id = enemies[head_idx].ai[0];
        while child_id != 0 {
            if let Some(child_idx) = enemies
                .iter()
                .position(|e| e.id.0 as i32 == child_id && e.hp > 0)
            {
                let dx = leader_pos[0] - enemies[child_idx].pos[0];
                let dy = leader_pos[1] - enemies[child_idx].pos[1];
                let dz = leader_pos[2] - enemies[child_idx].pos[2];
                let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                if dist > 1e-4 {
                    let factor = (dist - chain.spacing) / dist;
                    enemies[child_idx].pos[0] += dx * factor;
                    enemies[child_idx].pos[1] += dy * factor;
                    enemies[child_idx].pos[2] += dz * factor;
                    enemies[child_idx].vel = [
                        dx / dist * chain.max_speed,
                        dy / dist * chain.max_speed,
                        dz / dist * chain.max_speed,
                    ];
                }
                leader_pos = enemies[child_idx].pos;
                child_id = enemies[child_idx].ai[0];
            } else {
                break;
            }
        }
    }

    // 3. Fallback for any follower segment whose parent moved but wasn't reached from a head
    for _ in 0..enemies.len() {
        let mut any_adjusted = false;
        for i in 0..enemies.len() {
            if (enemies[i].kind == chain.body || enemies[i].kind == chain.tail) && enemies[i].hp > 0
            {
                let parent_id = enemies[i].ai[1];
                if parent_id != 0 {
                    if let Some(parent) = enemies
                        .iter()
                        .find(|e| e.id.0 as i32 == parent_id && e.hp > 0)
                    {
                        let dx = parent.pos[0] - enemies[i].pos[0];
                        let dy = parent.pos[1] - enemies[i].pos[1];
                        let dz = parent.pos[2] - enemies[i].pos[2];
                        let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                        if dist > 1e-4 && (dist - chain.spacing).abs() > 0.001 {
                            let factor = (dist - chain.spacing) / dist;
                            enemies[i].pos[0] += dx * factor;
                            enemies[i].pos[1] += dy * factor;
                            enemies[i].pos[2] += dz * factor;
                            enemies[i].vel = [
                                dx / dist * chain.max_speed,
                                dy / dist * chain.max_speed,
                                dz / dist * chain.max_speed,
                            ];
                            any_adjusted = true;
                        }
                    }
                }
            }
        }
        if !any_adjusted {
            break;
        }
    }
}

pub fn spawn_bone_serpent(start_id: u32, pos: [f32; 3], count: usize) -> Vec<Enemy> {
    spawn_worm_chain(start_id, pos, count, BONE_SERPENT_CHAIN)
}

pub fn tick_bone_serpent(enemies: &mut [Enemy], target: [f32; 3]) {
    tick_worm_chain(enemies, target, BONE_SERPENT_CHAIN);
}

pub fn spawn_eater_of_worlds(start_id: u32, pos: [f32; 3], count: usize) -> Vec<Enemy> {
    spawn_worm_chain(start_id, pos, count, EATER_CHAIN)
}

pub fn tick_eater_of_worlds(enemies: &mut [Enemy], target: [f32; 3]) {
    tick_worm_chain(enemies, target, EATER_CHAIN);
}

pub fn handle_eater_of_worlds_splitting(enemies: &mut [Enemy]) {
    let dead_links: Vec<(EnemyId, i32, i32)> = enemies
        .iter()
        .filter(|e| e.hp <= 0 && is_eater_of_worlds(e.kind))
        .map(|e| (e.id, e.ai[0], e.ai[1]))
        .collect();

    for (_dead_id, child_id, parent_id) in dead_links {
        // Child segment behind the dead segment becomes a Head
        if child_id != 0 {
            if let Some(child) = enemies
                .iter_mut()
                .find(|e| e.id.0 as i32 == child_id && e.hp > 0)
            {
                child.ai[1] = 0;
                if child.kind == EnemyKind::EaterOfWorldsBody
                    || child.kind == EnemyKind::EaterOfWorldsTail
                {
                    child.kind = EnemyKind::EaterOfWorldsHead;
                }
            }
        }
        // Parent segment in front of the dead segment becomes a Tail
        if parent_id != 0 {
            if let Some(parent) = enemies
                .iter_mut()
                .find(|e| e.id.0 as i32 == parent_id && e.hp > 0)
            {
                parent.ai[0] = 0;
                if parent.kind == EnemyKind::EaterOfWorldsBody {
                    parent.kind = EnemyKind::EaterOfWorldsTail;
                }
            }
        }
    }
}

pub const SKELETRON_HOVER_TICKS: i32 = 600;
pub const SKELETRON_SPIN_TICKS: i32 = 360;
pub const SKELETRON_HAND_LUNGE_INTERVAL: i32 = 180;
pub const SKELETRON_HAND_WINDUP_TICKS: i32 = 30;
pub const SKELETRON_HAND_LUNGE_TICKS: i32 = 40;
pub const SKELETRON_HAND_RETRACT_TICKS: i32 = 60;

pub fn spawn_skeletron(start_id: u32, pos: [f32; 3]) -> Vec<Enemy> {
    let head_id = EnemyId(start_id);
    let left_hand_id = EnemyId(start_id + 1);
    let right_hand_id = EnemyId(start_id + 2);

    let mut head = Enemy::new(head_id, EnemyKind::SkeletronHead, pos);
    head.ai = [0, 0, 0, 0];

    let mut left_hand = Enemy::new(
        left_hand_id,
        EnemyKind::SkeletronHand,
        [pos[0] - 6.0, pos[1] - 2.0, pos[2]],
    );
    left_hand.ai = [0, -1, start_id as i32, 0];

    let mut right_hand = Enemy::new(
        right_hand_id,
        EnemyKind::SkeletronHand,
        [pos[0] + 6.0, pos[1] - 2.0, pos[2]],
    );
    right_hand.ai = [60, 1, start_id as i32, 0];

    vec![head, left_hand, right_hand]
}

pub fn tick_skeletron_head(head: &mut Enemy, target: [f32; 3]) {
    if head.hp <= 0 {
        return;
    }
    if head.ai[2] == 1 {
        // Daytime Dungeon Guardian enrage: extreme speed spin directly at target
        head.ai[1] = 1;
        let v = [
            target[0] - head.pos[0],
            target[1] + 0.8 - head.pos[1],
            target[2] - head.pos[2],
        ];
        let dist = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if dist > 1e-4 {
            let accel = 0.08;
            for a in 0..3 {
                head.vel[a] += accel * v[a] / dist;
            }
        }
        let speed = (head.vel[0].powi(2) + head.vel[1].powi(2) + head.vel[2].powi(2)).sqrt();
        let max_speed = 0.75;
        if speed > max_speed {
            for a in 0..3 {
                head.vel[a] *= max_speed / speed;
            }
        }
        return;
    }
    head.ai[0] += 1;
    if head.ai[1] == 0 {
        // Hover phase (~10s)
        if head.ai[0] >= SKELETRON_HOVER_TICKS {
            head.ai[1] = 1;
            head.ai[0] = 0;
            // Aim initial spin velocity towards player
            let v = [
                target[0] - head.pos[0],
                target[1] + 0.8 - head.pos[1],
                target[2] - head.pos[2],
            ];
            let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            if len > 1e-4 {
                let speed = 0.25;
                for a in 0..3 {
                    head.vel[a] = speed * v[a] / len;
                }
            }
        } else {
            let hover_target = [
                target[0] + (head.ai[0] as f32 * 0.05).sin() * 4.0,
                target[1] + 10.0,
                target[2] + (head.ai[0] as f32 * 0.03).cos() * 4.0,
            ];
            let v = [
                hover_target[0] - head.pos[0],
                hover_target[1] - head.pos[1],
                hover_target[2] - head.pos[2],
            ];
            let dist = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            if dist > 1e-4 {
                let accel = 0.02;
                for a in 0..3 {
                    head.vel[a] += accel * v[a] / dist;
                }
            }
            let speed = (head.vel[0].powi(2) + head.vel[1].powi(2) + head.vel[2].powi(2)).sqrt();
            let max_speed = 0.20;
            if speed > max_speed {
                for a in 0..3 {
                    head.vel[a] *= max_speed / speed;
                }
            }
        }
    } else {
        // Spin attack phase (~6s)
        if head.ai[0] >= SKELETRON_SPIN_TICKS {
            head.ai[1] = 0;
            head.ai[0] = 0;
        } else {
            let v = [
                target[0] - head.pos[0],
                target[1] + 0.8 - head.pos[1],
                target[2] - head.pos[2],
            ];
            let dist = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            if dist > 1e-4 {
                let accel = 0.025;
                for a in 0..3 {
                    head.vel[a] += accel * v[a] / dist;
                }
            }
            let speed = (head.vel[0].powi(2) + head.vel[1].powi(2) + head.vel[2].powi(2)).sqrt();
            let max_speed = 0.35;
            if speed > max_speed {
                for a in 0..3 {
                    head.vel[a] *= max_speed / speed;
                }
            }
        }
    }
}

pub fn tick_skeletron_hand(hand: &mut Enemy, head_pos: [f32; 3], target: [f32; 3]) {
    if hand.hp <= 0 {
        return;
    }
    let side = if hand.ai[1] < 0 { -1.0 } else { 1.0 };
    match hand.ai[3] {
        0 => {
            // Hover / Flank
            hand.ai[0] += 1;
            let flank_target = [
                head_pos[0] + side * 6.0,
                head_pos[1] - 2.0 + (hand.ai[0] as f32 * 0.08).sin() * 0.5,
                head_pos[2],
            ];
            let v = [
                flank_target[0] - hand.pos[0],
                flank_target[1] - hand.pos[1],
                flank_target[2] - hand.pos[2],
            ];
            let dist = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            if dist > 1e-4 {
                let accel = 0.03;
                for a in 0..3 {
                    hand.vel[a] += accel * v[a] / dist;
                }
            }
            let speed = (hand.vel[0].powi(2) + hand.vel[1].powi(2) + hand.vel[2].powi(2)).sqrt();
            let max_speed = 0.25;
            if speed > max_speed {
                for a in 0..3 {
                    hand.vel[a] *= max_speed / speed;
                }
            }
            if hand.ai[0] >= SKELETRON_HAND_LUNGE_INTERVAL {
                hand.ai[3] = 1; // Windup
                hand.ai[0] = 0;
            }
        }
        1 => {
            // Windup
            hand.ai[0] += 1;
            hand.vel[0] *= 0.9;
            hand.vel[1] = 0.05;
            hand.vel[2] *= 0.9;
            if hand.ai[0] >= SKELETRON_HAND_WINDUP_TICKS {
                hand.ai[3] = 2; // Lunge
                hand.ai[0] = 0;
                let v = [
                    target[0] - hand.pos[0],
                    target[1] - hand.pos[1],
                    target[2] - hand.pos[2],
                ];
                let dist = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                if dist > 1e-4 {
                    let speed = 0.35;
                    for a in 0..3 {
                        hand.vel[a] = speed * v[a] / dist;
                    }
                }
            }
        }
        2 => {
            // Lunge
            hand.ai[0] += 1;
            let v = [
                target[0] - hand.pos[0],
                target[1] - hand.pos[1],
                target[2] - hand.pos[2],
            ];
            let dist = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            if dist > 1e-4 {
                let accel = 0.05;
                for a in 0..3 {
                    hand.vel[a] += accel * v[a] / dist;
                }
            }
            let speed = (hand.vel[0].powi(2) + hand.vel[1].powi(2) + hand.vel[2].powi(2)).sqrt();
            let max_speed = 0.40;
            if speed > max_speed {
                for a in 0..3 {
                    hand.vel[a] *= max_speed / speed;
                }
            }
            if hand.ai[0] >= SKELETRON_HAND_LUNGE_TICKS {
                hand.ai[3] = 3; // Retract
                hand.ai[0] = 0;
            }
        }
        _ => {
            // Retract
            hand.ai[0] += 1;
            let flank_target = [head_pos[0] + side * 6.0, head_pos[1] - 2.0, head_pos[2]];
            let v = [
                flank_target[0] - hand.pos[0],
                flank_target[1] - hand.pos[1],
                flank_target[2] - hand.pos[2],
            ];
            let dist = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            if dist > 1e-4 {
                let accel = 0.04;
                for a in 0..3 {
                    hand.vel[a] += accel * v[a] / dist;
                }
            }
            let speed = (hand.vel[0].powi(2) + hand.vel[1].powi(2) + hand.vel[2].powi(2)).sqrt();
            let max_speed = 0.30;
            if speed > max_speed {
                for a in 0..3 {
                    hand.vel[a] *= max_speed / speed;
                }
            }
            if hand.ai[0] >= SKELETRON_HAND_RETRACT_TICKS {
                hand.ai[3] = 0; // Back to hover
                hand.ai[0] = 0;
            }
        }
    }
}

pub fn tick_skeletron(enemies: &mut [Enemy], target: [f32; 3]) {
    tick_skeletron_with_day(enemies, target, false);
}

pub fn tick_skeletron_with_day(enemies: &mut [Enemy], target: [f32; 3], is_day: bool) {
    let living_hands = enemies
        .iter()
        .filter(|e| e.kind == EnemyKind::SkeletronHand && e.hp > 0)
        .count() as i32;
    let head_pos = enemies
        .iter()
        .find(|e| e.kind == EnemyKind::SkeletronHead && e.hp > 0)
        .map(|e| e.pos)
        .unwrap_or(target);

    for e in enemies.iter_mut() {
        if e.hp <= 0 {
            continue;
        }
        if e.kind == EnemyKind::SkeletronHead {
            e.ai[2] = i32::from(is_day);
            e.ai[3] = living_hands;
            tick_skeletron_head(e, target);
            for a in 0..3 {
                e.pos[a] += e.vel[a];
            }
        } else if e.kind == EnemyKind::SkeletronHand {
            tick_skeletron_hand(e, head_pos, target);
            for a in 0..3 {
                e.pos[a] += e.vel[a];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{AIR, DIRT};

    struct Floor;
    impl BlockQuery for Floor {
        fn block(&self, _x: i32, y: i32, _z: i32) -> BlockId {
            if y < 0 { DIRT } else { AIR }
        }
    }

    /// Floor plus a 2-block-high wall at x = 3.
    struct Wall;
    impl BlockQuery for Wall {
        fn block(&self, x: i32, y: i32, _z: i32) -> BlockId {
            if y < 0 || (x == 3 && y < 2) {
                DIRT
            } else {
                AIR
            }
        }
    }

    fn mk(kind: EnemyKind, pos: [f32; 3]) -> Enemy {
        Enemy::new(EnemyId(1), kind, pos)
    }

    #[test]
    fn eater_id_matches_terraria() {
        let k = EnemyKind::EaterOfSouls;
        assert_eq!(k.npc_id(), 6);
        assert!(!k.is_friendly());
        assert_eq!(
            k.stats(),
            EnemyStats {
                hp: 40,
                damage: 22,
                defense: 8,
                value: 90,
                knockback_resist: 0.5,
                half: [15.0 * PX, 15.0 * PX],
            }
        );
        assert_eq!(mk(k, [0.0; 3]).hp, 40);
    }

    #[test]
    fn eater_flies_toward_player() {
        let mut up = mk(EnemyKind::EaterOfSouls, [0.0, 100.0, 0.0]);
        let mut down = up.clone();
        tick_enemy(&Floor, &mut up, [100.0, 199.2, 100.0]);
        tick_enemy(&Floor, &mut down, [100.0, -0.8, 100.0]);
        assert!(up.vel.iter().all(|&v| v > 0.0), "3D ascent: {:?}", up.vel);
        assert!(down.vel[0] > 0.0 && down.vel[2] > 0.0 && down.vel[1] < 0.0);
        assert!(
            down.vel[1].abs() > up.vel[1] * 1.5,
            "stronger downward dive"
        );

        let mut level = mk(EnemyKind::EaterOfSouls, [0.0, 100.0, 0.0]);
        tick_enemy(&Floor, &mut level, [1000.0, 99.2, 0.0]);
        assert!((level.vel[0] - 0.02 * PX).abs() < 1e-6);
        assert!(level.vel[1].abs() < 1e-6, "no gravity");
        for _ in 0..600 {
            tick_enemy(&Floor, &mut level, [1000.0, 99.2, 0.0]);
            tick_enemy(&Floor, &mut down, [1000.0, -0.8, 1000.0]);
        }
        let speed = |e: &Enemy| e.vel.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!((speed(&level) - 4.0 * PX).abs() < 1e-6);
        assert!(
            speed(&down) <= 4.0 * PX + 1e-6,
            "dive preserves classic cap"
        );
    }

    #[test]
    fn eater_keeps_tile_collision_and_floats_during_knockback() {
        let mut e = mk(EnemyKind::EaterOfSouls, [0.5, 0.0, 0.5]);
        for _ in 0..300 {
            tick_enemy(&Wall, &mut e, [100.0, -0.8, 0.5]);
        }
        assert!((e.pos[0] - (3.0 - 15.0 * PX)).abs() < 1e-5);
        e.pos = [0.5, 5.0, 0.5];
        e.vel = [0.0; 3];
        e.hurt_ticks = 2;
        tick_enemy(&Floor, &mut e, [100.0, 0.0, 100.0]);
        assert_eq!(e.pos, [0.5, 5.0, 0.5]);
    }

    #[test]
    fn demon_eye_stats_match_spec() {
        // gameplay spec (type == 2): 30x32, damage 18, defense 2, lifeMax 60, value 75.
        let s = EnemyKind::DemonEye.stats();
        assert_eq!((s.hp, s.damage, s.defense, s.value), (60, 18, 2, 75));
        assert_eq!(EnemyKind::DemonEye.npc_id(), 2);
    }

    #[test]
    fn enemy_stats_match_terraria() {
        let s = EnemyKind::Slime.stats();
        assert_eq!((s.hp, s.damage, s.defense, s.value), (25, 7, 2, 25));
        assert_eq!(s.half, [12.0 * PX, 9.0 * PX]);
        let e = EnemyKind::DemonEye.stats();
        assert_eq!((e.hp, e.damage, e.defense, e.value), (60, 18, 2, 75));
        assert_eq!(e.half, [15.0 * PX, 16.0 * PX]);
        let z = EnemyKind::Zombie.stats();
        assert_eq!((z.hp, z.damage, z.defense, z.value), (45, 14, 6, 60));
        assert_eq!(z.half, [9.0 * PX, 20.0 * PX]);
        assert_eq!(
            [1, 2, 3],
            [
                EnemyKind::Slime.npc_id(),
                EnemyKind::DemonEye.npc_id(),
                EnemyKind::Zombie.npc_id()
            ]
        );
    }

    #[test]
    fn slime_hops_toward_player_on_interval() {
        let mut e = mk(EnemyKind::Slime, [0.5, 0.0, 0.5]);
        let t = [20.5, 0.0, 0.5];
        // Settle on the floor, then no hop until the interval elapses.
        tick_enemy(&Floor, &mut e, t);
        assert!(e.on_ground || e.pos[1] == 0.0);
        for _ in 0..100 {
            tick_enemy(&Floor, &mut e, t);
        }
        assert_eq!(e.pos[0], 0.5, "no hop before the interval");
        let mut hopped = false;
        for _ in 0..40 {
            tick_enemy(&Floor, &mut e, t);
            hopped |= e.vel[1] > 0.0;
        }
        assert!(hopped, "slime must hop after 120 ground ticks");
        assert!(e.pos[0] > 0.5, "hop heads toward the target");
    }

    #[test]
    fn zombie_jumps_at_wall() {
        let mut e = mk(EnemyKind::Zombie, [0.5, 0.0, 0.5]);
        let mut jumped = false;
        for _ in 0..300 {
            tick_enemy(&Wall, &mut e, [10.5, 0.0, 0.5]);
            jumped |= e.vel[1] > 0.0;
        }
        assert!(jumped);
        assert!(e.pos[0] < 3.0, "wall stops the zombie");
    }

    #[test]
    fn demon_eye_accelerates_and_caps() {
        let mut e = mk(EnemyKind::DemonEye, [0.5, 5.0, 0.5]);
        tick_enemy(&Floor, &mut e, [100.5, 5.0, 0.5]);
        assert!(
            (e.vel[0] - EYE_ACCEL).abs() < EYE_ACCEL * 0.1,
            "{:?}",
            e.vel
        );
        assert!(e.vel[1].abs() < 0.01);
        for _ in 0..600 {
            tick_enemy(&Floor, &mut e, [100.5, 5.0, 0.5]);
        }
        let s = (e.vel[0].powi(2) + e.vel[1].powi(2) + e.vel[2].powi(2)).sqrt();
        assert!(s <= EYE_MAX + 1e-6 && s > EYE_MAX * 0.95, "speed {s}");
    }

    #[test]
    fn hurt_ticks_suppress_ai() {
        let mut e = mk(EnemyKind::Zombie, [0.5, 0.0, 0.5]);
        e.hurt_ticks = 5;
        e.vel[0] = -0.2;
        tick_enemy(&Floor, &mut e, [10.5, 0.0, 0.5]);
        assert!((e.vel[0] - (-0.2 * 0.96)).abs() < 1e-6);
        assert_eq!(e.hurt_ticks, 4);
    }

    #[test]
    fn enemy_ai_is_deterministic() {
        let run = |kind| {
            let mut e = mk(kind, [0.5, 1.0, 0.5]);
            for _ in 0..600 {
                tick_enemy(&Wall, &mut e, [8.5, 0.0, 3.5]);
            }
            e
        };
        for k in [EnemyKind::Slime, EnemyKind::DemonEye, EnemyKind::Zombie] {
            assert_eq!(run(k), run(k));
        }
    }

    #[test]
    fn cave_enemies_stats_match_spec() {
        let skel = EnemyKind::Skeleton;
        assert_eq!(skel.npc_id(), 21);
        assert!(!skel.is_friendly());
        let s_skel = skel.stats();
        assert_eq!(s_skel.hp, 60);
        assert_eq!(s_skel.damage, 20);
        assert_eq!(s_skel.defense, 8);
        assert_eq!(s_skel.knockback_resist, 0.5);

        let bat = EnemyKind::CaveBat;
        assert_eq!(bat.npc_id(), 49);
        assert!(!bat.is_friendly());
        let s_bat = bat.stats();
        assert_eq!(s_bat.hp, 16);
        assert_eq!(s_bat.damage, 13);
        assert_eq!(s_bat.defense, 2);
        assert_eq!(s_bat.knockback_resist, 0.8);

        let worm = EnemyKind::GiantWorm;
        assert_eq!(worm.npc_id(), 10);
        assert!(!worm.is_friendly());
        let s_worm = worm.stats();
        assert_eq!(s_worm.hp, 30);
        assert_eq!(s_worm.damage, 14);
        assert_eq!(s_worm.defense, 4);
        assert_eq!(s_worm.knockback_resist, 0.0);
    }

    #[test]
    fn giant_worm_burrows_through_blocks() {
        struct SolidAll;
        impl BlockQuery for SolidAll {
            fn block(&self, _x: i32, _y: i32, _z: i32) -> BlockId {
                DIRT
            }
        }

        let mut worm = mk(EnemyKind::GiantWorm, [0.0, 10.0, 0.0]);
        let target = [10.0, 10.0, 0.0];
        for _ in 0..60 {
            tick_enemy(&SolidAll, &mut worm, target);
        }
        assert!(
            worm.pos[0] > 1.0,
            "worm must burrow through solid blocks, pos={:?}",
            worm.pos
        );
        assert!(worm.vel[0] > 0.0, "worm vel heading toward target");
    }

    #[test]
    fn eater_of_worlds_segments_follow_head() {
        let spawn = [0.0, 20.0, 0.0];
        let mut segments = spawn_eater_of_worlds(100, spawn, 5);
        assert_eq!(segments.len(), 5);
        assert_eq!(segments[0].kind, EnemyKind::EaterOfWorldsHead);
        assert_eq!(segments[1].kind, EnemyKind::EaterOfWorldsBody);
        assert_eq!(segments[4].kind, EnemyKind::EaterOfWorldsTail);

        let target = [50.0, 20.0, 0.0];
        for _ in 0..60 {
            tick_eater_of_worlds(&mut segments, target);
        }

        // Head moved towards target
        assert!(
            segments[0].pos[0] > 5.0,
            "head must move towards target: {:?}",
            segments[0].pos
        );

        // Body and tail segments follow, keeping spacing ~1.2 blocks
        for i in 1..segments.len() {
            let dx = segments[i - 1].pos[0] - segments[i].pos[0];
            let dy = segments[i - 1].pos[1] - segments[i].pos[1];
            let dz = segments[i - 1].pos[2] - segments[i].pos[2];
            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
            assert!(
                (dist - EATER_SEGMENT_SPACING).abs() < 0.1,
                "segment {} spacing from {} was {}, expected ~{}",
                i,
                i - 1,
                dist,
                EATER_SEGMENT_SPACING
            );
        }
    }

    #[test]
    fn eater_of_worlds_splits_when_body_segment_dies() {
        let mut segments = spawn_eater_of_worlds(200, [0.0, 0.0, 0.0], 5);
        assert_eq!(segments.len(), 5);
        assert_eq!(segments[0].kind, EnemyKind::EaterOfWorldsHead);
        assert_eq!(segments[1].kind, EnemyKind::EaterOfWorldsBody);
        assert_eq!(segments[2].kind, EnemyKind::EaterOfWorldsBody);
        assert_eq!(segments[3].kind, EnemyKind::EaterOfWorldsBody);
        assert_eq!(segments[4].kind, EnemyKind::EaterOfWorldsTail);

        // Kill intermediate segment 2
        segments[2].hp = 0;
        handle_eater_of_worlds_splitting(&mut segments);

        // Segment 1 (in front of segment 2) becomes a Tail
        assert_eq!(segments[1].kind, EnemyKind::EaterOfWorldsTail);
        assert_eq!(segments[1].ai[0], 0, "front worm tail has no child");

        // Segment 3 (behind segment 2) becomes a Head
        assert_eq!(segments[3].kind, EnemyKind::EaterOfWorldsHead);
        assert_eq!(segments[3].ai[1], 0, "rear worm head has no parent");

        // Now we have two independent worms:
        // Worm 1: Head (0) -> Tail (1)
        // Worm 2: Head (3) -> Tail (4)
        assert_eq!(segments[0].kind, EnemyKind::EaterOfWorldsHead);
        assert_eq!(segments[4].kind, EnemyKind::EaterOfWorldsTail);
    }

    #[test]
    fn dungeon_guardian_and_old_man_stats_match_spec() {
        let old_man = EnemyKind::OldMan;
        assert_eq!(old_man.npc_id(), 37);
        assert!(old_man.is_friendly());
        let s_old = old_man.stats();
        assert_eq!(s_old.hp, 250);
        assert_eq!(s_old.damage, 0);
        assert_eq!(s_old.defense, 15);
        assert_eq!(s_old.knockback_resist, 0.0);

        let dg = EnemyKind::DungeonGuardian;
        assert_eq!(dg.npc_id(), 68);
        assert!(!dg.is_friendly());
        let s_dg = dg.stats();
        assert_eq!(s_dg.hp, 9999);
        assert_eq!(s_dg.damage, 9999);
        assert_eq!(s_dg.defense, 9999);
        assert_eq!(s_dg.knockback_resist, 0.0);
    }

    #[test]
    fn dungeon_guardian_flies_noclip_directly_at_player() {
        struct SolidAll;
        impl BlockQuery for SolidAll {
            fn block(&self, _x: i32, _y: i32, _z: i32) -> BlockId {
                DIRT
            }
        }

        let mut dg = mk(EnemyKind::DungeonGuardian, [0.0, 10.0, 0.0]);
        let target = [100.0, 10.0, 0.0];
        tick_enemy(&SolidAll, &mut dg, target);

        // Speed 20 px/tick (20.0 * PX)
        let speed = (dg.vel[0].powi(2) + dg.vel[1].powi(2) + dg.vel[2].powi(2)).sqrt();
        assert!((speed - 20.0 * PX).abs() < 1e-4, "speed={speed}");
        assert_eq!(
            dg.pos[0], dg.vel[0],
            "no-clip position must advance directly through blocks"
        );
        assert_eq!(dg.pos[1], 10.0 + dg.vel[1]);
    }

    #[test]
    fn skeletron_head_spin_attack() {
        let mut head = Enemy::new(EnemyId(1), EnemyKind::SkeletronHead, [0.0, 50.0, 0.0]);
        let player = [0.0, 40.0, 0.0];
        assert_eq!(head.defense(), 10);
        assert_eq!(head.damage(), 32);

        // Tick in hover phase
        tick_skeletron(std::slice::from_mut(&mut head), player);
        assert_eq!(head.ai[1], 0, "should be in hover phase");
        assert_eq!(head.defense(), 10);
        assert_eq!(head.damage(), 32);

        // Advance to trigger spin phase
        head.ai[0] = 599;
        tick_skeletron(std::slice::from_mut(&mut head), player);
        assert_eq!(head.ai[1], 1, "should transition to spin attack phase");
        assert_eq!(head.defense(), 0, "defense drops to 0 during spin");
        assert_eq!(head.damage(), 64, "damage doubles during spin");
        assert!(
            head.vel[1] < 0.0,
            "head accelerates downwards towards player during spin"
        );

        // Advance to end of spin phase
        head.ai[0] = 359;
        tick_skeletron(std::slice::from_mut(&mut head), player);
        assert_eq!(head.ai[1], 0, "should return to hover phase");
        assert_eq!(head.defense(), 10);
        assert_eq!(head.damage(), 32);
    }

    #[test]
    fn skeletron_hands_hover_and_lunge() {
        let mut enemies = spawn_skeletron(1, [0.0, 50.0, 0.0]);
        assert_eq!(enemies.len(), 3, "skeletron spawns 1 head + 2 hands");
        assert_eq!(enemies[0].kind, EnemyKind::SkeletronHead);
        assert_eq!(enemies[1].kind, EnemyKind::SkeletronHand);
        assert_eq!(enemies[2].kind, EnemyKind::SkeletronHand);

        let player = [0.0, 40.0, 0.0];
        // Tick in hover phase: hands stay near head flank positions (±6 blocks X)
        tick_skeletron(&mut enemies, player);
        let head_pos = enemies[0].pos;
        let left_hand = &enemies[1];
        let right_hand = &enemies[2];
        assert!(left_hand.pos[0] < head_pos[0], "left hand hovers on left");
        assert!(
            right_hand.pos[0] > head_pos[0],
            "right hand hovers on right"
        );

        // Advance left hand to lunge
        enemies[1].ai[3] = 1;
        enemies[1].ai[0] = SKELETRON_HAND_WINDUP_TICKS;
        tick_skeletron(&mut enemies, player);
        assert_eq!(enemies[1].ai[3], 2, "hand should enter lunge state");
        assert!(
            enemies[1].vel[1] < 0.0,
            "hand accelerates towards player during lunge"
        );
    }

    #[test]
    fn dungeon_enemies_stats_match_spec() {
        let ab = EnemyKind::AngryBones;
        assert_eq!(ab.npc_id(), 31);
        let s_ab = ab.stats();
        assert_eq!(s_ab.hp, 80);
        assert_eq!(s_ab.damage, 26);
        assert_eq!(s_ab.defense, 8);
        assert_eq!(s_ab.knockback_resist, 0.8);

        let dc = EnemyKind::DarkCaster;
        assert_eq!(dc.npc_id(), 32);
        let s_dc = dc.stats();
        assert_eq!(s_dc.hp, 50);
        assert_eq!(s_dc.damage, 20);
        assert_eq!(s_dc.defense, 2);
        assert_eq!(s_dc.knockback_resist, 0.6);

        let cs = EnemyKind::CursedSkull;
        assert_eq!(cs.npc_id(), 34);
        let s_cs = cs.stats();
        assert_eq!(s_cs.hp, 40);
        assert_eq!(s_cs.damage, 35);
        assert_eq!(s_cs.defense, 6);
        assert_eq!(s_cs.knockback_resist, 0.2);
    }

    #[test]
    fn cursed_skull_flies_noclip_directly_at_player() {
        struct SolidAll;
        impl BlockQuery for SolidAll {
            fn block(&self, _x: i32, _y: i32, _z: i32) -> BlockId {
                DIRT
            }
        }

        let mut skull = mk(EnemyKind::CursedSkull, [0.0, 10.0, 0.0]);
        let target = [100.0, 10.0, 0.0];
        tick_enemy(&SolidAll, &mut skull, target);

        let speed = (skull.vel[0].powi(2) + skull.vel[1].powi(2) + skull.vel[2].powi(2)).sqrt();
        assert!((speed - 1.2 * PX).abs() < 1e-4, "speed={speed}");
        assert_eq!(
            skull.pos[0], skull.vel[0],
            "no-clip position must advance directly through solid blocks"
        );
        assert_eq!(skull.pos[1], 10.0 + skull.vel[1]);
    }

    #[test]
    fn dark_caster_teleports_and_casts() {
        struct FlatFloor;
        impl BlockQuery for FlatFloor {
            fn block(&self, _x: i32, y: i32, _z: i32) -> BlockId {
                if y < 10 { DIRT } else { AIR }
            }
        }

        let mut caster = mk(EnemyKind::DarkCaster, [0.0, 10.0, 0.0]);
        let target = [20.0, 10.0, 0.0];

        // Tick 60: casts magic projectile after wind-up
        for _ in 0..60 {
            tick_enemy(&FlatFloor, &mut caster, target);
        }
        assert_eq!(
            caster.ai[1], 1,
            "dark caster triggers cast signal on tick 60"
        );

        // Tick 180: teleports near target
        caster.ai[0] = 179;
        tick_enemy(&FlatFloor, &mut caster, target);
        assert_eq!(caster.ai[0], 0, "timer resets after teleport");
        assert!(
            caster.pos[0] != 0.0,
            "caster should have teleported away from initial origin"
        );
    }

    #[test]
    fn underworld_enemy_stats_match_spec() {
        // FireImp: 70/30/16/350 kb0.5 half 9x20 px
        let fi = EnemyKind::FireImp;
        assert_eq!(fi.npc_id(), 24);
        assert_eq!(
            fi.stats(),
            EnemyStats {
                hp: 70,
                damage: 30,
                defense: 16,
                value: 350,
                knockback_resist: 0.5,
                half: [9.0 * PX, 20.0 * PX],
            }
        );

        // BoneSerpent: head 300/36/12, body 300/20/18, tail 300/16/18, value 1200, kb0 half 11x11 px
        let bsh = EnemyKind::BoneSerpentHead;
        let bsb = EnemyKind::BoneSerpentBody;
        let bst = EnemyKind::BoneSerpentTail;
        assert_eq!(bsh.npc_id(), 39);
        assert_eq!(bsb.npc_id(), 40);
        assert_eq!(bst.npc_id(), 41);

        assert_eq!(
            bsh.stats(),
            EnemyStats {
                hp: 300,
                damage: 36,
                defense: 12,
                value: 1200,
                knockback_resist: 0.0,
                half: [11.0 * PX, 11.0 * PX],
            }
        );
        assert_eq!(
            bsb.stats(),
            EnemyStats {
                hp: 300,
                damage: 20,
                defense: 18,
                value: 1200,
                knockback_resist: 0.0,
                half: [11.0 * PX, 11.0 * PX],
            }
        );
        assert_eq!(
            bst.stats(),
            EnemyStats {
                hp: 300,
                damage: 16,
                defense: 18,
                value: 1200,
                knockback_resist: 0.0,
                half: [11.0 * PX, 11.0 * PX],
            }
        );

        // LavaSlime: 50/15/10/120 half 12x9 px, kb 0.0 (Slime base)
        let ls = EnemyKind::LavaSlime;
        assert_eq!(ls.npc_id(), 59);
        assert_eq!(
            ls.stats(),
            EnemyStats {
                hp: 50,
                damage: 15,
                defense: 10,
                value: 120,
                knockback_resist: 0.0,
                half: [12.0 * PX, 9.0 * PX],
            }
        );

        // Hellbat: 46/35/8/120 kb0.8 half 11x9 px
        let hb = EnemyKind::Hellbat;
        assert_eq!(hb.npc_id(), 60);
        assert_eq!(
            hb.stats(),
            EnemyStats {
                hp: 46,
                damage: 35,
                defense: 8,
                value: 120,
                knockback_resist: 0.8,
                half: [11.0 * PX, 9.0 * PX],
            }
        );

        // Demon: 120/32/8/300 kb0.8 half 14x24 px
        let dm = EnemyKind::Demon;
        assert_eq!(dm.npc_id(), 62);
        assert_eq!(
            dm.stats(),
            EnemyStats {
                hp: 120,
                damage: 32,
                defense: 8,
                value: 300,
                knockback_resist: 0.8,
                half: [14.0 * PX, 24.0 * PX],
            }
        );

        // VoodooDemon: 140/32/8/1000 kb0.8 half 14x24 px
        let vd = EnemyKind::VoodooDemon;
        assert_eq!(vd.npc_id(), 66);
        assert_eq!(
            vd.stats(),
            EnemyStats {
                hp: 140,
                damage: 32,
                defense: 8,
                value: 1000,
                knockback_resist: 0.8,
                half: [14.0 * PX, 24.0 * PX],
            }
        );
    }

    #[test]
    fn bone_serpent_spawns_connected_chain() {
        let chain = spawn_bone_serpent(10, [0.0, 10.0, 0.0], BONE_SERPENT_DEFAULT_SEGMENTS);
        assert_eq!(chain.len(), 6);
        assert_eq!(chain[0].kind, EnemyKind::BoneSerpentHead);
        assert_eq!(chain[0].ai[1], 0, "head parent id must be 0");
        assert_eq!(chain[0].ai[0], 11, "head child id must be 11");

        for i in 1..5 {
            assert_eq!(chain[i].kind, EnemyKind::BoneSerpentBody);
            assert_eq!(chain[i].ai[1], 10 + i as i32 - 1);
            assert_eq!(chain[i].ai[0], 10 + i as i32 + 1);
        }

        assert_eq!(chain[5].kind, EnemyKind::BoneSerpentTail);
        assert_eq!(chain[5].ai[1], 14);
        assert_eq!(chain[5].ai[0], 0, "tail child id must be 0");

        for i in 1..6 {
            let dy = (chain[i - 1].pos[1] - chain[i].pos[1]).abs();
            assert!((dy - 1.2).abs() < 1e-4, "spacing must be 1.2");
        }
    }

    #[test]
    fn bone_serpent_follows_target_as_chain() {
        let spawn = [0.0, 20.0, 0.0];
        let mut segments = spawn_bone_serpent(100, spawn, BONE_SERPENT_DEFAULT_SEGMENTS);
        let target = [50.0, 20.0, 0.0];
        for _ in 0..60 {
            tick_bone_serpent(&mut segments, target);
        }

        assert!(
            segments[0].pos[0] > 5.0,
            "head must move towards target: {:?}",
            segments[0].pos
        );

        for i in 1..segments.len() {
            let dx = segments[i - 1].pos[0] - segments[i].pos[0];
            let dy = segments[i - 1].pos[1] - segments[i].pos[1];
            let dz = segments[i - 1].pos[2] - segments[i].pos[2];
            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
            assert!(
                (dist - 1.2).abs() < 0.1,
                "segment {} spacing from {} was {}, expected ~1.2",
                i,
                i - 1,
                dist
            );
        }
    }

    #[test]
    fn fire_imp_and_caster_wind_up_at_least_10_ticks_before_firing() {
        struct FlatFloor;
        impl BlockQuery for FlatFloor {
            fn block(&self, _x: i32, y: i32, _z: i32) -> BlockId {
                if y < 10 { DIRT } else { AIR }
            }
        }

        let target = [20.0, 10.0, 0.0];
        for kind in [
            EnemyKind::FireImp,
            EnemyKind::DarkCaster,
            EnemyKind::Demon,
            EnemyKind::VoodooDemon,
        ] {
            let mut e = mk(kind, [0.0, 10.0, 0.0]);
            let mut windup_start_tick = None;
            let mut fire_tick = None;
            for tick in 1..=250 {
                tick_enemy(&FlatFloor, &mut e, target);
                if e.ai[3] == 1 {
                    assert!(
                        windup_start_tick.is_none(),
                        "{kind:?} emitted telegraph twice before firing"
                    );
                    assert_eq!(e.ai[1], 0, "{kind:?} must not fire on telegraph start tick");
                    windup_start_tick = Some(tick);
                    e.ai[3] = 0;
                }
                if e.ai[1] == 1 {
                    fire_tick = Some(tick);
                    break;
                }
            }
            let start = windup_start_tick.expect(&format!("{kind:?} must start wind-up"));
            let fire = fire_tick.expect(&format!("{kind:?} must fire after wind-up"));
            assert!(
                fire - start >= 10,
                "{kind:?} wind-up was {} ticks, expected >= 10",
                fire - start
            );
        }
    }

    #[test]
    fn critters_flee_nearby_player_and_have_no_combat_or_drops() {
        struct FlatFloor;
        impl BlockQuery for FlatFloor {
            fn block(&self, _x: i32, y: i32, _z: i32) -> BlockId {
                if y < 10 { DIRT } else { AIR }
            }
        }

        let player = [0.0, 10.0, 0.0];
        for kind in [
            EnemyKind::Bunny,
            EnemyKind::Bird,
            EnemyKind::Squirrel,
            EnemyKind::CaveCritter,
        ] {
            assert!(kind.is_critter(), "{kind:?} must be a critter");
            assert!(!kind.is_friendly(), "{kind:?} must not be a town NPC");
            let st = kind.stats();
            assert_eq!(st.damage, 0, "{kind:?} must deal no contact damage");
            assert_eq!(st.value, 0, "{kind:?} must drop no coins");

            let start_pos = if kind == EnemyKind::Bird {
                [2.5, 11.0, 0.0]
            } else {
                [3.0, 10.0, 0.0]
            };
            let mut e = mk(kind, start_pos);
            let start_dist = (e.pos[0] - player[0]).hypot(e.pos[2] - player[2]);
            for _ in 0..30 {
                tick_enemy(&FlatFloor, &mut e, player);
            }
            let end_dist = (e.pos[0] - player[0]).hypot(e.pos[2] - player[2]);
            if kind == EnemyKind::Bird {
                assert!(
                    end_dist > start_dist + 0.3,
                    "{kind:?} must flush away when very close (start={start_dist}, end={end_dist})"
                );
            } else {
                assert!(
                    (end_dist - start_dist).abs() < 2.0,
                    "{kind:?} must remain calm and not bolt away (start={start_dist}, end={end_dist})"
                );
            }
        }
        assert!(!EnemyKind::Slime.is_critter());
        assert!(!EnemyKind::Guide.is_critter());
    }

    #[test]
    fn wof_enemy_kinds_and_stats_match_terraria() {
        assert_eq!(EnemyKind::WallOfFleshMouth.npc_id(), 113);
        assert_eq!(EnemyKind::WallOfFleshEye.npc_id(), 114);
        assert_eq!(EnemyKind::TheHungry.npc_id(), 115);
        assert_eq!(EnemyKind::Leech.npc_id(), 117);

        assert!(EnemyKind::WallOfFleshMouth.is_boss());
        assert!(EnemyKind::WallOfFleshEye.is_boss());
        assert!(!EnemyKind::TheHungry.is_boss());
        assert!(!EnemyKind::Leech.is_boss());

        let mouth = EnemyKind::WallOfFleshMouth.stats();
        assert_eq!(
            (mouth.hp, mouth.damage, mouth.defense, mouth.value),
            (8000, 50, 12, 80000)
        );
        let eye = EnemyKind::WallOfFleshEye.stats();
        assert_eq!((eye.hp, eye.damage, eye.defense), (8000, 50, 0));
        let hungry = EnemyKind::TheHungry.stats();
        assert_eq!((hungry.hp, hungry.damage, hungry.defense), (250, 30, 10));
        let leech = EnemyKind::Leech.stats();
        assert_eq!((leech.hp, leech.damage, leech.defense), (60, 26, 2));
    }

    #[test]
    fn demon_eye_converges_without_overshoot_runaway() {
        let mut e = mk(EnemyKind::DemonEye, [0.5, 10.0, 0.5]);
        let target = [10.5, 10.0, 0.5];
        let mut max_x_after_crossing = f32::NEG_INFINITY;
        let mut crossed = false;
        for _ in 0..300 {
            tick_enemy(&Floor, &mut e, target);
            if e.pos[0] >= target[0] {
                crossed = true;
            }
            if crossed {
                max_x_after_crossing = max_x_after_crossing.max(e.pos[0]);
            }
        }
        assert!(crossed, "demon eye must reach the target");
        assert!(
            max_x_after_crossing - target[0] < 3.0,
            "demon eye overshot too far past target: max_x={max_x_after_crossing}, target={}",
            target[0]
        );
    }

    #[test]
    fn slime_converges_toward_player_and_clears_residual_vel() {
        let mut e = mk(EnemyKind::Slime, [20.5, 0.0, 0.5]);
        e.vel = [0.2, 0.0, 0.0];
        let target = [0.5, 0.0, 0.5];
        for _ in 0..260 {
            tick_enemy(&Floor, &mut e, target);
        }
        assert!(
            e.pos[0] < 19.0,
            "slime must converge toward target, got pos={:?}",
            e.pos
        );
    }

    #[test]
    fn zombie_retreats_and_despawns_at_dawn() {
        let mut z = mk(EnemyKind::Zombie, [10.0, 10.0, 0.0]);
        let target = [0.0, 10.0, 0.0];
        assert!(!tick_dawn_retreat(&mut z, target, false));
        let mut despawned = false;
        for _ in 0..100 {
            if tick_dawn_retreat(&mut z, target, true) {
                despawned = true;
                break;
            }
        }
        assert!(despawned, "Zombie must retreat and despawn at dawn");
        assert!(z.pos[0] > 10.0, "Zombie must retreat away from target");
    }

    #[test]
    fn skeletron_head_has_hand_defense_and_daytime_enrage() {
        let mut enemies = spawn_skeletron(1, [0.0, 50.0, 0.0]);
        let player = [0.0, 40.0, 0.0];
        tick_skeletron_with_day(&mut enemies, player, false);
        assert_eq!(enemies[0].defense(), 10 + 50, "2 living hands = +50 def");
        enemies[1].hp = 0;
        tick_skeletron_with_day(&mut enemies, player, false);
        assert_eq!(enemies[0].defense(), 10 + 25, "1 living hand = +25 def");
        tick_skeletron_with_day(&mut enemies, player, true);
        assert_eq!(enemies[0].defense(), 9999, "daytime enraged def");
        assert!(enemies[0].damage() >= 1000, "daytime enraged dmg");
    }

    #[test]
    fn neutral_daytime_slime_does_not_chase_target() {
        let mut s = mk(EnemyKind::Slime, [20.5, 0.0, 0.5]);
        s.ai[2] = 1; // unprovoked daytime surface flag
        let target = [0.5, 0.0, 0.5];
        for _ in 0..200 {
            tick_enemy(&Floor, &mut s, target);
        }
        assert!(
            s.pos[0] > 18.0,
            "neutral slime must not chase target, pos={:?}",
            s.pos
        );
    }

    #[test]
    fn flyers_bounce_on_block_and_hurt_applies_air_drag() {
        struct WallAt5;
        impl BlockQuery for WallAt5 {
            fn block(&self, x: i32, _y: i32, _z: i32) -> BlockId {
                if x >= 5 { DIRT } else { AIR }
            }
        }
        let mut bat = mk(EnemyKind::CaveBat, [4.2, 10.0, 0.5]);
        bat.hurt_ticks = 4;
        bat.vel = [0.9, 0.0, 0.0];
        tick_enemy(&WallAt5, &mut bat, [10.0, 10.0, 0.5]);
        assert!(
            bat.vel[0] < 0.0,
            "bat must bounce off wall, vel={:?}",
            bat.vel
        );
        // Hurt air drag slows airborne knockback
        let mut eye = mk(EnemyKind::DemonEye, [0.5, 10.0, 0.5]);
        eye.hurt_ticks = 3;
        eye.vel = [1.0, 0.0, 0.0];
        tick_enemy(&WallAt5, &mut eye, [10.0, 10.0, 0.5]);
        assert!(eye.vel[0] < 1.0, "hurt air drag must slow airborne vel");
    }

    #[test]
    fn fire_imp_fires_three_burst_and_teleports() {
        let mut imp = mk(EnemyKind::FireImp, [10.0, 10.0, 10.0]);
        let start = imp.pos;
        let target = [0.0, 10.0, 0.0];
        let mut shots = 0;
        for _ in 0..170 {
            tick_enemy(&Floor, &mut imp, target);
            if imp.ai[1] == 1 {
                shots += 1;
                imp.ai[1] = 0;
            }
        }
        assert_eq!(shots, 3, "FireImp must fire 3 Burning Spheres per cycle");
        assert_ne!(imp.pos, start, "FireImp must teleport after burst");
    }

    #[test]
    fn demon_casts_without_teleporting() {
        let mut demon = mk(EnemyKind::Demon, [10.0, 10.0, 10.0]);
        let target = [0.0, 10.0, 0.0];
        let mut shots = 0;
        let mut max_step = 0.0f32;
        for _ in 0..170 {
            let prev = demon.pos;
            tick_enemy(&Floor, &mut demon, target);
            let step = ((demon.pos[0] - prev[0]).powi(2)
                + (demon.pos[1] - prev[1]).powi(2)
                + (demon.pos[2] - prev[2]).powi(2))
            .sqrt();
            max_step = max_step.max(step);
            if demon.ai[1] == 1 {
                shots += 1;
                demon.ai[1] = 0;
            }
        }
        assert_eq!(shots, 3, "Demon must cast 3 Demon Scythes per cycle");
        assert!(
            max_step < 1.0,
            "Demon must fly smoothly without teleporting (max_step={max_step})"
        );
    }

    #[test]
    fn fleeing_bird_descends_when_safe() {
        struct Empty;
        impl BlockQuery for Empty {
            fn block(&self, _x: i32, _y: i32, _z: i32) -> BlockId {
                AIR
            }
        }
        let mut bird = mk(EnemyKind::Bird, [30.0, 20.0, 0.0]);
        let start_y = bird.pos[1];
        for _ in 0..30 {
            tick_enemy(&Empty, &mut bird, [0.0, 10.0, 0.0]);
        }
        assert!(
            bird.pos[1] < start_y,
            "safe bird must descend toward ground instead of flying to infinity"
        );
    }
}
