# crawl-dungeon
Scheme でプログラミングしてダンジョンを自動で攻略するゲームだったらいいな〜と思います。

## リプレイビューア

`RunResult.events` を再生して冒険を見られる Bevy 製ビューア。feature `viewer` のときだけビルドされる（lib と `cargo test` は Bevy に依存しない）。

```bash
cargo run --features viewer --bin replay -- --seed 3 [--game-seed 0] [--script scripts/sample.scm]
```

操作: Space 再生/一時停止、←/→ 1イベント戻る/進む、↑/↓ 再生速度、Home/End 最初/最後へ。
