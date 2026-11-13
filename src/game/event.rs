//! ゲームイベントと実行結果。

use crate::dungeon::{ItemKind, Pos};

use super::types::Dir;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BattleEnd {
    Won,
    Fled,
    Wiped,
}

/// ある時点のメンバーの HP/MP。`Party::members` と同じ並び（倒れたメンバーも含む）。
/// 初期状態は `INITIAL_PARTY` と `class_stats` から復元できる。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemberSnapshot {
    pub hp: i32,
    pub mp: i32,
}

/// 各イベントの `turn` は、その行動が完了した後のターン数。
#[derive(Clone, Debug, PartialEq)]
pub enum GameEvent {
    Moved { turn: u32, floor: usize, from: Pos, to: Pos },
    /// 壁にぶつかった（ターン消費なし）。
    Blocked { turn: u32, floor: usize, pos: Pos, facing: Dir },
    Turned { turn: u32, facing: Dir },
    DoorOpened { turn: u32, floor: usize, pos: Pos },
    Encountered { turn: u32, floor: usize, enemies: usize },
    BattleResult {
        turn: u32,
        floor: usize,
        result: BattleEnd,
        enemies: usize,
        rounds: u32,
        damage_dealt: i32,
        damage_taken: i32,
        mp_used: i32,
        members_lost: usize,
        /// 戦闘後の僧侶の回復量の合計。
        healed: i32,
        /// 戦闘（と戦闘後の回復）の直後の全メンバー。
        party: Vec<MemberSnapshot>,
    },
    /// `healed` は回復したHPの合計。`party` は野営の直後の全メンバー。
    Rested { turn: u32, healed: i32, party: Vec<MemberSnapshot> },
    Descended { turn: u32, from: usize, to: usize },
    Ascended { turn: u32, from: usize, to: usize },
    PickedUp { turn: u32, floor: usize, pos: Pos, item: ItemKind },
    Exited { turn: u32, with_book: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitKind {
    NoBook,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Won,
    PartyWiped,
    TurnLimit,
    ScriptError(String),
    Exited(ExitKind),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RunResult {
    pub outcome: Outcome,
    pub turns: u32,
    pub events: Vec<GameEvent>,
}
