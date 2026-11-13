//! ゲーム本体（Scheme 非依存）。各プリミティブは Rust のメソッドとして実装し、
//! `script.rs` が薄いラッパーとして steel に登録する。

use std::collections::HashSet;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::dungeon::{Dungeon, Floor, ItemKind, ItemPlacement, Pos, Tile};

use super::battle::{self, BattleReport};
use super::consts::*;
use super::event::{BattleEnd, ExitKind, GameEvent, Outcome};
use super::memory::FloorMemory;
use super::types::{Dir, Party};

/// ゲームが終了した（または中断された）ので、これ以上行動できない。
/// 理由は `World::status` にある。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Halt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveResult {
    Moved,
    Blocked,
    BattleWon,
    PartyWiped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestResult {
    Rested,
    BattleWon,
    PartyWiped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FollowResult {
    Arrived,
    Blocked,
    BattleWon,
    PartyWiped,
}

pub struct World {
    dungeon: Dungeon,
    pub party: Party,
    /// 床上のアイテム（`Floor.items` を実行時にコピーして管理）。添字 = 階 - 1。
    floor_items: Vec<Vec<ItemPlacement>>,
    memory: Vec<FloorMemory>,
    opened_doors: HashSet<(usize, Pos)>,
    rng: ChaCha8Rng,
    pub events: Vec<GameEvent>,
    turn_limit: u32,
    /// `Some` になったら終了済み。
    pub status: Option<Outcome>,
    idle_streak: u32,
    host_calls: u64,
}

impl World {
    pub fn new(dungeon: &Dungeon, game_seed: u64, turn_limit: u32) -> Self {
        let first = &dungeon.floors[0];
        let mut w = World {
            dungeon: dungeon.clone(),
            party: Party::new(first.stairs_up),
            floor_items: dungeon.floors.iter().map(|f| f.items.clone()).collect(),
            memory: dungeon.floors.iter().map(FloorMemory::new).collect(),
            opened_doors: HashSet::new(),
            rng: ChaCha8Rng::seed_from_u64(game_seed),
            events: Vec::new(),
            turn_limit,
            status: None,
            idle_streak: 0,
            host_calls: 0,
        };
        w.reveal_here();
        w
    }

    // ---- 内部ヘルパー ----

    fn floor(&self) -> &Floor {
        &self.dungeon.floors[self.party.floor - 1]
    }

    fn mem(&self) -> &FloorMemory {
        &self.memory[self.party.floor - 1]
    }

    fn reveal_here(&mut self) {
        let i = self.party.floor - 1;
        self.memory[i].arrive(&self.dungeon.floors[i], self.party.pos);
    }

    fn halt(&mut self, outcome: Outcome) -> Halt {
        self.status.get_or_insert(outcome);
        Halt
    }

    /// すべてのホスト関数の入口。終了済みなら中断し、呼び出し回数を数える。
    pub fn guard(&mut self) -> Result<(), Halt> {
        if self.status.is_some() {
            return Err(Halt);
        }
        self.host_calls += 1;
        if self.host_calls > MAX_HOST_CALLS {
            return Err(self.halt(Outcome::ScriptError("too many host calls".into())));
        }
        Ok(())
    }

    /// `cost` ターンを消費する行動を始める。上限を超えるなら TurnLimit で中断。
    fn spend(&mut self, cost: u32) -> Result<(), Halt> {
        if self.status.is_some() {
            return Err(Halt);
        }
        if self.party.turn + cost > self.turn_limit {
            return Err(self.halt(Outcome::TurnLimit));
        }
        self.party.turn += cost;
        self.idle_streak = 0;
        Ok(())
    }

    fn encounter_percent(&self, table: &[u32; 5]) -> u32 {
        table[(self.party.floor - 1).min(table.len() - 1)]
    }

    /// 確率で戦闘を起こして解決する。
    fn maybe_battle(&mut self, percent: u32) -> Option<BattleEnd> {
        if self.rng.random_range(0..100) >= percent {
            return None;
        }
        let enemies = battle::roll_enemy_count(&mut self.rng);
        let floor = self.party.floor;
        self.events.push(GameEvent::Encountered { turn: self.party.turn, floor, enemies });
        let BattleReport { result, rounds, damage_dealt, damage_taken, mp_used, members_lost, healed, .. } =
            battle::fight(&mut self.party, floor, enemies, &mut self.rng);
        self.events.push(GameEvent::BattleResult {
            turn: self.party.turn,
            floor,
            result,
            enemies,
            rounds,
            damage_dealt,
            damage_taken,
            mp_used,
            members_lost,
            healed,
        });
        if result == BattleEnd::Wiped {
            self.halt(Outcome::PartyWiped);
        }
        Some(result)
    }

    fn front(&self) -> Pos {
        self.party.facing.step(self.party.pos)
    }

    // ---- 行動 ----

    pub fn do_move(&mut self) -> Result<MoveResult, Halt> {
        if self.status.is_some() {
            return Err(Halt);
        }
        let to = self.front();
        if !self.floor().is_walkable(to) {
            self.events.push(GameEvent::Blocked {
                turn: self.party.turn,
                floor: self.party.floor,
                pos: self.party.pos,
                facing: self.party.facing,
            });
            self.idle_streak += 1;
            if self.idle_streak > MAX_IDLE_ACTIONS {
                return Err(self.halt(Outcome::ScriptError("too many consecutive blocked moves".into())));
            }
            return Ok(MoveResult::Blocked);
        }
        self.spend(MOVE_COST)?;
        let from = std::mem::replace(&mut self.party.pos, to);
        let floor = self.party.floor;
        self.events.push(GameEvent::Moved { turn: self.party.turn, floor, from, to });
        if self.floor().tile(to) == Some(Tile::Door) && self.opened_doors.insert((floor, to)) {
            self.events.push(GameEvent::DoorOpened { turn: self.party.turn, floor, pos: to });
        }
        self.reveal_here();
        let pct = self.encounter_percent(&MOVE_ENCOUNTER_PERCENT);
        Ok(match self.maybe_battle(pct) {
            Some(BattleEnd::Won) => MoveResult::BattleWon,
            Some(BattleEnd::Wiped) => MoveResult::PartyWiped,
            Some(BattleEnd::Fled) | None => MoveResult::Moved,
        })
    }

    fn turn_to(&mut self, facing: Dir) -> Result<(), Halt> {
        self.spend(TURN_COST)?;
        self.party.facing = facing;
        self.events.push(GameEvent::Turned { turn: self.party.turn, facing });
        Ok(())
    }

    pub fn turn_left(&mut self) -> Result<(), Halt> {
        self.turn_to(self.party.facing.turn_left())
    }

    pub fn turn_right(&mut self) -> Result<(), Halt> {
        self.turn_to(self.party.facing.turn_right())
    }

    pub fn rest(&mut self) -> Result<RestResult, Halt> {
        self.spend(REST_COST)?;
        let mut healed = 0;
        for m in self.party.members.iter_mut().filter(|m| m.is_alive()) {
            let hp = (m.max_hp * REST_HEAL_PERCENT / 100).min(m.max_hp - m.hp);
            m.hp += hp;
            healed += hp;
            m.mp = (m.mp + m.max_mp * REST_HEAL_PERCENT / 100).min(m.max_mp);
        }
        self.events.push(GameEvent::Rested { turn: self.party.turn, healed });
        let pct = self.encounter_percent(&REST_ENCOUNTER_PERCENT);
        Ok(match self.maybe_battle(pct) {
            Some(BattleEnd::Won) => RestResult::BattleWon,
            Some(BattleEnd::Wiped) => RestResult::PartyWiped,
            Some(BattleEnd::Fled) | None => RestResult::Rested,
        })
    }

    /// 失敗（階段の上にいない）でもターンは消費する。
    pub fn descend(&mut self) -> Result<bool, Halt> {
        self.spend(STAIRS_COST)?;
        let from = self.party.floor;
        let Some(down) = self.floor().stairs_down else { return Ok(false) };
        if self.party.pos != down {
            return Ok(false);
        }
        self.party.floor += 1;
        self.party.pos = self.floor().stairs_up;
        self.events.push(GameEvent::Descended { turn: self.party.turn, from, to: self.party.floor });
        self.reveal_here();
        Ok(true)
    }

    /// B1F の入口では上れない（`exit_dungeon` を使う）。失敗でもターンは消費する。
    pub fn ascend(&mut self) -> Result<bool, Halt> {
        self.spend(STAIRS_COST)?;
        let from = self.party.floor;
        if from == 1 || self.party.pos != self.floor().stairs_up {
            return Ok(false);
        }
        self.party.floor -= 1;
        self.party.pos = self.floor().stairs_down.expect("upper floor has stairs down");
        self.events.push(GameEvent::Ascended { turn: self.party.turn, from, to: self.party.floor });
        self.reveal_here();
        Ok(true)
    }

    pub fn item_here(&self) -> Option<ItemKind> {
        self.floor_items[self.party.floor - 1].iter().find(|i| i.pos == self.party.pos).map(|i| i.kind)
    }

    /// 失敗（足元に何もない）でもターンは消費する。
    pub fn pickup(&mut self) -> Result<bool, Halt> {
        self.spend(PICKUP_COST)?;
        let items = &mut self.floor_items[self.party.floor - 1];
        let Some(i) = items.iter().position(|i| i.pos == self.party.pos) else { return Ok(false) };
        let item = items.remove(i);
        self.party.items.push(item.kind);
        self.events.push(GameEvent::PickedUp {
            turn: self.party.turn,
            floor: self.party.floor,
            pos: item.pos,
            item: item.kind,
        });
        Ok(true)
    }

    pub fn entrance_pos(&self) -> Pos {
        self.dungeon.floors[0].stairs_up
    }

    /// 入口で呼ぶと冒険が終わる。魔導書があれば勝利、なければ `Exited(NoBook)`。
    /// 入口以外では `false`（ターンは消費する）。
    pub fn exit_dungeon(&mut self) -> Result<bool, Halt> {
        self.spend(EXIT_COST)?;
        if self.party.floor != 1 || self.party.pos != self.entrance_pos() {
            return Ok(false);
        }
        let with_book = self.party.has_item(ItemKind::MagicBook);
        self.events.push(GameEvent::Exited { turn: self.party.turn, with_book });
        self.halt(if with_book { Outcome::Won } else { Outcome::Exited(ExitKind::NoBook) });
        Ok(true)
    }

    // ---- 状況取得（ターン消費なし） ----

    pub fn tile_at(&self, p: Pos) -> Tile {
        self.floor().tile(p).unwrap_or(Tile::Wall)
    }

    pub fn look(&self) -> Tile {
        self.tile_at(self.front())
    }

    pub fn tile_here(&self) -> Tile {
        self.tile_at(self.party.pos)
    }

    pub fn known_tile(&self, p: Pos) -> Option<Tile> {
        self.mem().known(p)
    }

    pub fn visited(&self, p: Pos) -> bool {
        self.mem().visited(p)
    }

    pub fn frontier(&self) -> Vec<Pos> {
        self.mem().frontier()
    }

    pub fn path_to(&self, to: Pos) -> Option<Vec<Dir>> {
        self.mem().path(self.party.pos, to)
    }

    /// 方向リストに沿って向き直りと移動を繰り返す。
    /// 壁・戦闘（勝利/全滅）があればその時点で打ち切って理由を返す。
    pub fn follow_path(&mut self, path: &[Dir]) -> Result<FollowResult, Halt> {
        for &dir in path {
            match self.party.facing.clockwise_steps_to(dir) {
                0 => {}
                1 => self.turn_right()?,
                2 => {
                    self.turn_right()?;
                    self.turn_right()?;
                }
                _ => self.turn_left()?,
            }
            match self.do_move()? {
                MoveResult::Moved => {}
                MoveResult::Blocked => return Ok(FollowResult::Blocked),
                MoveResult::BattleWon => return Ok(FollowResult::BattleWon),
                MoveResult::PartyWiped => return Ok(FollowResult::PartyWiped),
            }
        }
        Ok(FollowResult::Arrived)
    }
}
