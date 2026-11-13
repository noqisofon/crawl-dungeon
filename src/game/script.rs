//! steel への登録と `run_script`。
//!
//! 各ホスト関数は `Arc<Mutex<World>>` を clone して持つクロージャ。ゲームが終了
//! （勝利・全滅・ターン上限など）すると `World.status` に理由が入り、以降の
//! ホスト関数呼び出しは `Err` を返してスクリプトを中断する。

use std::sync::{Arc, Mutex};

use steel::rerrs::{ErrorKind, SteelErr};
use steel::rvals::SteelVal;
use steel::steel_vm::engine::Engine;
use steel::steel_vm::register_fn::RegisterFn;

use crate::dungeon::{Dungeon, ItemKind, Pos, Tile};

use super::consts::DEFAULT_TURN_LIMIT;
use super::event::{Outcome, RunResult};
use super::types::Dir;
use super::world::{FollowResult, MoveResult, RestResult, World};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunConfig {
    /// 戦闘の乱数の種。ダンジョンのシードとは独立。
    pub game_seed: u64,
    pub turn_limit: u32,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self { game_seed: 0, turn_limit: DEFAULT_TURN_LIMIT }
    }
}

type Shared = Arc<Mutex<World>>;
type Val = Result<SteelVal, SteelErr>;

fn sym(s: &str) -> SteelVal {
    SteelVal::SymbolV(s.into())
}

fn pos_val(p: Pos) -> SteelVal {
    SteelVal::ListV(vec![SteelVal::IntV(p.x as isize), SteelVal::IntV(p.y as isize)].into())
}

fn tile_sym(t: Tile) -> SteelVal {
    sym(match t {
        Tile::Wall => "wall",
        Tile::Floor => "floor",
        Tile::Door => "door",
        Tile::StairsUp => "stairs-up",
        Tile::StairsDown => "stairs-down",
    })
}

/// スクリプトから来た座標を `Pos` にする。`i32` に収まらなければ `None`（丸めない）。
fn to_pos(x: isize, y: isize) -> Option<Pos> {
    Some(Pos::new(i32::try_from(x).ok()?, i32::try_from(y).ok()?))
}

fn abort(msg: &str) -> SteelErr {
    SteelErr::new(ErrorKind::Generic, msg.to_string())
}

/// 終了済みチェック付きで `World` を使う。
fn with<R>(w: &Shared, f: impl FnOnce(&mut World) -> Result<R, super::world::Halt>) -> Result<R, SteelErr> {
    let mut w = w.lock().expect("world lock");
    w.guard().and_then(|_| f(&mut w)).map_err(|_| abort("game halted"))
}

fn register(e: &mut Engine, w: &Shared) {
    // ---- 行動 ----
    let s = w.clone();
    e.register_fn("move", move || -> Val {
        with(&s, |w| w.do_move()).map(|r| {
            sym(match r {
                MoveResult::Moved => "moved",
                MoveResult::Blocked => "blocked",
                MoveResult::BattleWon => "battle-won",
                MoveResult::PartyWiped => "party-wiped",
            })
        })
    });
    let s = w.clone();
    e.register_fn("turn-left", move || -> Val { with(&s, |w| w.turn_left()).map(|_| SteelVal::Void) });
    let s = w.clone();
    e.register_fn("turn-right", move || -> Val { with(&s, |w| w.turn_right()).map(|_| SteelVal::Void) });
    let s = w.clone();
    e.register_fn("rest", move || -> Val {
        with(&s, |w| w.rest()).map(|r| {
            sym(match r {
                RestResult::Rested => "rested",
                RestResult::BattleWon => "battle-won",
                RestResult::PartyWiped => "party-wiped",
            })
        })
    });
    let s = w.clone();
    e.register_fn("descend", move || -> Val { with(&s, |w| w.descend()).map(SteelVal::BoolV) });
    let s = w.clone();
    e.register_fn("ascend", move || -> Val { with(&s, |w| w.ascend()).map(SteelVal::BoolV) });
    let s = w.clone();
    e.register_fn("pickup", move || -> Val { with(&s, |w| w.pickup()).map(SteelVal::BoolV) });
    let s = w.clone();
    e.register_fn("exit-dungeon", move || -> Val { with(&s, |w| w.exit_dungeon()).map(SteelVal::BoolV) });

    // ---- 状況取得 ----
    let s = w.clone();
    e.register_fn("facing", move || -> Val { with(&s, |w| Ok(sym(w.party.facing.symbol()))) });
    let s = w.clone();
    e.register_fn("position", move || -> Val { with(&s, |w| Ok(pos_val(w.party.pos))) });
    let s = w.clone();
    e.register_fn("floor-level", move || -> Val { with(&s, |w| Ok(SteelVal::IntV(w.party.floor as isize))) });
    let s = w.clone();
    e.register_fn("look", move || -> Val { with(&s, |w| Ok(tile_sym(w.look()))) });
    let s = w.clone();
    e.register_fn("can-move?", move || -> Val { with(&s, |w| Ok(SteelVal::BoolV(w.look().is_walkable()))) });
    let s = w.clone();
    e.register_fn("tile-here", move || -> Val { with(&s, |w| Ok(tile_sym(w.tile_here()))) });
    let s = w.clone();
    e.register_fn("item-here", move || -> Val {
        with(&s, |w| {
            Ok(match w.item_here() {
                Some(ItemKind::MagicBook) => sym("magic-book"),
                None => SteelVal::BoolV(false),
            })
        })
    });
    let s = w.clone();
    e.register_fn("has-item?", move |item: SteelVal| -> Val {
        with(&s, |w| {
            let has = matches!(&item, SteelVal::SymbolV(n) if n.as_str() == "magic-book")
                && w.party.has_item(ItemKind::MagicBook);
            Ok(SteelVal::BoolV(has))
        })
    });
    let s = w.clone();
    e.register_fn("party-hp-ratio", move || -> Val { with(&s, |w| Ok(SteelVal::NumV(w.party.hp_ratio()))) });
    let s = w.clone();
    e.register_fn("alive-count", move || -> Val {
        with(&s, |w| Ok(SteelVal::IntV(w.party.alive_count() as isize)))
    });
    // メンバー番号が範囲外なら #f。
    let s = w.clone();
    e.register_fn("member-hp", move |i: isize| -> Val {
        with(&s, |w| {
            Ok(usize::try_from(i)
                .ok()
                .and_then(|i| w.party.members.get(i))
                .map_or(SteelVal::BoolV(false), |m| SteelVal::IntV(m.hp as isize)))
        })
    });
    let s = w.clone();
    e.register_fn("member-class", move |i: isize| -> Val {
        with(&s, |w| {
            Ok(usize::try_from(i)
                .ok()
                .and_then(|i| w.party.members.get(i))
                .map_or(SteelVal::BoolV(false), |m| sym(m.class.symbol())))
        })
    });

    // ---- マップ記憶 ----
    let s = w.clone();
    e.register_fn("known-tile", move |x: isize, y: isize| -> Val {
        with(&s, |w| {
            Ok(match to_pos(x, y).and_then(|p| w.known_tile(p)) {
                Some(t) => tile_sym(t),
                None => sym("unknown"),
            })
        })
    });
    let s = w.clone();
    e.register_fn("visited?", move |x: isize, y: isize| -> Val {
        with(&s, |w| Ok(SteelVal::BoolV(to_pos(x, y).is_some_and(|p| w.visited(p)))))
    });
    let s = w.clone();
    e.register_fn("frontier", move || -> Val {
        with(&s, |w| Ok(SteelVal::ListV(w.frontier().into_iter().map(pos_val).collect::<Vec<_>>().into())))
    });
    let s = w.clone();
    e.register_fn("path-to", move |x: isize, y: isize| -> Val {
        with(&s, |w| {
            Ok(match to_pos(x, y).and_then(|p| w.path_to(p)) {
                Some(path) => SteelVal::ListV(path.into_iter().map(|d| sym(d.symbol())).collect::<Vec<_>>().into()),
                None => SteelVal::BoolV(false),
            })
        })
    });
    let s = w.clone();
    e.register_fn("follow-path", move |path: SteelVal| -> Val {
        let SteelVal::ListV(list) = path else { return Err(abort("follow-path: expected a list of directions")) };
        let mut dirs = Vec::new();
        for v in list.iter() {
            match v {
                SteelVal::SymbolV(n) if Dir::from_symbol(n.as_str()).is_some() => {
                    dirs.push(Dir::from_symbol(n.as_str()).unwrap())
                }
                _ => return Err(abort("follow-path: invalid direction in list")),
            }
        }
        with(&s, |w| w.follow_path(&dirs)).map(|r| {
            sym(match r {
                FollowResult::Arrived => "arrived",
                FollowResult::Blocked => "blocked",
                FollowResult::BattleWon => "battle-won",
                FollowResult::PartyWiped => "party-wiped",
            })
        })
    });
    let s = w.clone();
    e.register_fn("entrance-pos", move || -> Val { with(&s, |w| Ok(pos_val(w.entrance_pos()))) });
}

/// デフォルト設定（ゲームシード 0、ターン上限 5000）でスクリプトを実行する。
pub fn run_script(dungeon: &Dungeon, script: &str) -> RunResult {
    run_script_with(dungeon, script, &RunConfig::default())
}

/// 同じ（ダンジョン, `config.game_seed`, スクリプト）なら結果は完全に再現される。
pub fn run_script_with(dungeon: &Dungeon, script: &str, config: &RunConfig) -> RunResult {
    let world: Shared = Arc::new(Mutex::new(World::new(dungeon, config.game_seed, config.turn_limit)));
    let mut engine = Engine::new();
    register(&mut engine, &world);

    let run = engine.compile_and_run_raw_program(script.to_string());

    let mut w = world.lock().expect("world lock");
    let outcome = match (w.status.clone(), run) {
        (Some(outcome), _) => outcome,
        (None, Err(e)) => Outcome::ScriptError(e.to_string()),
        (None, Ok(_)) => Outcome::ScriptError("script ended without exiting the dungeon".into()),
    };
    RunResult { outcome, turns: w.party.turn, events: std::mem::take(&mut w.events) }
}
