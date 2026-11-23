//! Client/server messages and the in-process transport.
//!
//! The single-player build runs the server and client in one process over
//! `std::sync::mpsc` channels; a networked transport can implement the same
//! traits later without touching the message types.

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};

use crate::block::BlockId;
use crate::chunk::{Chunk, ChunkPos};
use crate::crafting::{CraftError, RecipeId};
use crate::drop::DropId;
use crate::enemy::{EnemyId, EnemyKind};
use crate::facts::{CharacterFacts, WorldFacts};
use crate::interact::UseEvent;
use crate::inventory::{Inventory, ItemStack, SortMode};
use crate::item::{ArmorSlot, ItemId, Prefix};
use crate::player::{DeathCause, PlayerBody, PlayerEvent, PlayerInput, Vitals};
use crate::projectile::Projectile;
use crate::slots::{ClickKind, SlotRef};
use crate::star::Star;
use crate::town_npc::TownKind;
use crate::weather::WeatherKind;
use crate::world_bounds::WorldSize;

/// Chunks are streamed within this horizontal distance of a spectator or player.
pub const VIEW_RADIUS_BLOCKS: f32 = 140.0;
/// Chunks beyond `VIEW_RADIUS_BLOCKS + DESPAWN_MARGIN` are unloaded.
pub const DESPAWN_MARGIN: f32 = 32.0;
/// Vertical chunk radius streamed around the player/camera (`cy - 3 ..= cy + 3`, up to 7 layers).
pub const VERTICAL_CHUNK_RADIUS: i32 = 3;

/// Active vertical chunk layer range `0..WORLD_CHUNK_LAYERS` centered around world `y`.
pub fn vertical_layers_for_y(y: f32) -> std::ops::Range<i32> {
    let cy = (y / crate::chunk::CHUNK_SIZE as f32).floor() as i32;
    let lo = (cy - VERTICAL_CHUNK_RADIUS).max(0);
    let hi = (cy + VERTICAL_CHUNK_RADIUS + 1).min(crate::WORLD_CHUNK_LAYERS);
    lo..hi
}

pub type PlayerId = u32;

/// Classification of damage source for combat feedback (audio/particles/numbers).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DamageKind {
    Melee,
    Ranged,
    Magic,
    Contact,
    Projectile,
    Environment,
    Unknown,
}

/// Classification of the entity that was hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitTargetKind {
    Enemy,
    Boss,
    Player,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreativeCmd {
    FullHeal,
    ToggleGodMode,
    SetTime(u32),
}

#[derive(Clone, Debug)]
pub enum ClientMsg {
    Hello {
        seed: u32,
        /// Save slot for this player; `None` = "player".
        name: Option<String>,
        /// Encoded Character bytes (M4).
        character: Option<Vec<u8>>,
    },
    Input {
        seq: u32,
        input: PlayerInput,
    },
    Spectate {
        pos: [f32; 3],
    },
    /// Craft `count` executions of a recipe; the server validates everything.
    Craft {
        recipe: RecipeId,
        count: u16,
    },
    /// Equip the armor item in inventory slot `from`; the server validates.
    Equip {
        from: u8,
    },
    Unequip {
        slot: ArmorSlot,
    },
    /// Open the chest block at `pos`; the server validates reach and ownership.
    OpenChest {
        pos: [i32; 3],
    },
    CloseChest,
    /// One Terraria-style click on a slot; the server owns the held stack.
    ClickSlot {
        at: SlotRef,
        kind: ClickKind,
    },
    /// The inventory screen closed: the held stack goes back to the bag.
    CloseInventory,
    /// Right click on a door at `pos`: the server toggles it (reach and overlap
    /// are validated; refusals are silent).
    UseTile {
        pos: [i32; 3],
    },
    /// Move one stack: inventory slot -> chest (`to_chest`) or chest slot -> inventory.
    ChestMove {
        to_chest: bool,
        slot: u8,
    },
    /// Break the background wall at `pos` (reach-checked; returns the item as a drop).
    /// Walls are placed through the normal use path, so there is no place message.
    BreakWall {
        pos: [i32; 3],
    },
    /// Ask whether the room containing `pos` is valid housing (reach-checked;
    /// at most one query per connection per tick is answered).
    QueryHouse {
        pos: [i32; 3],
    },
    /// Assign a town NPC to the valid house containing `pos`.
    AssignHousing {
        npc: TownKind,
        pos: [i32; 3],
    },
    /// Right click on a town NPC (reach-checked): answers with `Dialogue`, and
    /// `ShopContents` for a Merchant.
    TalkNpc {
        id: EnemyId,
    },
    /// Buy `count` units of stock entry `index` from the Merchant `npc` (reach, funds and space checked).
    ShopBuy {
        npc: EnemyId,
        index: u8,
        count: u16,
    },
    /// Sell the stack in inventory `slot` to the Merchant last talked to.
    ShopSell {
        slot: u8,
    },
    /// Stack matching non-hotbar items into the open chest.
    ChestQuickStack,
    /// Sort bag slots `0..BAG` by `mode`; coins, ammo, and held stack are untouched.
    SortInventory {
        mode: SortMode,
    },
    /// Request respawn immediately while dead.
    Respawn,
    /// QA-only: ask the server to kill this player (honoured only when the server
    /// was started with `qa_kill`).
    QaKill,
    /// Request current Guide advice.
    GuideAdvice,
    /// Drop the cursor stack (`from_cursor: true`) or selected hotbar stack into the world.
    DropItem {
        dir: [f32; 3],
        from_cursor: bool,
    },
    /// Quick-heal (`H`): consume best healing potion/mushroom in bag.
    QuickHeal,
    /// Quick-mana (`J`): consume best mana potion in bag.
    QuickMana,
    /// Quick-buff (`B`): consume distinct unapplied buff potions in bag.
    QuickBuff,
    /// Creative mode: give `count` of `item` into inventory.
    CreativeGive {
        item: ItemId,
        count: u32,
    },
    /// Creative mode: execute a creative command.
    CreativeAction(CreativeCmd),
    /// Player dismissed the first-spawn onboarding card.
    DismissOnboarding,
    /// Player closed the Guide crafting lookup tab (return placed material).
    CloseGuideCrafting,
    /// Shift-click a bag slot while Guide crafting is open to place/swap a material.
    GuideSlotShift {
        slot: u8,
    },
    /// Request full heal + debuff clear from the Nurse (reach & funds checked).
    HealAtNurse {
        npc: EnemyId,
    },
    /// Reforge an eligible item in bag `slot` at the Goblin Tinkerer (reach & funds checked).
    ReforgeItem {
        npc: EnemyId,
        slot: u8,
    },
    Bye,
}

#[derive(Clone, Debug)]
pub enum ServerMsg {
    Welcome {
        id: PlayerId,
        spawn: [f32; 3],
        size: WorldSize,
    },
    Chunk {
        pos: ChunkPos,
        chunk: Arc<Chunk>,
    },
    Unload {
        pos: ChunkPos,
    },
    PlayerState {
        ack: u32,
        body: PlayerBody,
        vitals: Vitals,
        dead: bool,
        respawn_ticks: i32,
    },
    Event(PlayerEvent),
    BlockChanged {
        pos: [i32; 3],
        block: BlockId,
    },
    /// A wall bit changed in a chunk this client holds.
    WallChanged {
        pos: [i32; 3],
        on: bool,
    },
    /// Liquid cells changed in a chunk this client holds: `(local_index, packed_liquid)`.
    LiquidDelta {
        pos: ChunkPos,
        cells: Vec<(u16, u16)>,
    },
    /// Answer to `QueryHouse`: `reason` is 0 (ok), 1..=7 `RoomFail::code`, 8 = furniture missing.
    HouseStatus {
        ok: bool,
        reason: u8,
    },
    Inventory(Inventory),
    /// Current character state snapshot (encoded `Character` bytes) sent on autosave and disconnect.
    CharacterState(Vec<u8>),
    /// The stack on the cursor (server-owned, never persisted).
    Held(Option<ItemStack>),
    /// Worn armor (head, body, legs); sent on join and on change.
    Armor {
        pieces: [Option<ItemId>; 3],
    },
    /// Worn accessories (5 slots) and their prefixes; sent on join and on change.
    Accessories {
        pieces: [Option<ItemId>; 5],
        prefixes: [Prefix; 5],
    },
    /// Vanity / social armor (3 slots) and accessories (5 slots + prefixes); sent on join and on change.
    Vanity {
        armor: [Option<ItemId>; 3],
        accessories: [Option<ItemId>; 5],
        accessory_prefixes: [Prefix; 5],
    },
    Use(UseEvent),
    /// World clock ticks (see `time::WorldClock`).
    Time {
        tick: u32,
    },
    /// Authoritative weather state broadcast on join, transition, and 1 Hz.
    WeatherState {
        kind: WeatherKind,
        intensity: f32,
    },
    /// An enemy entered this player's radius.
    EnemySpawn {
        id: EnemyId,
        kind: EnemyKind,
        pos: [f32; 3],
    },
    /// 10 Hz snapshot of an enemy the player knows.
    EnemyState {
        id: EnemyId,
        pos: [f32; 3],
        vel: [f32; 3],
        hp: i32,
        on_fire: bool,
    },
    EnemyDespawn {
        id: EnemyId,
    },
    Dialogue {
        npc: EnemyId,
        text: String,
    },
    /// Overhead / ambient speech bubble for a town NPC.
    NpcSpeech {
        id: EnemyId,
        text: String,
    },
    /// Merchant stock: `(item, price in copper)`.
    ShopContents {
        items: Vec<(ItemId, u32)>,
    },
    /// Outcome of a `ShopBuy` / `ShopSell`.
    ShopResult {
        ok: bool,
        cannot_afford: bool,
    },
    /// Sent once after `EnemySpawn` of a town NPC: whether it has a house yet.
    TownNpcInfo {
        id: EnemyId,
        homeless: bool,
    },
    /// An enemy contact hurt this player (replaces `Event(Hurt)` for contact).
    PlayerHit {
        amount: i32,
    },
    PlayerDied {
        cause: DeathCause,
        coins_lost: u32,
        survived_ticks: u32,
    },
    /// A world item entered this player's radius.
    DropSpawn {
        id: DropId,
        stack: ItemStack,
        pos: [f32; 3],
    },
    DropDespawn {
        id: DropId,
    },
    /// This player collected the drop (the inventory message follows).
    DropPickup {
        id: DropId,
        item: ItemId,
        count: u32,
    },
    /// Answer to one `Craft`; `crafted` is output item units.
    /// Full contents of the chest this player has open (sent on open and on change).
    ChestContents {
        pos: [i32; 3],
        slots: Vec<Option<ItemStack>>,
    },
    /// The open chest is closed (refused, out of reach, broken, or `CloseChest`).
    ChestClosed,
    /// Every live arrow near this player this tick. Sent only while any exist, plus
    /// one empty list when the last one disappears so the client can clear them.
    Projectiles {
        list: Vec<Projectile>,
    },
    /// Falling stars: active star entities falling from the sky. Sent every tick while
    /// any exist, plus one empty list when the last one clears.
    Stars {
        list: Vec<Star>,
    },
    /// The Eye of Cthulhu, every tick while it lives (position is the feet centre).
    Boss {
        pos: [f32; 3],
        vel: [f32; 3],
        life: i32,
        life_max: i32,
        /// 1 = first form, 2 = transition and second form.
        phase: u8,
        /// 0 = hovering, 1 = dashing, 2 = transform stage 1, 3 = fleeing, 4 = transform stage 2.
        state: u8,
        /// Spin angle during transition, radians.
        spin: f32,
    },
    /// The boss died, fled or was cleared; sent once.
    BossGone,
    /// Wall of Flesh boss bar and sweep state, sent while active.
    WofState {
        wall_x: f32,
        dir: i8,
        life: i32,
        life_max: i32,
    },
    /// Wall of Flesh died, crushed, or cleared; sent once.
    WofGone,
    /// Broadcast once when the Wall of Flesh is defeated; opens the Victory overlay.
    WallOfFleshDefeated,
    CraftResult {
        recipe: RecipeId,
        crafted: u32,
        error: Option<CraftError>,
    },
    /// Progression facts for world and character.
    SyncFacts {
        world: WorldFacts,
        character: CharacterFacts,
    },
    /// Authoritative combat hit event for client-side feedback.
    CombatHit {
        pos: [f32; 3],
        attacker_pos: Option<[f32; 3]>,
        amount: i32,
        crit: bool,
        damage_kind: DamageKind,
        target_kind: HitTargetKind,
        killed: bool,
    },
    /// Emitted when an enemy begins a ranged attack wind-up.
    EnemyTelegraph {
        id: EnemyId,
        kind: EnemyKind,
    },
    /// Creative god-mode state toggled on/off for this player.
    GodMode(bool),
    /// Current item placed in the Guide's crafting lookup slot.
    GuideSlot(Option<ItemStack>),
    /// Outcome of `HealAtNurse`.
    NurseHealed {
        ok: bool,
        healed: i32,
        cost_copper: u32,
    },
    /// Outcome of `ReforgeItem`.
    ReforgeResult {
        ok: bool,
        slot: u8,
        new_prefix: Prefix,
        cost_copper: u32,
    },
}

/// The other end of a transport has been dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Disconnected;

/// Client side of a transport.
pub trait ClientTransport: Send {
    fn send(&self, m: ClientMsg) -> Result<(), Disconnected>;
    fn try_recv(&self) -> Result<Option<ServerMsg>, Disconnected>;
}

/// Server side of one client's connection.
pub trait ServerConn: Send {
    fn send(&self, m: ServerMsg) -> Result<(), Disconnected>;
    fn try_recv(&self) -> Result<Option<ClientMsg>, Disconnected>;
}

pub struct InProcClient {
    tx: Sender<ClientMsg>,
    rx: Receiver<ServerMsg>,
}

pub struct InProcServerConn {
    tx: Sender<ServerMsg>,
    rx: Receiver<ClientMsg>,
}

/// Creates a connected client/server pair backed by two channels.
pub fn in_process_pair() -> (InProcClient, InProcServerConn) {
    let (to_server, from_client) = mpsc::channel();
    let (to_client, from_server) = mpsc::channel();
    (
        InProcClient {
            tx: to_server,
            rx: from_server,
        },
        InProcServerConn {
            tx: to_client,
            rx: from_client,
        },
    )
}

impl ClientTransport for InProcClient {
    fn send(&self, m: ClientMsg) -> Result<(), Disconnected> {
        self.tx.send(m).map_err(|_| Disconnected)
    }

    fn try_recv(&self) -> Result<Option<ServerMsg>, Disconnected> {
        match self.rx.try_recv() {
            Ok(m) => Ok(Some(m)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(Disconnected),
        }
    }
}

impl ServerConn for InProcServerConn {
    fn send(&self, m: ServerMsg) -> Result<(), Disconnected> {
        self.tx.send(m).map_err(|_| Disconnected)
    }

    fn try_recv(&self) -> Result<Option<ClientMsg>, Disconnected> {
        match self.rx.try_recv() {
            Ok(m) => Ok(Some(m)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(Disconnected),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pair_roundtrip() {
        let (client, server) = in_process_pair();
        client
            .send(ClientMsg::Hello {
                seed: 1,
                name: None,
                character: None,
            })
            .unwrap();
        let got = server.try_recv().unwrap();
        assert!(matches!(got, Some(ClientMsg::Hello { seed: 1, .. })));

        server
            .send(ServerMsg::Welcome {
                id: 7,
                spawn: [1.0, 2.0, 3.0],
                size: WorldSize::Small,
            })
            .unwrap();
        let got = client.try_recv().unwrap();
        assert!(matches!(got, Some(ServerMsg::Welcome { id: 7, .. })));
    }

    #[test]
    fn hello_with_character_roundtrips_over_net() {
        let (client, server) = in_process_pair();
        let char_bytes = vec![1, 2, 3, 4, 5];
        client
            .send(ClientMsg::Hello {
                seed: 42,
                name: Some("Hero".into()),
                character: Some(char_bytes.clone()),
            })
            .unwrap();
        let got = server.try_recv().unwrap();
        assert!(matches!(
            got,
            Some(ClientMsg::Hello {
                seed: 42,
                name: Some(ref n),
                character: Some(ref b),
            }) if n == "Hero" && b == &char_bytes
        ));
    }

    #[test]
    fn craft_messages_roundtrip() {
        let (client, server) = in_process_pair();
        client
            .send(ClientMsg::Craft {
                recipe: RecipeId(1),
                count: 2,
            })
            .unwrap();
        assert!(matches!(
            server.try_recv().unwrap(),
            Some(ClientMsg::Craft {
                recipe: RecipeId(1),
                count: 2
            })
        ));
        server
            .send(ServerMsg::CraftResult {
                recipe: RecipeId(1),
                crafted: 0,
                error: Some(CraftError::MissingStation),
            })
            .unwrap();
        assert!(matches!(
            client.try_recv().unwrap(),
            Some(ServerMsg::CraftResult {
                recipe: RecipeId(1),
                crafted: 0,
                error: Some(CraftError::MissingStation)
            })
        ));
    }

    #[test]
    fn try_recv_empty_is_none() {
        let (client, server) = in_process_pair();
        assert!(matches!(client.try_recv(), Ok(None)));
        assert!(matches!(server.try_recv(), Ok(None)));
    }

    #[test]
    fn send_after_disconnect_is_err() {
        let (client, server) = in_process_pair();
        drop(server);
        assert_eq!(client.send(ClientMsg::Bye), Err(Disconnected));
        assert_eq!(client.try_recv().map(|_| ()), Err(Disconnected));
    }

    #[test]
    fn liquid_delta_roundtrip() {
        let (client, server) = in_process_pair();
        let cp = ChunkPos { x: 1, y: 2, z: 3 };
        let cells = vec![(10, 255), (20, 0)];
        server
            .send(ServerMsg::LiquidDelta {
                pos: cp,
                cells: cells.clone(),
            })
            .unwrap();
        let got = client.try_recv().unwrap();
        assert!(matches!(
            got,
            Some(ServerMsg::LiquidDelta { pos, cells: c }) if pos == cp && c == cells
        ));
    }

    #[test]
    fn potion_sickness_ticks_in_stats_msg() {
        use crate::player::PlayerBody;
        use crate::player::vitals::Vitals;
        let (client, server) = in_process_pair();
        let mut vitals = Vitals::new();
        vitals.potion_delay_ticks = 3600;
        server
            .send(ServerMsg::PlayerState {
                ack: 1,
                body: PlayerBody::new([0.0; 3]),
                vitals,
                dead: false,
                respawn_ticks: 0,
            })
            .unwrap();
        let got = client.try_recv().unwrap().expect("message");
        match got {
            ServerMsg::PlayerState { vitals, .. } => {
                assert_eq!(vitals.potion_delay_ticks, 3600);
            }
            _ => panic!("expected PlayerState message"),
        }
    }
}
