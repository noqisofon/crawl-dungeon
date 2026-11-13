//! 戦闘（v1 は最小限）。1回の move/rest の中で完結する。

use rand::Rng;

use super::consts::*;
use super::event::BattleEnd;
use super::types::{Class, Party};

struct Enemy {
    hp: i32,
    atk: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BattleReport {
    pub result: BattleEnd,
    pub enemies: usize,
    pub rounds: u32,
    pub damage_dealt: i32,
    pub damage_taken: i32,
    pub mp_used: i32,
    pub members_lost: usize,
    pub healed: i32,
}

/// 階 `level` で遭遇する敵の数（遭遇イベント用に先に決める）。
pub fn roll_enemy_count(rng: &mut impl Rng) -> usize {
    rng.random_range(ENEMY_COUNT_MIN..=ENEMY_COUNT_MAX)
}

fn roll(rng: &mut impl Rng, base: i32) -> i32 {
    base + rng.random_range(0..=DAMAGE_VARIANCE)
}

fn first_alive(enemies: &[Enemy]) -> Option<usize> {
    enemies.iter().position(|e| e.hp > 0)
}

pub fn fight(party: &mut Party, level: usize, enemy_count: usize, rng: &mut impl Rng) -> BattleReport {
    let depth = level as i32 - 1;
    let mut enemies: Vec<Enemy> = (0..enemy_count)
        .map(|_| Enemy {
            hp: ENEMY_HP_BASE + ENEMY_HP_PER_FLOOR * depth,
            atk: ENEMY_ATK_BASE + ENEMY_ATK_PER_FLOOR * depth,
        })
        .collect();
    let mut rep = BattleReport {
        result: BattleEnd::Fled,
        enemies: enemy_count,
        rounds: 0,
        damage_dealt: 0,
        damage_taken: 0,
        mp_used: 0,
        members_lost: 0,
        healed: 0,
    };

    while rep.rounds < MAX_ROUNDS {
        rep.rounds += 1;

        // 味方の攻撃（先頭の生存敵に集中攻撃）。
        for i in 0..party.members.len() {
            let Some(t) = first_alive(&enemies) else { break };
            let m = &mut party.members[i];
            if !m.is_alive() {
                continue;
            }
            let base = match m.class {
                Class::Fighter => FIGHTER_ATK,
                Class::Thief => THIEF_ATK,
                Class::Priest => PRIEST_ATK,
                Class::Mage if m.mp >= MAGE_SPELL_MP => {
                    m.mp -= MAGE_SPELL_MP;
                    rep.mp_used += MAGE_SPELL_MP;
                    MAGE_SPELL_DMG
                }
                Class::Mage => MAGE_STAFF_ATK,
            };
            let dmg = roll(rng, base);
            enemies[t].hp -= dmg;
            rep.damage_dealt += dmg;
        }
        if first_alive(&enemies).is_none() {
            rep.result = BattleEnd::Won;
            break;
        }

        // 敵の攻撃（生存メンバーからランダムに狙う）。
        for e in enemies.iter().filter(|e| e.hp > 0) {
            let alive: Vec<usize> = (0..party.members.len()).filter(|&i| party.members[i].is_alive()).collect();
            if alive.is_empty() {
                break;
            }
            let m = &mut party.members[alive[rng.random_range(0..alive.len())]];
            let dmg = roll(rng, e.atk).min(m.hp);
            m.hp -= dmg;
            rep.damage_taken += dmg;
            if m.hp == 0 {
                rep.members_lost += 1;
            }
        }
        if party.alive_count() == 0 {
            rep.result = BattleEnd::Wiped;
            break;
        }

        // 劣勢なら逃走を試みる。盗賊が生きているほど成功しやすい。
        if party.hp_ratio() < FLEE_HP_RATIO {
            let thieves = party.members.iter().filter(|m| m.is_alive() && m.class == Class::Thief).count() as u32;
            let pct = (FLEE_BASE_PERCENT + FLEE_THIEF_BONUS_PERCENT * thieves).min(FLEE_MAX_PERCENT);
            if rng.random_range(0..100) < pct {
                rep.result = BattleEnd::Fled;
                break;
            }
        }
    }

    if rep.result == BattleEnd::Won {
        let priests = party.members.iter().filter(|m| m.is_alive() && m.class == Class::Priest).count() as i32;
        for _ in 0..priests {
            for m in party.members.iter_mut().filter(|m| m.is_alive()) {
                let heal = PRIEST_HEAL_AFTER_BATTLE.min(m.max_hp - m.hp);
                m.hp += heal;
                rep.healed += heal;
            }
        }
    }
    rep
}
