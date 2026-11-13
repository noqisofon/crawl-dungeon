//! ビューアの畳み込みロジック（Bevy なし）のテスト。

#[path = "../src/bin/replay/fold.rs"]
#[allow(dead_code)]
mod fold;

use crawl_dungeon::dungeon::{Tile, generate_dungeon};
use crawl_dungeon::game::consts::{INITIAL_PARTY, class_stats};
use crawl_dungeon::game::*;
use fold::{ViewState, fold};

const SAMPLE: &str = include_str!("../scripts/sample.scm");

fn run(seed: u64) -> (crawl_dungeon::dungeon::Dungeon, RunResult) {
    let d = generate_dungeon(seed);
    let r = run_script_with(&d, SAMPLE, &RunConfig { game_seed: seed, ..Default::default() });
    (d, r)
}

#[test]
fn zero_events_is_the_initial_state() {
    let (d, r) = run(3);
    let s = fold(&d, &r.events, 0);
    assert_eq!((s.floor, s.pos, s.facing, s.turn), (1, d.floors[0].stairs_up, Dir::South, 0));
    for (m, &(_, c)) in s.members.iter().zip(INITIAL_PARTY.iter()) {
        assert_eq!((m.hp, m.mp), class_stats(c));
    }
    // 入口は最初から既知。
    assert_eq!(s.memory[0].known(s.pos), Some(Tile::StairsUp));
}

#[test]
fn fold_matches_independent_scan_at_every_prefix() {
    for seed in [0u64, 3, 7] {
        let (d, r) = run(seed);
        let mut pos = d.floors[0].stairs_up;
        let mut floor = 1;
        let mut facing = Dir::South;
        let mut members = fold(&d, &r.events, 0).members;
        let mut turn = 0;
        for n in 0..=r.events.len() {
            let s = fold(&d, &r.events, n);
            assert_eq!(s.applied, n);
            assert_eq!((s.floor, s.pos, s.facing, s.turn), (floor, pos, facing, turn), "seed {seed} n {n}");
            assert_eq!(s.members, members, "seed {seed} n {n}");
            let Some(e) = r.events.get(n) else { break };
            match e {
                GameEvent::Moved { to, floor: f, .. } => (pos, floor) = (*to, *f),
                GameEvent::Turned { facing: f, .. } => facing = *f,
                GameEvent::Descended { to, .. } => (floor, pos) = (*to, d.floors[to - 1].stairs_up),
                GameEvent::Ascended { to, .. } => (floor, pos) = (*to, d.floors[to - 1].stairs_down.unwrap()),
                GameEvent::BattleResult { party, .. } | GameEvent::Rested { party, .. } => members = party.clone(),
                _ => {}
            }
            turn = match e {
                GameEvent::Moved { turn, .. }
                | GameEvent::Blocked { turn, .. }
                | GameEvent::Turned { turn, .. }
                | GameEvent::DoorOpened { turn, .. }
                | GameEvent::Encountered { turn, .. }
                | GameEvent::BattleResult { turn, .. }
                | GameEvent::Rested { turn, .. }
                | GameEvent::Descended { turn, .. }
                | GameEvent::Ascended { turn, .. }
                | GameEvent::PickedUp { turn, .. }
                | GameEvent::Exited { turn, .. } => *turn,
            };
        }
    }
}

#[test]
fn incremental_apply_equals_refold() {
    let (d, r) = run(5);
    let mut s = ViewState::initial(&d);
    for (i, e) in r.events.iter().enumerate() {
        s.apply(&d, e);
        if i % 37 == 0 {
            let f = fold(&d, &r.events, i + 1);
            assert_eq!((s.floor, s.pos, s.facing, s.turn, &s.members), (f.floor, f.pos, f.facing, f.turn, &f.members));
        }
    }
}

#[test]
fn final_state_matches_the_run() {
    for seed in 0..5u64 {
        let (d, r) = run(seed);
        let s = fold(&d, &r.events, usize::MAX);
        assert_eq!(s.applied, r.events.len());
        assert_eq!(s.turn, r.turns);
        // 魔導書を拾って入口へ戻って終了している。
        assert_eq!((s.floor, s.pos), (1, d.floors[0].stairs_up));
        assert!(s.items[4].is_empty());
        // 通ったセルは既知で、記憶は戻っても消えない（B5F の階段も既知）。
        assert!(s.memory[4].known(d.floors[4].stairs_up).is_some());
        // 最後の HP は最後のスナップショットと一致。
        let last = r
            .events
            .iter()
            .rev()
            .find_map(|e| match e {
                GameEvent::BattleResult { party, .. } | GameEvent::Rested { party, .. } => Some(party.clone()),
                _ => None,
            });
        if let Some(p) = last {
            assert_eq!(s.members, p);
        }
    }
}

#[test]
fn fog_grows_only_with_events() {
    let (d, r) = run(3);
    let known = |s: &ViewState| -> usize {
        let fl = &d.floors[0];
        (0..fl.height)
            .flat_map(|y| (0..fl.width).map(move |x| crawl_dungeon::dungeon::Pos::new(x, y)))
            .filter(|&p| s.memory[0].known(p).is_some())
            .count()
    };
    let mut prev = 0;
    for n in (0..60).step_by(5) {
        let k = known(&fold(&d, &r.events, n));
        assert!(k >= prev);
        prev = k;
    }
    assert!(prev > known(&fold(&d, &r.events, 0)));
}
