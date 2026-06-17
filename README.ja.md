# ccpast

`ccpast` は Claude Code のセッション履歴を作業ディレクトリをまたいで一枚のフラットな一覧として閲覧・再開・ゴミ箱削除できる、Rust 製の単一バイナリ TUI です。プロジェクトを思い出せなくても、最近順に並んだリストから直接そのセッションへ飛べます。

[English / 英語版](./README.md)

## 特徴

- **横断フラット一覧が主役**。プロジェクトでまず絞る必要なし。
- **単一バイナリ・外部依存なし**。`fzf` も Python も不要。外部プロセス起動は `claude` 本体だけ。
- **ゴミ箱方式の削除**。`~/.claude/.trash/` に移動し、`index.jsonl` に元パスを記録。`mv` で復元できる。

## インストール

```bash
cargo install --path .
# あるいは
cargo build --release
# ./target/release/ccpast が生成される
```

## 使い方

```bash
ccpast           # インタラクティブ TUI
ccpast --list    # プレーン出力（標準出力が TTY でないときも自動）
ccpast --help
```

### キーバインド（List 画面）

| キー             | 動作                                |
|----------------|--------------------------------------|
| `↑`/`↓`, `j`/`k` | カーソル移動                         |
| `Enter`        | Reader（全文）を開く                  |
| `r`            | 再開 (`claude --resume`)             |
| `f`            | フォーク再開 (`--fork-session`)      |
| `d`            | 削除（確認後にゴミ箱へ）              |
| `/`            | title / cwd の部分一致でインクリメンタル絞り込み |
| `p`            | プレビューペインのトグル              |
| `q` / `Esc`    | 終了                                |

### Reader 画面

| キー                  | 動作                |
|---------------------|----------------------|
| `↑`/`↓`, `j`/`k`     | 1行スクロール        |
| `PageUp`/`PageDown` | 1ページスクロール     |
| `r`                 | 再開                 |
| `q` / `Esc`         | 一覧に戻る            |

## 削除と復元

`d` → `y` で確認すると、`~/.claude/projects/<encoded-cwd>/<id>.jsonl` を `~/.claude/.trash/<timestamp>-<id>.jsonl` に**移動**し、`~/.claude/.trash/index.jsonl` に1行追記します:

```json
{"trashed_path":"...","original_path":"...","session_id":"...","deleted_at":"..."}
```

復元したいときは `index.jsonl` で `original_path` を確認し、`mv` で戻してください:

```bash
mv ~/.claude/.trash/<timestamp>-<id>.jsonl <original_path>
```

`ccpast` は `~/.claude/history.jsonl` を**触りません**。削除したセッションのプロンプト履歴の痕跡が残ることがあります。

## クレジット

JSONL 形式の理解は [`choplin/cclog`](https://github.com/choplin/cclog) 同梱の仕様メモを参考にしました。コードや文書は取り込んでおらず、このリポジトリのパーサは独自実装です。

## ライセンス

MIT — [LICENSE](./LICENSE) を参照してください。
