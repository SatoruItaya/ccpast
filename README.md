# ccpast

`ccpast` is a single-binary terminal UI for browsing, resuming, and trash-deleting Claude Code session history across all working directories at once. It is intentionally **flat**: every session from every project is merged into one list sorted by most recent activity, so you can jump straight to a session without remembering which repository it belonged to.

[日本語版 / Japanese](./README.ja.md)

## Highlights

- **Flat cross-cutting list as the primary view.** No project drill-down required.
- **Single binary, no external dependencies.** No `fzf`, no Python. The only external process invoked is `claude` itself.
- **Trash-based deletion.** Sessions move to `~/.claude/.trash/` with an `index.jsonl` audit line; restore manually with `mv`.

## Install

```bash
cargo install --path .
# or
cargo build --release
# produces ./target/release/ccpast
```

## Usage

```bash
ccpast           # interactive TUI
ccpast --list    # plain listing to stdout (auto when stdout is not a TTY)
ccpast --help
```

### Keybindings (List screen)

| Key            | Action                                |
|----------------|---------------------------------------|
| `↑`/`↓`, `j`/`k` | Move cursor                         |
| `Enter`        | Open the Reader (full conversation)   |
| `r`            | Resume the session (`claude --resume`)|
| `f`            | Fork-resume (adds `--fork-session`)   |
| `d`            | Delete with confirmation (trash move) |
| `/`            | Incremental filter on title and cwd   |
| `p`            | Toggle preview pane                   |
| `q` / `Esc`    | Quit                                  |

### Reader screen

| Key                  | Action            |
|----------------------|-------------------|
| `↑`/`↓`, `j`/`k`     | Scroll one line   |
| `PageUp`/`PageDown`  | Scroll one page   |
| `r`                  | Resume            |
| `q` / `Esc`          | Back to list      |

## Deletion and recovery

Pressing `d` and confirming moves the JSONL file from `~/.claude/projects/<encoded-cwd>/<id>.jsonl` to `~/.claude/.trash/<timestamp>-<id>.jsonl` and appends a line to `~/.claude/.trash/index.jsonl`:

```json
{"trashed_path":"...","original_path":"...","session_id":"...","deleted_at":"..."}
```

To restore a session, look up its `original_path` in `index.jsonl` and move it back:

```bash
mv ~/.claude/.trash/<timestamp>-<id>.jsonl <original_path>
```

`ccpast` does **not** touch `~/.claude/history.jsonl`. Some prompt-history residue from the deleted session may remain there.

## Credit

The JSONL schema understanding was informed by the spec notes shipped with [`choplin/cclog`](https://github.com/choplin/cclog). No code or text was imported; the parser here is an independent implementation.

## License

MIT — see [LICENSE](./LICENSE).
