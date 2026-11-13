//! ゲーム状態と Scheme スクリプトAPI。Bevy には依存しない。

pub mod battle;
pub mod consts;
mod event;
mod memory;
mod script;
mod types;
mod world;

pub use event::{BattleEnd, ExitKind, GameEvent, MemberSnapshot, Outcome, RunResult};
pub use memory::FloorMemory;
pub use script::{RunConfig, run_script, run_script_with};
pub use types::{Class, Dir, Member, Party};
pub use world::{FollowResult, Halt, MoveResult, RestResult, World};
