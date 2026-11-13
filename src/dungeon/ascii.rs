use super::types::{Floor, ItemKind, Pos, Tile};

/// デバッグ用: 壁 `#`、床 `.`、扉 `+`、上り `<`、下り `>`、魔導書 `B`。
pub fn to_ascii(floor: &Floor) -> String {
    let mut out = String::with_capacity(((floor.width + 1) * floor.height) as usize);
    for y in 0..floor.height {
        for x in 0..floor.width {
            let p = Pos::new(x, y);
            let c = if floor.items.iter().any(|i| i.pos == p && i.kind == ItemKind::MagicBook) {
                'B'
            } else {
                match floor.tiles[floor.index(p)] {
                    Tile::Wall => '#',
                    Tile::Floor => '.',
                    Tile::Door => '+',
                    Tile::StairsUp => '<',
                    Tile::StairsDown => '>',
                }
            };
            out.push(c);
        }
        out.push('\n');
    }
    out
}
