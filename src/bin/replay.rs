//! リプレイビューア（feature `viewer`）。
//!
//! 起動時に `generate_dungeon` と `run_script_with` を実行し、得られた `RunResult.events` を再生する。
//! 表示状態は `fold` で先頭から畳み込んで作る（逆再生用の差分は持たない）。

#[path = "replay/fold.rs"]
mod fold;

use bevy::prelude::*;
use crawl_dungeon::dungeon::{Dungeon, Pos, Tile, generate_dungeon};
use crawl_dungeon::game::consts::{INITIAL_PARTY, class_stats};
use crawl_dungeon::game::{Outcome, RunConfig, RunResult, run_script_with};

use fold::{ViewState, describe, fold};

const CELL: f32 = 16.0;
const WINDOW: Vec2 = Vec2::new(1040.0, 680.0);
const PANEL_W: f32 = 360.0;
const LOG_LINES: usize = 14;
/// 再生速度（イベント/秒）。
const SPEEDS: [f32; 7] = [2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0];
const DEFAULT_SPEED: usize = 3;

struct Args {
    seed: u64,
    game_seed: u64,
    script: String,
    exit_at_end: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut a = Args { seed: 0, game_seed: 0, script: "scripts/sample.scm".into(), exit_at_end: false };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = |name: &str| it.next().ok_or(format!("{name} needs a value"));
        match flag.as_str() {
            "--seed" => a.seed = value("--seed")?.parse().map_err(|e| format!("--seed: {e}"))?,
            "--game-seed" => a.game_seed = value("--game-seed")?.parse().map_err(|e| format!("--game-seed: {e}"))?,
            "--script" => a.script = value("--script")?,
            // 再生が最後まで行ったら終了する（動作確認用）。
            "--exit-at-end" => a.exit_at_end = true,
            _ => return Err(format!("unknown argument: {flag}\nusage: replay [--seed N] [--game-seed N] [--script PATH]")),
        }
    }
    Ok(a)
}

#[derive(Resource)]
struct Data {
    dungeon: Dungeon,
    result: RunResult,
}

#[derive(Resource)]
struct Playback {
    /// 適用済みのイベント数（0..=events.len()）。
    idx: usize,
    playing: bool,
    speed: usize,
    acc: f32,
    dirty: bool,
    reported: bool,
    exit_at_end: bool,
}

#[derive(Component)]
struct MapCell(Pos);
#[derive(Component)]
struct ItemMarker {
    floor: usize,
    pos: Pos,
}
#[derive(Component)]
struct PartyMarker;
#[derive(Component)]
struct PartyNose;
#[derive(Component)]
struct HeaderText;
#[derive(Component)]
struct LogText;
#[derive(Component)]
struct OutcomeText;
#[derive(Component)]
struct MemberLabel(usize);
#[derive(Component)]
struct HpFill(usize);
#[derive(Component)]
struct MpFill(usize);

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    let script = match std::fs::read_to_string(&args.script) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read {}: {e}", args.script);
            std::process::exit(1);
        }
    };
    let dungeon = generate_dungeon(args.seed);
    let result = run_script_with(&dungeon, &script, &RunConfig { game_seed: args.game_seed, ..Default::default() });
    println!("seed {} game-seed {}: {:?} in {} turns, {} events", args.seed, args.game_seed, result.outcome, result.turns, result.events.len());

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: format!("crawl-dungeon replay (seed {})", args.seed),
                resolution: WINDOW.into(),
                resizable: false,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.04, 0.04, 0.06)))
        .insert_resource(Data { dungeon, result })
        .insert_resource(Playback {
            idx: 0,
            playing: true,
            speed: DEFAULT_SPEED,
            acc: 0.0,
            dirty: true,
            reported: false,
            exit_at_end: args.exit_at_end,
        })
        .add_systems(Startup, setup)
        .add_systems(Update, (input, advance, render).chain())
        .run();
}

fn cell_world(floor_w: i32, floor_h: i32, p: Pos) -> Vec2 {
    let map_cx = -WINDOW.x / 2.0 + (WINDOW.x - PANEL_W) / 2.0;
    Vec2::new(
        map_cx + (p.x as f32 - floor_w as f32 / 2.0 + 0.5) * CELL,
        (floor_h as f32 / 2.0 - p.y as f32 - 0.5) * CELL,
    )
}

fn setup(mut commands: Commands, data: Res<Data>) {
    commands.spawn(Camera2d);
    let f = &data.dungeon.floors[0];
    for y in 0..f.height {
        for x in 0..f.width {
            let p = Pos::new(x, y);
            let w = cell_world(f.width, f.height, p);
            commands.spawn((
                Sprite::from_color(Color::BLACK, Vec2::splat(CELL - 1.0)),
                Transform::from_xyz(w.x, w.y, 0.0),
                MapCell(p),
            ));
        }
    }
    for fl in &data.dungeon.floors {
        for it in &fl.items {
            let w = cell_world(f.width, f.height, it.pos);
            commands.spawn((
                Sprite::from_color(Color::srgb(0.7, 0.3, 1.0), Vec2::splat(CELL * 0.55)),
                Transform::from_xyz(w.x, w.y, 1.0),
                Visibility::Hidden,
                ItemMarker { floor: fl.level, pos: it.pos },
            ));
        }
    }
    commands.spawn((
        Sprite::from_color(Color::srgb(1.0, 0.9, 0.2), Vec2::splat(CELL * 0.6)),
        Transform::from_xyz(0.0, 0.0, 2.0),
        PartyMarker,
    ));
    commands.spawn((
        Sprite::from_color(Color::srgb(1.0, 0.2, 0.2), Vec2::splat(CELL * 0.3)),
        Transform::from_xyz(0.0, 0.0, 3.0),
        PartyNose,
    ));

    let text = |size: f32| TextFont { font_size: size, ..default() };
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(PANEL_W),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                row_gap: Val::Px(4.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.08, 0.08, 0.12)),
        ))
        .with_children(|p| {
            p.spawn((Text::new(""), text(15.0), HeaderText));
            p.spawn((
                Text::new("Space: play/pause  Left/Right: step\nUp/Down: speed  Home/End: first/last"),
                text(11.0),
                TextColor(Color::srgb(0.55, 0.55, 0.65)),
            ));
            for (i, &(name, class)) in INITIAL_PARTY.iter().enumerate() {
                let (max_hp, max_mp) = class_stats(class);
                p.spawn((Text::new(format!("{name} ({})", class.symbol())), text(14.0), MemberLabel(i)));
                for (is_hp, max) in [(true, max_hp), (false, max_mp)] {
                    let (back, fill) = if is_hp {
                        (Color::srgb(0.25, 0.1, 0.1), Color::srgb(0.85, 0.2, 0.2))
                    } else {
                        (Color::srgb(0.1, 0.12, 0.28), Color::srgb(0.3, 0.45, 0.95))
                    };
                    let mut bar = p.spawn((
                        Node { width: Val::Percent(100.0), height: Val::Px(if is_hp { 8.0 } else { 5.0 }), ..default() },
                        BackgroundColor(if max == 0 { Color::NONE } else { back }),
                    ));
                    bar.with_children(|b| {
                        let node = Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
                        if is_hp {
                            b.spawn((node, BackgroundColor(fill), HpFill(i)));
                        } else {
                            b.spawn((node, BackgroundColor(fill), MpFill(i)));
                        }
                    });
                }
            }
            p.spawn((Text::new(""), text(12.0), TextColor(Color::srgb(0.8, 0.8, 0.85)), Node { margin: UiRect::top(Val::Px(10.0)), ..default() }, LogText));
            p.spawn((Text::new(""), text(26.0), TextColor(Color::srgb(1.0, 0.9, 0.4)), Node { margin: UiRect::top(Val::Px(8.0)), ..default() }, OutcomeText));
        });
}

fn input(keys: Res<ButtonInput<KeyCode>>, data: Res<Data>, mut pb: ResMut<Playback>) {
    let len = data.result.events.len();
    if keys.just_pressed(KeyCode::Space) {
        if pb.idx >= len {
            pb.idx = 0;
        }
        pb.playing = !pb.playing;
        pb.dirty = true;
    }
    if keys.just_pressed(KeyCode::ArrowRight) {
        pb.idx = (pb.idx + 1).min(len);
        pb.playing = false;
        pb.dirty = true;
    }
    if keys.just_pressed(KeyCode::ArrowLeft) {
        pb.idx = pb.idx.saturating_sub(1);
        pb.playing = false;
        pb.dirty = true;
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        pb.speed = (pb.speed + 1).min(SPEEDS.len() - 1);
        pb.dirty = true;
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        pb.speed = pb.speed.saturating_sub(1);
        pb.dirty = true;
    }
    if keys.just_pressed(KeyCode::Home) {
        pb.idx = 0;
        pb.dirty = true;
    }
    if keys.just_pressed(KeyCode::End) {
        pb.idx = len;
        pb.playing = false;
        pb.dirty = true;
    }
}

fn advance(time: Res<Time>, data: Res<Data>, mut pb: ResMut<Playback>) {
    let len = data.result.events.len();
    if !pb.playing {
        return;
    }
    pb.acc += time.delta_secs() * SPEEDS[pb.speed];
    let steps = pb.acc as usize;
    pb.acc -= steps as f32;
    if steps > 0 {
        pb.idx = (pb.idx + steps).min(len);
        pb.dirty = true;
    }
    if pb.idx >= len {
        pb.playing = false;
    }
}

fn tile_color(t: Tile, visited: bool) -> Color {
    let (r, g, b) = match t {
        Tile::Wall => (0.22, 0.24, 0.32),
        Tile::Floor => (0.55, 0.5, 0.4),
        Tile::Door => (0.6, 0.35, 0.15),
        Tile::StairsUp => (0.2, 0.7, 0.3),
        Tile::StairsDown => (0.85, 0.3, 0.2),
    };
    let k = if visited && t != Tile::Wall { 1.0 } else { 0.8 };
    Color::srgb(r * k, g * k, b * k)
}

fn outcome_label(o: &Outcome) -> String {
    // 既定フォントは日本語を持たないので英語で出す。
    match o {
        Outcome::Won => "WON".into(),
        Outcome::PartyWiped => "PARTY WIPED".into(),
        Outcome::TurnLimit => "TURN LIMIT".into(),
        Outcome::ScriptError(e) => format!("ERROR: {e}"),
        Outcome::Exited(_) => "EXITED (no book)".into(),
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn render(
    mut pb: ResMut<Playback>,
    data: Res<Data>,
    mut exit: EventWriter<AppExit>,
    mut cells: Query<(&MapCell, &mut Sprite), (Without<PartyMarker>, Without<PartyNose>, Without<ItemMarker>)>,
    mut items: Query<(&ItemMarker, &mut Visibility)>,
    mut marker: Query<&mut Transform, (With<PartyMarker>, Without<PartyNose>)>,
    mut nose: Query<&mut Transform, (With<PartyNose>, Without<PartyMarker>)>,
    mut texts: ParamSet<(
        Query<&mut Text, With<HeaderText>>,
        Query<&mut Text, With<LogText>>,
        Query<&mut Text, With<OutcomeText>>,
        Query<(&MemberLabel, &mut Text, &mut TextColor)>,
    )>,
    mut hp: Query<(&HpFill, &mut Node), Without<MpFill>>,
    mut mp: Query<(&MpFill, &mut Node), Without<HpFill>>,
) {
    if !pb.dirty {
        return;
    }
    pb.dirty = false;
    let events = &data.result.events;
    let len = events.len();
    let s: ViewState = fold(&data.dungeon, events, pb.idx);
    let floor = &data.dungeon.floors[s.floor - 1];
    let mem = &s.memory[s.floor - 1];

    for (c, mut sprite) in &mut cells {
        sprite.color = match mem.known(c.0) {
            Some(t) => tile_color(t, mem.visited(c.0)),
            None => Color::srgb(0.03, 0.03, 0.04),
        };
    }
    for (m, mut vis) in &mut items {
        let shown = m.floor == s.floor && mem.known(m.pos).is_some() && s.items[m.floor - 1].iter().any(|i| i.pos == m.pos);
        *vis = if shown { Visibility::Inherited } else { Visibility::Hidden };
    }
    let w = cell_world(floor.width, floor.height, s.pos);
    if let Ok(mut t) = marker.get_single_mut() {
        t.translation.x = w.x;
        t.translation.y = w.y;
    }
    if let Ok(mut t) = nose.get_single_mut() {
        let (dx, dy) = s.facing.delta();
        t.translation.x = w.x + dx as f32 * CELL * 0.4;
        t.translation.y = w.y - dy as f32 * CELL * 0.4;
    }

    let state = if pb.idx >= len { "END" } else if pb.playing { "PLAY" } else { "PAUSE" };
    if let Ok(mut t) = texts.p0().get_single_mut() {
        **t = format!(
            "{state}  turn {}  B{}F  ({},{})\nevent {}/{}  speed {}/s",
            s.turn, s.floor, s.pos.x, s.pos.y, pb.idx, len, SPEEDS[pb.speed]
        );
    }
    if let Ok(mut t) = texts.p1().get_single_mut() {
        **t = events[pb.idx.saturating_sub(LOG_LINES)..pb.idx].iter().map(describe).collect::<Vec<_>>().join("\n");
    }
    let at_end = pb.idx >= len;
    if let Ok(mut t) = texts.p2().get_single_mut() {
        **t = if at_end { outcome_label(&data.result.outcome) } else { String::new() };
    }
    for (l, mut t, mut c) in &mut texts.p3() {
        let (name, class) = INITIAL_PARTY[l.0];
        let m = s.members[l.0];
        let (max_hp, max_mp) = class_stats(class);
        **t = format!("{name} ({})  HP {}/{max_hp}  MP {}/{max_mp}", class.symbol(), m.hp, m.mp);
        *c = TextColor(if m.hp > 0 { Color::WHITE } else { Color::srgb(0.5, 0.5, 0.5) });
    }
    for (f, mut n) in &mut hp {
        let max = class_stats(INITIAL_PARTY[f.0].1).0.max(1);
        n.width = Val::Percent(100.0 * s.members[f.0].hp as f32 / max as f32);
    }
    for (f, mut n) in &mut mp {
        let max = class_stats(INITIAL_PARTY[f.0].1).1.max(1);
        n.width = Val::Percent(100.0 * s.members[f.0].mp as f32 / max as f32);
    }

    if at_end && !pb.reported {
        pb.reported = true;
        println!("finished: {} ({:?}), turn {}", outcome_label(&data.result.outcome), data.result.outcome, data.result.turns);
        if pb.exit_at_end {
            exit.send(AppExit::Success);
        }
    } else if !at_end {
        pb.reported = false;
    }
}
