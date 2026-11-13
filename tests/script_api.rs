//! 各プリミティブの単体テスト（Rust API と Scheme API の両方）。

use crawl_dungeon::dungeon::*;
use crawl_dungeon::game::consts::*;
use crawl_dungeon::game::*;

const SEED: u64 = 3;

fn dungeon() -> Dungeon {
    generate_dungeon(SEED)
}

fn world(d: &Dungeon) -> World {
    World::new(d, 1, DEFAULT_TURN_LIMIT)
}

/// 前方が壁/歩行可能になるまで右を向く。
fn face(w: &mut World, walkable: bool) {
    for _ in 0..4 {
        if w.look().is_walkable() == walkable {
            return;
        }
        w.turn_right().unwrap();
    }
    panic!("no such direction at {:?}", w.party.pos);
}

fn run(d: &Dungeon, script: &str) -> RunResult {
    run_script(d, script)
}

// ---------- 初期状態 ----------

#[test]
fn initial_state() {
    let d = dungeon();
    let w = world(&d);
    assert_eq!(w.party.pos, d.floors[0].stairs_up);
    assert_eq!(w.party.floor, 1);
    assert_eq!(w.party.facing, Dir::South);
    assert_eq!(w.party.turn, 0);
    assert!(w.party.items.is_empty());
    let classes: Vec<_> = w.party.members.iter().map(|m| m.class).collect();
    use Class::*;
    assert_eq!(classes, [Fighter, Fighter, Fighter, Thief, Mage, Priest]);
    assert!(w.party.members.iter().all(|m| m.hp == m.max_hp && m.mp == m.max_mp));
}

#[test]
fn info_primitives_from_scheme() {
    let d = dungeon();
    let r = run(
        &d,
        "(assert! (equal? (facing) 'south))
         (assert! (equal? (position) (entrance-pos)))
         (assert! (= (floor-level) 1))
         (assert! (equal? (tile-here) 'stairs-up))
         (assert! (equal? (item-here) #f))
         (assert! (equal? (has-item? 'magic-book) #f))
         (assert! (= (alive-count) 6))
         (assert! (= (party-hp-ratio) 1.0))
         (assert! (equal? (member-class 0) 'fighter))
         (assert! (equal? (member-class 3) 'thief))
         (assert! (equal? (member-class 4) 'mage))
         (assert! (equal? (member-class 5) 'priest))
         (assert! (> (member-hp 0) 0))
         (assert! (equal? (member-hp 99) #f))
         (assert! (equal? (known-tile 0 0) 'unknown))
         (exit-dungeon)",
    );
    // 情報取得はターンを消費しない。exit-dungeon の 1 ターンだけ。
    assert_eq!(r.outcome, Outcome::Exited(ExitKind::NoBook), "{r:?}");
    assert_eq!(r.turns, 1);
}

// ---------- 行動 ----------

#[test]
fn blocked_move_costs_no_turn() {
    let d = dungeon();
    let mut w = world(&d);
    face(&mut w, false);
    let (pos, turn) = (w.party.pos, w.party.turn);
    assert_eq!(w.do_move(), Ok(MoveResult::Blocked));
    assert_eq!((w.party.pos, w.party.turn), (pos, turn));
    assert!(matches!(w.events.last(), Some(GameEvent::Blocked { .. })));
}

#[test]
fn move_advances_position_and_turn() {
    let d = dungeon();
    let mut w = world(&d);
    face(&mut w, true);
    let (from, turn) = (w.party.pos, w.party.turn);
    let to = w.party.facing.step(from);
    let r = w.do_move().unwrap();
    assert!(matches!(r, MoveResult::Moved | MoveResult::BattleWon), "{r:?}");
    assert_eq!((w.party.pos, w.party.turn), (to, turn + MOVE_COST));
    assert!(w.events.iter().any(|e| matches!(e, GameEvent::Moved { from: f, to: t, .. } if *f == from && *t == to)));
}

#[test]
fn turning_cycles_and_costs_turns() {
    let d = dungeon();
    let mut w = world(&d);
    w.turn_left().unwrap();
    assert_eq!(w.party.facing, Dir::East);
    w.turn_left().unwrap();
    assert_eq!(w.party.facing, Dir::North);
    w.turn_right().unwrap();
    assert_eq!(w.party.facing, Dir::East);
    assert_eq!(w.party.turn, 3 * TURN_COST);
    let turned = w.events.iter().filter(|e| matches!(e, GameEvent::Turned { .. })).count();
    assert_eq!(turned, 3);
}

#[test]
fn stairs_down_and_up() {
    let d = dungeon();
    let mut w = world(&d);
    let down = d.floors[0].stairs_down.unwrap();

    // 階段の上でなければ失敗（ターンは消費する）。
    assert_eq!(w.descend(), Ok(false));
    assert_eq!(w.party.floor, 1);
    assert_eq!(w.party.turn, STAIRS_COST);

    w.party.pos = down;
    assert_eq!(w.tile_here(), Tile::StairsDown);
    assert_eq!(w.descend(), Ok(true));
    assert_eq!(w.party.floor, 2);
    assert_eq!(w.party.pos, d.floors[1].stairs_up);
    assert!(w.events.iter().any(|e| matches!(e, GameEvent::Descended { from: 1, to: 2, .. })));
    // 新しい階では到着地点が既知・訪問済み。
    assert!(w.visited(w.party.pos));

    assert_eq!(w.ascend(), Ok(true));
    assert_eq!(w.party.floor, 1);
    assert_eq!(w.party.pos, down);
    assert!(w.events.iter().any(|e| matches!(e, GameEvent::Ascended { from: 2, to: 1, .. })));
}

#[test]
fn cannot_ascend_from_entrance_or_descend_on_last_floor() {
    let d = dungeon();
    let mut w = world(&d);
    assert_eq!(w.ascend(), Ok(false)); // B1F の入口。上がるのではなく exit-dungeon。
    assert_eq!(w.party.floor, 1);
    w.party.floor = FLOOR_COUNT;
    w.party.pos = d.floors[FLOOR_COUNT - 1].stairs_up;
    assert_eq!(w.descend(), Ok(false));
}

#[test]
fn pickup_magic_book() {
    let d = dungeon();
    let mut w = world(&d);
    let book = d.floors[FLOOR_COUNT - 1].items[0];
    w.party.floor = FLOOR_COUNT;
    w.party.pos = book.pos;

    assert_eq!(w.item_here(), Some(ItemKind::MagicBook));
    assert_eq!(w.pickup(), Ok(true));
    assert!(w.party.has_item(ItemKind::MagicBook));
    assert_eq!(w.item_here(), None);
    assert_eq!(w.pickup(), Ok(false)); // もう無い
    assert_eq!(w.party.items.len(), 1);
    assert!(w.events.iter().any(|e| matches!(e, GameEvent::PickedUp { item: ItemKind::MagicBook, .. })));
    // 元のダンジョンは書き換わらない（実行時にコピーして管理している）。
    assert_eq!(d.floors[FLOOR_COUNT - 1].items.len(), 1);
}

#[test]
fn pickup_fails_on_empty_floor() {
    let d = dungeon();
    let mut w = world(&d);
    assert_eq!(w.pickup(), Ok(false));
    assert_eq!(w.party.turn, PICKUP_COST);
}

#[test]
fn exit_without_book_ends_run() {
    let d = dungeon();
    let r = run(&d, "(exit-dungeon)");
    assert_eq!(r.outcome, Outcome::Exited(ExitKind::NoBook));
    assert!(matches!(r.events.last(), Some(GameEvent::Exited { with_book: false, .. })));
}

#[test]
fn exit_with_book_wins() {
    let d = dungeon();
    let mut w = world(&d);
    w.party.items.push(ItemKind::MagicBook);
    assert_eq!(w.exit_dungeon(), Ok(true));
    assert_eq!(w.status, Some(Outcome::Won));
    // 終了後は何もできない。
    assert_eq!(w.do_move(), Err(Halt));
    assert_eq!(w.guard(), Err(Halt));
}

#[test]
fn exit_away_from_entrance_fails_without_ending() {
    let d = dungeon();
    let mut w = world(&d);
    w.party.items.push(ItemKind::MagicBook);
    w.party.pos = d.floors[0].stairs_down.unwrap();
    assert_eq!(w.exit_dungeon(), Ok(false));
    assert_eq!(w.status, None);
}

#[test]
fn rest_heals_and_costs_ten_turns() {
    let d = dungeon();
    let mut w = world(&d);
    for m in &mut w.party.members {
        m.hp = 1;
        m.mp = 0;
    }
    let r = w.rest().unwrap();
    assert!(matches!(r, RestResult::Rested | RestResult::BattleWon), "{r:?}");
    assert_eq!(w.party.turn, REST_COST);
    let rested = w.events.iter().find_map(|e| match e {
        GameEvent::Rested { healed, .. } => Some(*healed),
        _ => None,
    });
    assert!(rested.unwrap() > 0);
    let fighter = &w.party.members[0];
    assert!(fighter.hp > 1 || fighter.hp == 0);
    let mage = &w.party.members[4];
    assert!(mage.hp == 0 || mage.mp > 0);
}

#[test]
fn rest_does_not_revive_the_fallen() {
    let d = dungeon();
    let mut w = world(&d);
    w.party.members[0].hp = 0;
    w.rest().unwrap();
    assert_eq!(w.party.members[0].hp, 0);
}

// ---------- マップ記憶 ----------

#[test]
fn initial_memory_is_limited_to_what_is_seen() {
    let d = dungeon();
    let w = world(&d);
    let start = w.party.pos;
    assert!(w.visited(start));
    assert_eq!(w.known_tile(start), Some(Tile::StairsUp));
    let f = &d.floors[0];
    let known = (0..f.height)
        .flat_map(|y| (0..f.width).map(move |x| Pos::new(x, y)))
        .filter(|&p| w.known_tile(p).is_some())
        .count();
    assert!(known < f.tiles.len() / 4, "known {known}");
    // 他の部屋は未知。
    let far = f.rooms.iter().rev().find(|r| !r.contains(start)).unwrap().center();
    assert_eq!(w.known_tile(far), None);
    assert!(!w.visited(far));
}

#[test]
fn entering_a_room_reveals_the_whole_room() {
    let d = dungeon();
    let w = world(&d);
    let room = d.floors[0].rooms.iter().find(|r| r.contains(w.party.pos)).unwrap();
    for p in room.cells() {
        assert!(w.known_tile(p).is_some(), "{p:?} should be known");
    }
}

#[test]
fn sight_stops_at_walls() {
    let d = dungeon();
    let w = world(&d);
    let f = &d.floors[0];
    for dir in Dir::ALL {
        let mut p = dir.step(w.party.pos);
        let mut hit_wall = false;
        while f.in_bounds(p) && !hit_wall {
            assert!(w.known_tile(p).is_some(), "{p:?} along {dir:?}");
            hit_wall = f.tile(p) == Some(Tile::Wall);
            p = dir.step(p);
        }
    }
}

#[test]
fn frontier_cells_are_known_walkable_next_to_unknown() {
    let d = dungeon();
    let w = world(&d);
    let frontier = w.frontier();
    assert!(!frontier.is_empty());
    for p in frontier {
        assert!(w.known_tile(p).is_some_and(Tile::is_walkable));
        assert!(Dir::ALL.iter().any(|dir| {
            let q = dir.step(p);
            d.floors[0].in_bounds(q) && w.known_tile(q).is_none()
        }));
    }
}

#[test]
fn visiting_a_frontier_updates_visited_and_frontier() {
    let d = dungeon();
    let mut w = world(&d);
    let target = w.frontier().into_iter().filter(|&p| p != w.party.pos).min_by_key(|&p| w.path_to(p).map(|x| x.len())).unwrap();
    let path = w.path_to(target).unwrap();
    assert!(!path.is_empty());
    assert!(!w.visited(target));
    let r = w.follow_path(&path).unwrap();
    if r == FollowResult::Arrived {
        assert_eq!(w.party.pos, target);
        assert!(w.visited(target));
        assert!(!w.frontier().contains(&target));
    }
}

#[test]
fn path_to_uses_only_known_map() {
    let d = dungeon();
    let w = world(&d);
    let f = &d.floors[0];
    let far = f.rooms.iter().rev().find(|r| !r.contains(w.party.pos)).unwrap().center();
    // 実際には到達可能だが、まだ知らないので経路は無い。
    assert!(shortest_path(f, w.party.pos, far).is_some());
    assert_eq!(w.path_to(far), None);
    // 壁や範囲外も None。
    assert_eq!(w.path_to(Pos::new(-1, -1)), None);
    // 現在地は空の経路。
    assert_eq!(w.path_to(w.party.pos), Some(vec![]));
}

#[test]
fn follow_path_stops_when_blocked() {
    let d = dungeon();
    let mut w = world(&d);
    face(&mut w, false);
    let wall_dir = w.party.facing;
    let before = w.party.pos;
    assert_eq!(w.follow_path(&[wall_dir, wall_dir]), Ok(FollowResult::Blocked));
    assert_eq!(w.party.pos, before);
}

#[test]
fn follow_path_turns_then_moves() {
    let d = dungeon();
    let mut w = world(&d);
    face(&mut w, true);
    let dir = w.party.facing.turn_right().turn_right(); // 反対側
    let start = w.party.pos;
    let r = w.follow_path(&[dir]).unwrap();
    if w.floor_is_walkable_for_test(start, dir) {
        assert!(matches!(r, FollowResult::Arrived | FollowResult::BattleWon));
        assert_eq!(w.party.facing, dir);
    }
}

trait WalkableExt {
    fn floor_is_walkable_for_test(&self, from: Pos, dir: Dir) -> bool;
}
impl WalkableExt for World {
    fn floor_is_walkable_for_test(&self, from: Pos, dir: Dir) -> bool {
        self.known_tile(dir.step(from)).is_some_and(Tile::is_walkable)
    }
}

#[test]
fn scheme_map_memory_api() {
    let d = dungeon();
    let r = run(
        &d,
        "(define p (position))
         (assert! (visited? (car p) (car (cdr p))))
         (assert! (not (visited? 0 0)))
         (assert! (equal? (known-tile (car p) (car (cdr p))) 'stairs-up))
         (assert! (pair? (frontier)))
         (assert! (equal? (path-to 0 0) #f))
         (assert! (equal? (path-to (car p) (car (cdr p))) '()))
         (define f (car (frontier)))
         (assert! (list? (path-to (car f) (car (cdr f)))))
         (exit-dungeon)",
    );
    assert_eq!(r.outcome, Outcome::Exited(ExitKind::NoBook), "{r:?}");
}

#[test]
fn scheme_actions_and_follow_path() {
    let d = dungeon();
    let r = run(
        &d,
        "(define (loop-until-open)
           (if (can-move?) 'ok (begin (turn-right) (loop-until-open))))
         (loop-until-open)
         (define r (move))
         (assert! (or (equal? r 'moved) (equal? r 'battle-won)))
         (define back (path-to (car (entrance-pos)) (car (cdr (entrance-pos)))))
         (assert! (= (length back) 1))
         (assert! (equal? (follow-path back) 'arrived))
         (assert! (equal? (position) (entrance-pos)))
         (exit-dungeon)",
    );
    assert_eq!(r.outcome, Outcome::Exited(ExitKind::NoBook), "{r:?}");
}

// ---------- 終了条件 ----------

#[test]
fn turn_limit_stops_the_script() {
    let d = dungeon();
    let cfg = RunConfig { turn_limit: 50, ..Default::default() };
    let r = run_script_with(&d, "(define (spin) (turn-left) (spin)) (spin)", &cfg);
    assert_eq!(r.outcome, Outcome::TurnLimit);
    assert_eq!(r.turns, 50);
    assert_eq!(r.events.len(), 50);
}

#[test]
fn default_turn_limit_is_5000() {
    let d = dungeon();
    let r = run(&d, "(define (spin) (turn-left) (spin)) (spin)");
    assert_eq!(r.outcome, Outcome::TurnLimit);
    assert_eq!(r.turns, 5000);
}

#[test]
fn endless_blocked_moves_are_cut_off() {
    let d = dungeon();
    let r = run(
        &d,
        "(define (face-wall) (if (can-move?) (begin (turn-right) (face-wall)) 'ok))
         (face-wall)
         (define (spin) (move) (spin))
         (spin)",
    );
    assert!(matches!(r.outcome, Outcome::ScriptError(_)), "{:?}", r.outcome);
}

#[test]
fn script_errors_are_reported() {
    let d = dungeon();
    assert!(matches!(run(&d, "(no-such-function)").outcome, Outcome::ScriptError(_)));
    assert!(matches!(run(&d, "(follow-path '(up))").outcome, Outcome::ScriptError(_)));
    assert!(matches!(run(&d, "(follow-path 3)").outcome, Outcome::ScriptError(_)));
    assert!(matches!(run(&d, "(+ 1").outcome, Outcome::ScriptError(_)));
    // 何もせず終わった場合もエラー扱い。
    assert!(matches!(run(&d, "(facing)").outcome, Outcome::ScriptError(_)));
}

// ---------- 戦闘 ----------

#[test]
fn strong_party_wins_and_priest_heals() {
    use rand::SeedableRng;
    let d = dungeon();
    let mut w = world(&d);
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(0);
    w.party.members[0].hp = 10;
    let rep = crawl_dungeon::game::battle::fight(&mut w.party, 1, 1, &mut rng);
    assert_eq!(rep.result, BattleEnd::Won);
    assert!(rep.damage_dealt > 0);
    assert!(rep.healed > 0, "priest should heal after the battle");
    assert!(rep.mp_used > 0 || rep.rounds == 1);
}

#[test]
fn weak_party_is_wiped_or_flees() {
    use rand::SeedableRng;
    let d = dungeon();
    let mut wiped = 0;
    for s in 0..20 {
        let mut w = world(&d);
        // 生き残りは HP1 の1人だけ。先制攻撃で敵を全滅させられない。
        for m in &mut w.party.members {
            m.hp = 0;
            m.mp = 0;
        }
        w.party.members[3].hp = 1;
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(s);
        let rep = crawl_dungeon::game::battle::fight(&mut w.party, FLOOR_COUNT, 3, &mut rng);
        match rep.result {
            BattleEnd::Wiped => {
                wiped += 1;
                assert_eq!(w.party.alive_count(), 0);
            }
            BattleEnd::Fled => assert!(w.party.alive_count() > 0),
            BattleEnd::Won => {}
        }
    }
    assert!(wiped > 0);
}

#[test]
fn thieves_help_fleeing() {
    // 盗賊がいる方が逃走率が高い（設定値の関係で確認）。
    assert!(FLEE_BASE_PERCENT + FLEE_THIEF_BONUS_PERCENT <= FLEE_MAX_PERCENT);
    assert!(FLEE_THIEF_BONUS_PERCENT > 0);
}

#[test]
fn huge_coordinates_are_not_truncated() {
    let d = dungeon();
    // 4294967296 = 2^32。`as i32` だと 0 に丸められてしまう。
    let r = run(
        &d,
        "(assert! (equal? (known-tile 4294967296 4294967296) 'unknown))
         (assert! (equal? (visited? 4294967296 4294967296) #f))
         (assert! (equal? (path-to 4294967296 4294967296) #f))
         (assert! (equal? (known-tile -5 -5) 'unknown))
         (exit-dungeon)",
    );
    assert_eq!(r.outcome, Outcome::Exited(ExitKind::NoBook), "{r:?}");
}
