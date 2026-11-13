//! サンプルスクリプトと再現性の統合テスト。

use crawl_dungeon::dungeon::generate_dungeon;
use crawl_dungeon::game::*;

const SAMPLE: &str = include_str!("../scripts/sample.scm");

fn run_seed(seed: u64) -> RunResult {
    run_script_with(&generate_dungeon(seed), SAMPLE, &RunConfig { game_seed: seed, ..Default::default() })
}

#[test]
fn sample_wins_on_ten_seeds() {
    for seed in 0..10u64 {
        let r = run_seed(seed);
        assert_eq!(r.outcome, Outcome::Won, "seed {seed}: turns={}", r.turns);
        assert!(r.turns < 5000);
        // 魔導書を拾って、入口から出ている。
        assert!(r.events.iter().any(|e| matches!(e, GameEvent::PickedUp { .. })));
        assert!(matches!(r.events.last(), Some(GameEvent::Exited { with_book: true, .. })));
        // B5F まで降りている。
        assert!(r.events.iter().any(|e| matches!(e, GameEvent::Descended { to: 5, .. })));
    }
}

#[test]
fn same_inputs_give_identical_results() {
    for seed in [0u64, 4, 7] {
        assert_eq!(run_seed(seed), run_seed(seed));
    }
}

#[test]
fn game_seed_changes_the_battles_but_not_the_dungeon() {
    let d = generate_dungeon(3);
    let a = run_script_with(&d, SAMPLE, &RunConfig { game_seed: 1, ..Default::default() });
    let b = run_script_with(&d, SAMPLE, &RunConfig { game_seed: 2, ..Default::default() });
    assert_ne!(a.events, b.events);
    // 同じゲームシードなら同じ。
    let a2 = run_script_with(&d, SAMPLE, &RunConfig { game_seed: 1, ..Default::default() });
    assert_eq!(a, a2);
}

#[test]
fn party_wipe_ends_the_run() {
    // 休まないサンプルなら、どこかのシードで全滅する。
    let script = SAMPLE.replace("(define REST-BELOW 0.55)", "(define REST-BELOW 0.0)");
    let r = (0..40u64)
        .map(|s| run_script_with(&generate_dungeon(s), &script, &RunConfig { game_seed: s, ..Default::default() }))
        .find(|r| r.outcome == Outcome::PartyWiped)
        .expect("some seed should wipe without resting");
    // 全滅した戦闘が最後のイベント（以降は行動できない）。
    assert!(matches!(r.events.last(), Some(GameEvent::BattleResult { result: BattleEnd::Wiped, .. })));
}

#[test]
fn battle_and_rest_events_carry_the_party_state() {
    use crawl_dungeon::game::consts::{INITIAL_PARTY, class_stats};
    let mut seen = 0;
    for seed in 0..10u64 {
        for e in run_seed(seed).events {
            if let GameEvent::BattleResult { party, .. } | GameEvent::Rested { party, .. } = e {
                seen += 1;
                assert_eq!(party.len(), INITIAL_PARTY.len());
                for (s, &(_, class)) in party.iter().zip(INITIAL_PARTY.iter()) {
                    let (max_hp, max_mp) = class_stats(class);
                    assert!((0..=max_hp).contains(&s.hp) && (0..=max_mp).contains(&s.mp));
                }
            }
        }
    }
    assert!(seen > 0);
}
