//! イベント列を先頭から畳み込んで、ある時点の表示状態を作る純粋なロジック。
//! Bevy には依存しない（テストは `tests/replay_fold.rs` が `#[path]` で取り込む）。

use crawl_dungeon::dungeon::{Dungeon, ItemPlacement, Pos};
use crawl_dungeon::game::consts::{INITIAL_PARTY, class_stats};
use crawl_dungeon::game::{Dir, FloorMemory, GameEvent, MemberSnapshot};

/// `events` の先頭 `applied` 個を適用した時点の表示状態。
#[derive(Clone, Debug)]
pub struct ViewState {
    pub applied: usize,
    pub turn: u32,
    /// 階番号（B1F = 1）。
    pub floor: usize,
    pub pos: Pos,
    pub facing: Dir,
    /// `INITIAL_PARTY` と同じ並び。
    pub members: Vec<MemberSnapshot>,
    /// まだ床に残っているアイテム。添字 = 階 - 1。
    pub items: Vec<Vec<ItemPlacement>>,
    /// 階ごとのマップ記憶。添字 = 階 - 1。
    pub memory: Vec<FloorMemory>,
}

impl ViewState {
    /// 冒険の開始時点（B1F の入口、南向き、全員満タン）。
    pub fn initial(dungeon: &Dungeon) -> Self {
        let first = &dungeon.floors[0];
        let mut memory: Vec<FloorMemory> = dungeon.floors.iter().map(FloorMemory::new).collect();
        memory[0].arrive(first, first.stairs_up);
        Self {
            applied: 0,
            turn: 0,
            floor: 1,
            pos: first.stairs_up,
            facing: Dir::South,
            members: INITIAL_PARTY
                .iter()
                .map(|&(_, c)| {
                    let (hp, mp) = class_stats(c);
                    MemberSnapshot { hp, mp }
                })
                .collect(),
            items: dungeon.floors.iter().map(|f| f.items.clone()).collect(),
            memory,
        }
    }

    fn arrive(&mut self, dungeon: &Dungeon) {
        let i = self.floor - 1;
        self.memory[i].arrive(&dungeon.floors[i], self.pos);
    }

    /// イベントを1つ適用する。
    pub fn apply(&mut self, dungeon: &Dungeon, event: &GameEvent) {
        self.applied += 1;
        match event {
            GameEvent::Moved { turn, floor, to, .. } => {
                self.turn = *turn;
                self.floor = *floor;
                self.pos = *to;
                self.arrive(dungeon);
            }
            GameEvent::Turned { turn, facing } => {
                self.turn = *turn;
                self.facing = *facing;
            }
            GameEvent::Descended { turn, to, .. } => {
                self.turn = *turn;
                self.floor = *to;
                self.pos = dungeon.floors[to - 1].stairs_up;
                self.arrive(dungeon);
            }
            GameEvent::Ascended { turn, to, .. } => {
                self.turn = *turn;
                self.floor = *to;
                self.pos = dungeon.floors[to - 1].stairs_down.expect("upper floor has stairs down");
                self.arrive(dungeon);
            }
            GameEvent::PickedUp { turn, floor, pos, .. } => {
                self.turn = *turn;
                self.items[floor - 1].retain(|i| i.pos != *pos);
            }
            GameEvent::BattleResult { turn, party, .. } | GameEvent::Rested { turn, party, .. } => {
                self.turn = *turn;
                self.members.clone_from(party);
            }
            GameEvent::Blocked { turn, .. }
            | GameEvent::DoorOpened { turn, .. }
            | GameEvent::Encountered { turn, .. }
            | GameEvent::Exited { turn, .. } => self.turn = *turn,
        }
    }
}

/// 先頭から `n` イベントまで畳み込む（`n` は長さに丸める）。シークはすべてこれで行う。
pub fn fold(dungeon: &Dungeon, events: &[GameEvent], n: usize) -> ViewState {
    let mut s = ViewState::initial(dungeon);
    for e in &events[..n.min(events.len())] {
        s.apply(dungeon, e);
    }
    s
}

/// ログ用の1行の説明。
pub fn describe(e: &GameEvent) -> String {
    use GameEvent::*;
    match e {
        Moved { turn, to, .. } => format!("[{turn}] move -> ({}, {})", to.x, to.y),
        Blocked { turn, facing, .. } => format!("[{turn}] blocked ({})", facing.symbol()),
        Turned { turn, facing } => format!("[{turn}] turn {}", facing.symbol()),
        DoorOpened { turn, .. } => format!("[{turn}] door opened"),
        Encountered { turn, enemies, .. } => format!("[{turn}] encounter x{enemies}"),
        BattleResult { turn, result, rounds, damage_dealt, damage_taken, .. } => {
            format!("[{turn}] battle {result:?} r{rounds} dealt {damage_dealt} taken {damage_taken}")
        }
        Rested { turn, healed, .. } => format!("[{turn}] rest +{healed}hp"),
        Descended { turn, to, .. } => format!("[{turn}] descend -> B{to}F"),
        Ascended { turn, to, .. } => format!("[{turn}] ascend -> B{to}F"),
        PickedUp { turn, item, .. } => format!("[{turn}] picked up {item:?}"),
        Exited { turn, with_book } => format!("[{turn}] exit (book: {with_book})"),
    }
}
