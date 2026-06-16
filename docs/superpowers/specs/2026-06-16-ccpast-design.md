# ccpast — Design Document

- Date: 2026-06-16
- Status: v1 design approved (pre-implementation)

## 1. Purpose and differentiation

`ccpast` is a single-binary terminal UI written in Rust for browsing, resuming, and deleting Claude Code session history. The flagship view is **one flat list across all working directories**, sorted by most recent activity.

It is intentionally different from existing tools (e.g. `choplin/cclog`) in three ways:

1. **Flat cross-cutting list is the main view.** Sessions from every project are merged into a single list. A user does not need to remember which repository a session belonged to.
2. **Single binary with no external dependencies.** No `fzf`, no Python. The only external process invoked is `claude` (for resume).
3. **Built-in trash deletion.** Unwanted sessions move to `~/.claude/.trash/`; an `index.jsonl` keeps the original path so a manual restore is straightforward.

## 2. Tech stack and constraints

- Rust (single binary; package name = binary name = `ccpast`)
- `ratatui` + `crossterm` for TUI
- `serde_json` (+ `serde` derive where useful)
- `chrono` for timestamps
- `dirs` to resolve `~/.claude`
- `anyhow` for errors

No fuzzy matcher in v1. The search layer must stay simple enough to swap later (e.g. `nucleo-matcher`).

## 3. Data source and verified schema

### Layout

- Session files live at `~/.claude/projects/<encoded-cwd>/<session-id>.jsonl` (one JSON object per line).
- The directory name is `path-with-slashes-replaced-by-dashes`. **Do not decode it back.** Read `cwd` from inside the JSONL (first non-null `cwd` field encountered).
- **Do not read or write `~/.claude/history.jsonl`.** It may not cover every session and is unfit as a list source. v1 leaves it alone on delete; the README must call this out.

### Record types observed on disk

Records are distinguished by `type`. Observed: `user`, `assistant`, `summary`, `attachment`, `mode`, `permission-mode`, `last-prompt`, and others. Unknown types are skipped silently.

#### user / assistant

```
{
  "type": "user" | "assistant",
  "message": { "role": "...", "content": <string | array | missing> },
  "uuid": "...",
  "timestamp": "2026-01-01T00:00:00.000Z",
  "cwd": "/abs/path",
  "sessionId": "<uuid>",
  ...
}
```

- `message.content` is **string**, **array**, or **missing**.
- Array block types observed: `text`, `thinking`, `tool_use`, `tool_result`.
- `tool_result.content` is a string.
- Some `user` records have only `{ role: "user" }` with no content (scaffolding for a later turn). Must be null-safe.

#### summary

```
{"type":"summary","summary":"<string>","leafUuid":"..."}
```

No `cwd` / `sessionId` / `timestamp`. The `summary` text was observed to contain noise such as `API Error: 404 ...`. **v1 ignores `summary` for title selection.**

#### others

`last-prompt`, `mode`, `permission-mode`, etc. usually carry null `cwd` / `timestamp`. Skip during metadata extraction.

### Per-session data model

Lightweight metadata (built for every session at startup):

| Field | Source |
|---|---|
| `session_id` | filename stem |
| `path` | absolute path of the JSONL file |
| `cwd` | first non-null `cwd` field in the file |
| `cwd_exists` | filesystem check |
| `last_activity` | max `timestamp` in the file; fall back to file mtime |
| `title` | first human user utterance (rules below); `(no title)` if none |
| `message_count` | number of `user`/`assistant` rows |

Title extraction:
- Walk `type:"user"` rows. If `message.content` is a string, use it; if it is an array, use the first `type:"text"` block's `text`.
- Skip user rows whose content is only `tool_result` blocks.
- Collapse newlines into spaces.
- No candidate → `(no title)`.

## 4. Confirmed design decisions

1. **Title source**: always the first user utterance. v1 ignores `summary`.
2. **Project filter UI**: not in v1. The `/` substring search (case-insensitive, over `cwd` and `title`) covers the use case.
3. **Preview pane**: auto on/off by terminal-width threshold (e.g. < 100 cols hides it). `p` toggles it manually.
4. **Trash scope**: physical move + `index.jsonl` append only. Manual restore via `mv`. `history.jsonl` untouched.
5. **Scan strategy**: two-pass. Startup extracts lightweight metadata only (early-break per file once required fields are filled). Reader re-reads the whole file when opened.

## 5. Architecture and module layout

```
src/
├── main.rs        # CLI flags (--help, --list, --version) and dispatch
├── scan.rs        # Enumerate ~/.claude/projects/**/*.jsonl and extract lightweight metadata
├── parser.rs      # Low-level JSONL line → typed struct
├── session.rs     # SessionMeta / SessionFull and their construction
├── reader.rs      # Load one session's full body into a sequence of turns
├── trash.rs       # Move to ~/.claude/.trash/ and append index.jsonl
├── resume.rs      # Spawn `claude --resume`
├── ui/
│   ├── mod.rs     # App state, event loop, ratatui draw hub
│   ├── list.rs    # List screen
│   ├── reader.rs  # Reader screen
│   └── confirm.rs # Delete confirmation modal
└── util.rs        # Width-aware truncation, ISO8601 formatting, etc.
```

Avoid premature abstraction. Each module should be answerable on "what does it do / how do you use it / what does it depend on?" without leaking internals.

## 6. Data flow

### Startup → List screen

```
main
 └─ scan::list_session_files(~/.claude/projects/)   → Vec<PathBuf>
      └─ extract_meta(path)
           └─ stream lines; early-break once required fields are filled
                ├─ cwd: first non-null
                ├─ first_user_text
                ├─ first_timestamp / last_timestamp
                └─ message_count
      → Vec<SessionMeta>  (sorted by last_activity desc)
 └─ ui::run(sessions)
```

### Opening Reader

```
ui::list, Enter pressed
 └─ reader::load_full(path) → Vec<Turn>
      └─ parse every line, materialize user/assistant turns
           ├─ tool_use   → "[tool: <name>]"
           ├─ tool_result → "[tool result]"
           └─ thinking   → dropped in v1
 └─ ui::reader draws the turns
```

### `/` filter

Re-derive filtered indices on every keystroke:

```
sessions.iter().enumerate()
  .filter(|(_, s)| ci_contains(s.title, q) || ci_contains(s.cwd, q))
  .collect()
```

### `d` delete

```
confirm modal → y
 └─ trash::move_to_trash(meta)
      ├─ mkdir -p ~/.claude/.trash/
      ├─ fs::rename(path, trashed_path)
      │     (fall back to copy + remove if EXDEV)
      └─ append a line to index.jsonl
 └─ remove from in-memory list; redraw
```

### `r` / `f` resume

```
ui::teardown()  (leave raw mode, alt screen, restore cursor)
 └─ resume::spawn(meta, fork)
      ├─ verify cwd_exists
      ├─ Command::new("claude").current_dir(cwd).args(...).spawn()?.wait()
      └─ Err → restart UI and report; Ok → process::exit
```

## 7. UI / UX

### List screen

- Left pane: flat list, row format
  `<✓|✗>  YYYY-MM-DD HH:MM  <basename(cwd)>  <title truncated>`
  - `✓` cwd exists / `✗` orphaned (cannot resume).
  - When focused, full cwd is shown (preview top or status line).
- Right pane: preview of the focused session (first few turns). Auto-hidden in narrow terminals; toggle with `p`.
- Bottom: keybinding help line.
- Sort: `last_activity` descending, fixed.

### Keybindings (List)

| Key | Action |
|---|---|
| `↑`/`↓`, `j`/`k` | move cursor |
| `Enter` | open Reader |
| `r` | resume (`claude --resume <id>`) |
| `f` | fork-resume (adds `--fork-session`) |
| `d` | delete (confirmation, then trash) |
| `/` | incremental filter |
| `p` | toggle preview pane |
| `q` / `Esc` | quit |

### Reader screen

- Renders the whole conversation with `user` / `assistant` turn headers.
- `tool_use` → `[tool: <name>]`, `tool_result` → `[tool result]`, `thinking` dropped in v1.
- Scrolling: `↑`/`↓`, `PageUp`/`PageDown`, `j`/`k`.
- `r`: resume from here.
- `q` / `Esc`: back to List.

## 8. Resume

1. Tear down the TUI (leave raw mode, leave alt screen, show cursor).
2. `Command::new("claude").current_dir(<cwd>).args(["--resume", <id>])` (+ `--fork-session` for fork). Inherit stdio.
3. After the child exits, `ccpast` exits. The user returns to their original shell at their original cwd.
4. `cwd_exists == false` → do not spawn. Show a status-line message.
5. `claude` not on PATH (`io::ErrorKind::NotFound`) → show a status-line message; never panic.

A child cannot change the parent shell's cwd. The child's `current_dir` is set instead.

## 9. Delete (trash)

- `d` → `y/n` confirmation → move JSONL to `~/.claude/.trash/<timestamp>-<session-id>.jsonl`. No `rm`.
- Append to `~/.claude/.trash/index.jsonl`:
  ```
  {"trashed_path":"...","original_path":"...","session_id":"...","deleted_at":"<ISO8601>"}
  ```
- `history.jsonl` is left untouched. The README explains that prompt-history residue may remain.
- Remove the row from the in-memory list immediately.

## 10. Error handling

- No `unwrap` / `expect` / `panic!` in production paths. Use `anyhow::Result` + `?`.
- JSONL line parse failures, empty files, and records without required fields are silently skipped.
- `~/.claude/projects/` missing → show "no sessions found" and an empty list; do not error out.
- Terminal resize → redraw.
- Zero sessions → list pane shows `(no sessions found)`; only `q` is accepted.
- `claude` not found → detect `io::ErrorKind::NotFound` and message.
- Non-zero child exit → status bar shows the exit code.
- `fs::rename` across devices → fall back to copy + remove.
- `index.jsonl` append failure → roll back the move and report.

The `ui::mod` module includes a Drop-guarded raw-mode handle so the TUI is torn down even on panic.

## 11. Testing

Pure logic gets unit tests. The TUI itself is verified manually.

| Module | Tests |
|---|---|
| `parser` | string content / array content / tool_result-only / missing message / unknown type / malformed JSON |
| `session::extract_meta` | builds `SessionMeta` from fixtures; covers mtime fallback and `(no title)` |
| `reader::load_full` | `tool_use` / `tool_result` rendering |
| `trash::move_to_trash` | tempdir test for rename + index.jsonl appending; verify restorability |
| `util` | multibyte-aware width truncation, ISO8601 formatting |

Fixtures live under `tests/fixtures/` (minimal hand-crafted samples). **Do not copy real user sessions.**

Manual verification:
- `cargo run -- --list` lists every JSONL on the real machine without crashing.
- TUI keys (`↑/↓ Enter q r d / p`) behave as specified.
- Empty `~/.claude/projects/` directory does not crash.
- 80-col terminal hides the preview pane automatically.

## 12. CLI

- `--help` / `-h`
- `--version` / `-V`
- `--list`: skip the TUI, print a plain listing. Also chosen automatically when stdout is not a TTY.

## 13. Implementation increments

Each step must build and pass tests.

1. `cargo init` + dependencies + empty `main.rs`
2. `scan` + `parser` + `SessionMeta` + `--list` plain output (real-data checkpoint)
3. Minimal `ui::list` (cursor + draw + `q`)
4. Preview pane + `p` toggle + width threshold
5. `reader::load_full` + `ui::reader` + `Enter`
6. `/` filter
7. `resume` (`r` / `f`)
8. `trash` (`d` + confirmation modal)
9. README (English + Japanese)

## 14. Acceptance criteria

- Every session under `~/.claude/projects/` appears in one flat list sorted by most recent activity.
- Rows show date, `basename(cwd)`, title, existence mark (✓/✗); focusing a row also shows the full cwd.
- `↑/↓ + Enter` opens the Reader; `r` actually resumes the session.
- `d` moves the file to `~/.claude/.trash/` and the file is recoverable from there.
- `/` filters incrementally.
- Zero sessions, broken lines, or empty files never crash the tool.
- Runs as a single binary; no `fzf` / Python on PATH required.

## 15. Documentation language policy

- Claude-facing docs (this design, future ADRs, `CLAUDE.md`) are written in English.
- User-facing docs (the eventual `README.md` and any usage guide) ship in both English and Japanese (e.g. `README.md` + `README.ja.md`).
- Conversational replies in chat continue in Japanese unless the user changes language.

## 16. Credit

The JSONL format understanding was informed by `choplin/cclog`'s spec notes. **Do not import or copy that project's code or text.** This parser is an independent implementation. The README will include a one-line credit.
