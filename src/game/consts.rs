//! ゲームバランス用の定数。調整はここだけで行う。

use super::types::Class;

// ---- 実行 ----
pub const DEFAULT_TURN_LIMIT: u32 = 5000;
/// ターンを消費しない行動（壁にぶつかる）が連続できる上限。無限ループ対策。
pub const MAX_IDLE_ACTIONS: u32 = 1000;
/// ホスト関数の総呼び出し回数の上限。無限ループ対策。
pub const MAX_HOST_CALLS: u64 = 5_000_000;

// ---- ターンコスト ----
pub const MOVE_COST: u32 = 1;
pub const TURN_COST: u32 = 1;
pub const REST_COST: u32 = 10;
pub const STAIRS_COST: u32 = 1;
pub const PICKUP_COST: u32 = 1;
pub const EXIT_COST: u32 = 1;

// ---- 初期パーティ ----
pub const INITIAL_PARTY: [(&str, Class); 6] = [
    ("Gorm", Class::Fighter),
    ("Brunhild", Class::Fighter),
    ("Roland", Class::Fighter),
    ("Silk", Class::Thief),
    ("Merlin", Class::Mage),
    ("Lyra", Class::Priest),
];

/// (最大HP, 最大MP)
pub const fn class_stats(class: Class) -> (i32, i32) {
    match class {
        Class::Fighter => (60, 0),
        Class::Thief => (40, 0),
        Class::Mage => (28, 20),
        Class::Priest => (36, 10),
    }
}

// ---- 遭遇 ----
/// 移動1回あたりの遭遇確率(%)。添字 = 階 - 1。
pub const MOVE_ENCOUNTER_PERCENT: [u32; 5] = [3, 5, 7, 9, 11];
/// 野営1回あたりの遭遇確率(%)。
pub const REST_ENCOUNTER_PERCENT: [u32; 5] = [10, 15, 20, 25, 30];

// ---- 野営 ----
/// 野営で回復する最大HP/MPに対する割合(%)。倒れたメンバーは復活しない。
pub const REST_HEAL_PERCENT: i32 = 50;

// ---- 敵 ----
pub const ENEMY_COUNT_MIN: usize = 1;
pub const ENEMY_COUNT_MAX: usize = 3;
pub const ENEMY_HP_BASE: i32 = 14;
pub const ENEMY_HP_PER_FLOOR: i32 = 6;
pub const ENEMY_ATK_BASE: i32 = 5;
pub const ENEMY_ATK_PER_FLOOR: i32 = 1;

// ---- 戦闘 ----
/// ダメージのブレ（0..=この値を加算）。
pub const DAMAGE_VARIANCE: i32 = 3;
pub const FIGHTER_ATK: i32 = 12;
pub const THIEF_ATK: i32 = 8;
pub const PRIEST_ATK: i32 = 4;
pub const MAGE_STAFF_ATK: i32 = 3;
pub const MAGE_SPELL_DMG: i32 = 30;
pub const MAGE_SPELL_MP: i32 = 4;
/// 戦闘後に僧侶1人が生存メンバー全員へ与える回復量。
pub const PRIEST_HEAL_AFTER_BATTLE: i32 = 6;
/// パーティHP割合がこれを下回ると各ラウンド末に逃走を試みる。
pub const FLEE_HP_RATIO: f64 = 0.35;
pub const FLEE_BASE_PERCENT: u32 = 10;
/// 生存している盗賊1人あたりの逃走率加算(%)。
pub const FLEE_THIEF_BONUS_PERCENT: u32 = 30;
pub const FLEE_MAX_PERCENT: u32 = 90;
/// 1回の戦闘の最大ラウンド数（超えたら双方引いたものとして逃走扱い）。
pub const MAX_ROUNDS: u32 = 100;
