# CLAUDE.md — ccpast

Project instructions for Claude Code working in this repository.

## This is a public repository — keep internal data out

This repository is **public**. Before adding any file (docs, code comments, examples, fixtures, test data, commit messages), make sure it does **not** contain:

- Real absolute paths from the local machine (`/Users/<name>/...`, `/home/<name>/...`)
- The user's name, email, GitHub handle in places where they are not already public
- Real session IDs / UUIDs / `request_id`s observed in `~/.claude/` or other private data
- Real timestamps captured from on-disk sessions (use a clearly synthetic example like `2026-01-01T00:00:00.000Z`)
- Names of private/work repositories, organizations, or projects observed during exploration of `~/.claude/projects/`
- Verbatim content from real conversations or tool outputs encountered during development

When an example needs concrete-looking data, **synthesize it** (e.g. `/abs/path`, `<session-id>`, `2026-01-01T00:00:00.000Z`). Fixtures under `tests/fixtures/` must be hand-crafted, not copied from real `.jsonl` files.

If you discover that something internal has already been committed, raise it immediately and propose a sanitizing commit (do not silently rewrite history on a public branch unless explicitly asked).

## Documentation language policy

- **Claude-facing docs** (this file, design specs under `docs/`, internal notes): write in **English**.
- **User-facing docs** (README, install guides, tutorials, changelogs aimed at end users): provide **both** an English file and a Japanese file (e.g. `README.md` + `README.ja.md`). Keep them in sync.
- Conversational replies to the user remain in Japanese unless the user switches language.

## Purpose and differentiation

`ccpast` is a single-binary terminal UI for browsing, resuming, and deleting Claude Code session history across **all** working directories at once. It is intentionally different from tools like `choplin/cclog` (project-first + fzf/python) in three ways:

1. **Flat cross-cutting list is the main view.** All sessions from all projects are merged into one list sorted by `last_activity` descending. Users do not have to remember which repository a session belonged to.
2. **Single binary, no external dependencies.** No `fzf`, no Python. The only external process invoked is `claude` itself (for resume).
3. **Built-in trash-based deletion.** Sessions can be deleted safely (recoverable via manual restore from a trash directory).

## Tech stack

- Rust (single binary from `cargo build --release`; package name = binary name = `ccpast`)
- `ratatui` + `crossterm` for the TUI (no `fzf` dependency)
- `serde_json` (+ `serde` derive as needed) for JSONL parsing
- `chrono` for timestamps
- `dirs` to locate `~/.claude`
- `anyhow` for error propagation

Do **not** add `nucleo-matcher` or any fuzzy matcher in v1. Keep the search layer simple enough to swap later.

## Module layout

```
src/
├── main.rs        # CLI flags (--help, --list, --version) and dispatch
├── scan.rs        # Enumerate ~/.claude/projects/**/*.jsonl and extract lightweight metadata
├── parser.rs      # Low-level: turn a JSONL line (serde_json::Value) into typed structs
├── session.rs     # SessionMeta / SessionFull and their construction
├── reader.rs      # Load one session's full body into a sequence of turns
├── trash.rs       # Move to ~/.claude/.trash/ and append to index.jsonl
├── resume.rs      # Spawn `claude --resume`
├── ui/
│   ├── mod.rs     # App state, event loop, ratatui draw hub
│   ├── list.rs    # List screen
│   ├── reader.rs  # Reader screen
│   └── confirm.rs # Delete confirmation modal
└── util.rs        # Width-aware string truncation, ISO8601 formatting, etc.
```

Avoid premature abstraction. Before adding a new file, check whether it fits into an existing module.

## Data source

- Sessions live at `~/.claude/projects/<encoded-cwd>/<session-id>.jsonl` — one JSON object per line.
- The directory name is an encoded form of the original path (`/` → `-`). **Do not try to decode it back** to the original path. Read `cwd` from inside the JSONL instead (first non-null `cwd` field encountered).
- **Never read or modify `~/.claude/history.jsonl`.** It does not cover all sessions, so it is unsuitable as a list source; v1 also does not touch it on delete (note this in the README).

### JSONL schema observations (verified on real data, 2026-06-16)

Records are distinguished by `type`. Observed types: `user`, `assistant`, `summary`, `attachment`, `mode`, `permission-mode`, `last-prompt`, and others. **Treat unknown `type` values as no-ops** (skip silently).

#### user / assistant records (carry messages)

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

- `message.content` is **string**, **array**, or **absent**. Be null-safe in all paths.
- Array block types observed: `text`, `thinking`, `tool_use`, `tool_result`.
- `tool_result.content` is a string.
- Some `user` records have only `{ role: "user" }` with no content — these are scaffolding for later turns; skip them when scanning for a title.

#### summary records

```
{"type":"summary","summary":"<string>","leafUuid":"..."}
```

These have no `cwd` / `sessionId` / `timestamp`. The `summary` field has been seen to contain strings like `API Error: 404 ...` — it is **not** a reliable title source. **v1 ignores `summary` for the title** (see decisions below).

#### other types

`last-prompt`, `mode`, `permission-mode`, etc. usually have null `cwd` / `timestamp`. Skip them during metadata extraction.

### Per-session data model

Lightweight metadata, built once at startup for every session:

| Field | Source |
|---|---|
| `session_id` | filename stem |
| `path` | absolute path to the JSONL file |
| `cwd` | first non-null `cwd` in the file |
| `cwd_exists` | filesystem check on `cwd` |
| `last_activity` | max `timestamp` in the file; fall back to file mtime |
| `title` | first human user utterance (see below); `(no title)` if none |
| `message_count` | number of `user`/`assistant` rows |

Title extraction rules:
- Scan `type:"user"` rows. If `message.content` is a string, use it. If it is an array, use the first `type:"text"` block's `text`.
- Reject rows whose user content consists only of `tool_result` blocks.
- Collapse newlines into spaces.
- If nothing qualifies, set title to `(no title)`.

## Confirmed design decisions (from brainstorming)

1. **Title source**: always prefer the first user utterance. v1 ignores `summary`.
2. **Project filter UI**: not in v1. `/` substring search over `cwd` and `title` (case-insensitive) is enough.
3. **Preview pane**: auto on/off by terminal width threshold (e.g. < 100 cols hides it); user can toggle with `p`.
4. **Trash scope**: physical move + append to `index.jsonl`. Manual restore via `mv`. Do not touch `history.jsonl`.
5. **Scan strategy**: two-pass. At startup extract lightweight metadata only (may early-break per file once needed fields are filled). When the Reader opens, re-read the whole file to format turns.

## UI / UX

### List screen (primary)

- Left pane: flat list of all sessions. Row format:
  `<✓|✗>  YYYY-MM-DD HH:MM  <basename(cwd)>  <title truncated>`
  - `✓` = `cwd` exists on disk; `✗` = orphaned (cannot resume).
  - When a row is focused, show the full `cwd` somewhere visible (preview top or status line).
- Right pane: preview of the focused session (first few turns, formatted). Auto-hidden in narrow terminals; toggle with `p`.
- Bottom line: keybinding help.
- Sort: `last_activity` descending, fixed.

### Keybindings (List)

| Key | Action |
|---|---|
| `↑`/`↓`, `j`/`k` | move cursor |
| `Enter` | open Reader (primary action is viewing) |
| `r` | resume (`claude --resume <id>`) |
| `f` | fork-resume (adds `--fork-session`) |
| `d` | delete (confirmation modal, then trash) |
| `/` | incremental filter (case-insensitive substring over title and cwd) |
| `p` | toggle preview pane |
| `q` / `Esc` | quit |

### Reader screen

- Render the whole conversation with `user` / `assistant` turn headers.
- `tool_use` blocks → `[tool: <name>]`. `tool_result` blocks → `[tool result]`. `thinking` blocks are dropped in v1.
- Scrolling: `↑`/`↓`, `PageUp`/`PageDown`, `j`/`k`.
- `r`: resume from here.
- `q` / `Esc`: back to List.

## Resume implementation

1. Tear down the TUI (leave raw mode, leave the alternate screen, show the cursor).
2. `Command::new("claude").current_dir(<cwd>).args(["--resume", <id>])`. For fork, also add `--fork-session`. Inherit stdio.
3. Wait for the child to exit, then exit `ccpast`. The user is returned to their original shell at their original cwd.
4. If `cwd_exists == false`, do not spawn; show a status line message.
5. If `claude` is not on PATH (`io::ErrorKind::NotFound`), show a status line message instead of panicking.

A child process cannot change the parent shell's cwd. We must set `current_dir` on the child.

## Delete (trash)

- `d` → `y/n` confirmation → move the JSONL to `~/.claude/.trash/<timestamp>-<session-id>.jsonl`. Never `rm`.
- Append to `~/.claude/.trash/index.jsonl`:
  ```
  {"trashed_path":"...","original_path":"...","session_id":"...","deleted_at":"<ISO8601>"}
  ```
- Do not touch `history.jsonl`. The README must mention that prompt-history residue may remain there.
- Remove the row from the in-memory list immediately.

## CLI

- `--help` / `-h`
- `--version` / `-V`
- `--list`: skip the TUI, print a plain listing. Also used automatically when stdout is not a TTY.

## Acceptance criteria

- Every session under `~/.claude/projects/` shows up in one flat list, sorted by most recent activity.
- Each row shows date, `basename(cwd)`, title, and the existence mark (✓/✗); full cwd is visible when a row is focused.
- `↑/↓ + Enter` opens the Reader; `r` actually resumes the session.
- `d` moves the file to `~/.claude/.trash/`; manual restore from there works.
- `/` filters incrementally.
- Zero sessions, broken lines, or empty files never crash the tool.
- Runs as a single binary with no `fzf` / Python on PATH.

## Working notes for Claude

- No `unwrap` / `expect` / `panic!` in production paths (`anyhow::Result` + `?`).
- Silently skip unparsable JSONL lines, empty files, and records without required fields.
- No speculative abstractions or future-proofing traits (YAGNI).
- Prefer editing existing files over creating new ones; new modules require justification.
- Default to writing no comments. Only comment when the *why* is non-obvious.
- Terraform fmt rule from global CLAUDE.md still applies if Terraform is ever added (currently not).
- PRs must be created with `gh pr create --draft` per the user's global rule.

## Credit

The JSONL format understanding was informed by `choplin/cclog`'s spec notes. **Do not import or copy that project's code or text.** The parser here is an independent implementation. Add a one-line credit in the README when it is written.
