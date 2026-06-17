# Deferred Full-Text Search Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an opt-in body-scope toggle to the existing `/` filter so users can press Tab to extend the substring match to include session conversation bodies.

**Architecture:** A `body_scope: bool` flag and an `Option<HashMap<PathBuf, String>>` cache are added to `FilterState`. A new `Action::LoadBodies` variant triggers a synchronous one-time read of every session body (lowercased on the way in), after which the filter checks the cache in addition to title/cwd. UI shows `[+body]` in the list title while active.

**Tech Stack:** Rust, existing ratatui/crossterm/serde_json stack. No new dependencies.

**Reference spec:** `docs/superpowers/specs/2026-06-17-fulltext-search-design.md`. Read it first.

**Working notes:**
- Public repo. Synthesize all example data.
- No `unwrap` / `expect` / `panic!` in production paths.
- Run `cargo fmt` and `cargo clippy --all-targets -- -D warnings` after each task.
- `cargo` lives at `/Users/satoru/.cargo/bin/cargo` if it isn't on PATH.

---

## Task 1: Extract a pure `match_session` helper for testable filter logic

**Files:**
- Modify: `src/ui/mod.rs`

The current `App::filtered_indices` mixes the App's state with the substring logic, which makes it awkward to unit-test. Extract the per-session match decision into a free function so we can test it directly. Behavior is identical for now.

- [ ] **Step 1: Add the free function `match_session`**

Open `src/ui/mod.rs`. Find the `impl App` block containing `filtered_indices`. Add this **module-level** function (outside the `impl App`):

```rust
/// Decide whether a single session matches the active filter.
/// `query_lower` is assumed to be already lowercased.
/// `body_cache` is the optional cache from `FilterState`; bodies inside are
/// expected to be already lowercased at load time.
fn match_session(
    meta: &SessionMeta,
    query_lower: &str,
    body_scope: bool,
    body_cache: Option<&std::collections::HashMap<std::path::PathBuf, String>>,
) -> bool {
    let title_hit = meta.title.to_lowercase().contains(query_lower);
    let cwd_hit = meta
        .cwd
        .as_deref()
        .map(|c| c.to_lowercase().contains(query_lower))
        .unwrap_or(false);
    let body_hit = body_scope
        && body_cache
            .and_then(|c| c.get(&meta.path))
            .map(|b| b.contains(query_lower))
            .unwrap_or(false);
    title_hit || cwd_hit || body_hit
}
```

(`body_scope` and `body_cache` parameters are wired up here even though `FilterState` does not yet have those fields — Task 2 adds them. Callers in this task pass `false` and `None`.)

- [ ] **Step 2: Refactor `filtered_indices` to use `match_session`**

Replace the existing `filtered_indices` method body with:

```rust
fn filtered_indices(&self) -> Vec<usize> {
    if self.filter.query.is_empty() {
        return (0..self.sessions.len()).collect();
    }
    let q = self.filter.query.to_lowercase();
    self.sessions
        .iter()
        .enumerate()
        .filter(|(_, m)| match_session(m, &q, false, None))
        .map(|(i, _)| i)
        .collect()
}
```

- [ ] **Step 3: Add a small unit test in the same file**

Append this at the bottom of `src/ui/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn meta(title: &str, cwd: Option<&str>, path: &str) -> SessionMeta {
        SessionMeta {
            session_id: "id".into(),
            path: PathBuf::from(path),
            cwd: cwd.map(String::from),
            cwd_exists: true,
            last_activity: chrono::Utc::now(),
            title: title.into(),
            message_count: 0,
        }
    }

    #[test]
    fn title_substring_matches_case_insensitive() {
        let m = meta("Implement TASK 4", Some("/p"), "/p/x.jsonl");
        assert!(match_session(&m, "task", false, None));
    }

    #[test]
    fn cwd_substring_matches() {
        let m = meta("(no title)", Some("/home/user/proj"), "/x.jsonl");
        assert!(match_session(&m, "proj", false, None));
    }

    #[test]
    fn body_scope_off_ignores_body_cache() {
        let m = meta("(no title)", Some("/p"), "/x.jsonl");
        let mut cache = HashMap::new();
        cache.insert(PathBuf::from("/x.jsonl"), "hello world".into());
        assert!(!match_session(&m, "hello", false, Some(&cache)));
    }
}
```

- [ ] **Step 4: Run tests and confirm pass**

```
/Users/satoru/.cargo/bin/cargo test --quiet ui
```

Expected: three new tests pass; total test count is 26 (23 existing + 3 new).

- [ ] **Step 5: Lint and commit**

```
/Users/satoru/.cargo/bin/cargo fmt
/Users/satoru/.cargo/bin/cargo clippy --all-targets -- -D warnings
git add src/ui/mod.rs
git commit -m "refactor(ui): extract match_session helper for testable filter logic"
```

---

## Task 2: Extend `FilterState` with `body_scope` and `body_cache`

**Files:**
- Modify: `src/ui/mod.rs`

Add the two new FilterState fields. Wire `App::filtered_indices` to pass them to `match_session`. Add unit tests covering body-scope matching.

- [ ] **Step 1: Add the imports near the top of `src/ui/mod.rs`**

Find the existing `use` lines at the top and add (or extend an existing `use std::collections::*` block):

```rust
use std::collections::HashMap;
use std::path::PathBuf;
```

The `PathBuf` import may already be unused at top level; if Rust complains about the unused import, leave it because Task 4's cache reads need it as a method parameter type.

- [ ] **Step 2: Extend `FilterState`**

Find the `struct FilterState` definition. Replace it with:

```rust
struct FilterState {
    active: bool,
    query: String,
    body_scope: bool,
    body_cache: Option<HashMap<PathBuf, String>>,
}
```

Find the place in `App::new` (or `Default` impl, depending on the existing code shape) that constructs `FilterState`. Update it to:

```rust
filter: FilterState {
    active: false,
    query: String::new(),
    body_scope: false,
    body_cache: None,
},
```

- [ ] **Step 3: Wire `filtered_indices` to pass the new fields**

Replace the `filtered_indices` method body so it forwards the new state into `match_session`:

```rust
fn filtered_indices(&self) -> Vec<usize> {
    if self.filter.query.is_empty() {
        return (0..self.sessions.len()).collect();
    }
    let q = self.filter.query.to_lowercase();
    self.sessions
        .iter()
        .enumerate()
        .filter(|(_, m)| {
            match_session(
                m,
                &q,
                self.filter.body_scope,
                self.filter.body_cache.as_ref(),
            )
        })
        .map(|(i, _)| i)
        .collect()
}
```

- [ ] **Step 4: Add three more body-scope tests**

In the existing `#[cfg(test)] mod tests` block at the bottom of `src/ui/mod.rs`, append:

```rust
#[test]
fn body_scope_on_with_body_hit_matches() {
    let m = meta("(no title)", Some("/p"), "/x.jsonl");
    let mut cache = HashMap::new();
    cache.insert(PathBuf::from("/x.jsonl"), "hello world".into());
    assert!(match_session(&m, "hello", true, Some(&cache)));
}

#[test]
fn body_scope_on_with_missing_cache_entry_does_not_match() {
    let m = meta("(no title)", Some("/p"), "/x.jsonl");
    let cache: HashMap<PathBuf, String> = HashMap::new();
    assert!(!match_session(&m, "hello", true, Some(&cache)));
}

#[test]
fn body_scope_on_still_matches_title() {
    let m = meta("Hello World", Some("/p"), "/x.jsonl");
    let cache: HashMap<PathBuf, String> = HashMap::new();
    assert!(match_session(&m, "hello", true, Some(&cache)));
}
```

- [ ] **Step 5: Run tests**

```
/Users/satoru/.cargo/bin/cargo test --quiet ui
```

Expected: six ui tests pass (3 from Task 1 + 3 new); total test count is 29.

- [ ] **Step 6: Lint and commit**

```
/Users/satoru/.cargo/bin/cargo fmt
/Users/satoru/.cargo/bin/cargo clippy --all-targets -- -D warnings
git add src/ui/mod.rs
git commit -m "feat(ui): extend FilterState with body_scope and body_cache"
```

---

## Task 3: Surface `body_scope` to `ListView` and update the list title and help line

**Files:**
- Modify: `src/ui/list.rs`
- Modify: `src/ui/mod.rs`

The list block title needs to show `[+body]` when body scope is active. The help line gains `Tab body`. ListView is the bridge.

- [ ] **Step 1: Extend `ListView` in `src/ui/list.rs`**

Find the `pub struct ListView<'a>` definition and add the new field:

```rust
pub struct ListView<'a> {
    pub sessions: &'a [SessionMeta],
    pub indices: &'a [usize],
    pub cursor: usize,
    pub show_preview: bool,
    pub filter_input: Option<&'a str>,
    pub status_override: Option<&'a str>,
    pub body_scope: bool,
}
```

- [ ] **Step 2: Update `render_list` to format the title with `[+body]`**

Find the `render_list` function in `src/ui/list.rs`. Replace the `title` computation with:

```rust
let title = match (view.filter_input, view.body_scope) {
    (Some(q), false) => format!(" Sessions   /{q} "),
    (Some(q), true) => format!(" Sessions   /{q} [+body] "),
    (None, _) => " Sessions ".into(),
};
```

- [ ] **Step 3: Update the help line text**

Find `render_help` in `src/ui/list.rs`. Replace the `help` string literal with:

```rust
let help = "↑/↓ move   Enter view   r resume   f fork   d delete   / filter   Tab body   p preview   q quit";
```

- [ ] **Step 4: Pass `body_scope` from the App's render**

In `src/ui/mod.rs`, find the `Mode::List` branch of `render` that builds `list::ListView { ... }`. Add the field:

```rust
list::render(
    f,
    f.area(),
    list::ListView {
        sessions: &app.sessions,
        indices: &indices,
        cursor: app.selected,
        show_preview: show,
        filter_input: app.filter.active.then(|| app.filter.query.as_str()),
        status_override: app.status.as_deref(),
        body_scope: app.filter.body_scope,
    },
);
```

- [ ] **Step 5: Verify build and tests**

```
/Users/satoru/.cargo/bin/cargo fmt
/Users/satoru/.cargo/bin/cargo clippy --all-targets -- -D warnings
/Users/satoru/.cargo/bin/cargo test --quiet
```

Expected: clean, all 29 tests pass.

- [ ] **Step 6: Commit**

```
git add src/ui/list.rs src/ui/mod.rs
git commit -m "feat(ui/list): expose body_scope to ListView and add [+body] indicator"
```

---

## Task 4: Add `Action::LoadBodies` and the Tab key handler

**Files:**
- Modify: `src/ui/mod.rs`

Wire the Tab key while the filter is active. The first Tab returns `Action::LoadBodies`; subsequent Tabs flip `body_scope`. The run-loop side that actually loads the bodies comes in Task 5.

- [ ] **Step 1: Extend the `Action` enum**

Find the `Action` enum and add a variant:

```rust
enum Action {
    None,
    Resume { fork: bool },
    LoadBodies,
}
```

- [ ] **Step 2: Add the Tab arm to the filter-active branch of `handle_key`**

Find the `if app.filter.active { match code { ... } return Action::None; }` block in `handle_key`. Add a new arm **before** the catch-all `_ => {}`:

```rust
KeyCode::Tab => {
    if app.filter.body_cache.is_none() {
        return Action::LoadBodies;
    }
    app.filter.body_scope = !app.filter.body_scope;
    return Action::None;
}
```

This means: if the cache has never been loaded, escalate to the run loop; otherwise toggle the flag in place.

- [ ] **Step 3: Update `Esc` behaviour to reset `body_scope`**

Per the spec section 7, Esc clears the query AND turns body scope off (cache stays). In the same filter-active branch, find the existing `KeyCode::Esc` arm and update it to:

```rust
KeyCode::Esc => {
    app.filter.active = false;
    app.filter.query.clear();
    app.filter.body_scope = false;
}
```

- [ ] **Step 4: Update the `/` entry handler to reset `body_scope`**

Find the `KeyCode::Char('/')` arm in the normal-mode List branch (NOT the filter-active branch). Update it to also reset `body_scope`:

```rust
KeyCode::Char('/') => {
    app.filter.active = true;
    app.filter.query.clear();
    app.filter.body_scope = false;
}
```

- [ ] **Step 5: Make the run loop a no-op handler for `LoadBodies` (temporary)**

The full implementation of `LoadBodies` is Task 5. For now, find the `match handle_key(key.code, &mut app) { ... }` block in `run`. Extend it to compile:

```rust
match handle_key(key.code, &mut app) {
    Action::None => {}
    Action::Resume { fork } => do_resume(&mut app, &mut terminal, fork)?,
    Action::LoadBodies => {
        // Filled in by Task 5.
        app.status = Some("body search not yet wired".into());
    }
}
```

- [ ] **Step 6: Verify**

```
/Users/satoru/.cargo/bin/cargo fmt
/Users/satoru/.cargo/bin/cargo clippy --all-targets -- -D warnings
/Users/satoru/.cargo/bin/cargo test --quiet
```

Expected: clean. All tests still pass.

- [ ] **Step 7: Commit**

```
git add src/ui/mod.rs
git commit -m "feat(ui): wire Tab key to body_scope and stub LoadBodies action"
```

---

## Task 5: Implement the `LoadBodies` action — synchronous read with status feedback

**Files:**
- Modify: `src/ui/mod.rs`

Replace the placeholder in `LoadBodies` with the real implementation: show a status message, redraw, load every session's body into the cache (lowercased), then flip `body_scope` on and emit a completion status.

- [ ] **Step 1: Replace the `LoadBodies` arm**

Find the `Action::LoadBodies` arm in the run loop. Replace it with:

```rust
Action::LoadBodies => {
    let total = app.sessions.len();
    app.status = Some(format!("reading bodies ({total} sessions)…"));
    terminal.draw(|f| render(f, &mut app))?;

    let mut cache: std::collections::HashMap<std::path::PathBuf, String> =
        std::collections::HashMap::with_capacity(total);
    for m in &app.sessions {
        let body = match crate::reader::load_turns(&m.path, None) {
            Ok(turns) => turns
                .iter()
                .map(|t| t.body.as_str())
                .collect::<Vec<_>>()
                .join("\n")
                .to_lowercase(),
            Err(_) => String::new(),
        };
        cache.insert(m.path.clone(), body);
    }
    app.filter.body_cache = Some(cache);
    app.filter.body_scope = true;
    app.status = Some(format!("body search enabled ({total} sessions)"));
}
```

Notes:
- The `terminal.draw(...)` before the load ensures the "reading bodies…" message is visible to the user before the synchronous read starts.
- `load_turns` errors map to empty strings; the session won't match anything but won't break the loop.
- The body of each session is `t.body` joined with newlines, then lowercased once.

- [ ] **Step 2: Build and test**

```
/Users/satoru/.cargo/bin/cargo fmt
/Users/satoru/.cargo/bin/cargo clippy --all-targets -- -D warnings
/Users/satoru/.cargo/bin/cargo test --quiet
```

Expected: clean. All 29 tests still pass.

- [ ] **Step 3: Manual smoke test (if a TTY is available)**

```
/Users/satoru/.cargo/bin/cargo run --release
```

In the TUI:
1. Press `/` and type a word that should NOT match any title or cwd but is likely to appear in body text (e.g. a common verb you recall using in a session).
2. Verify zero results.
3. Press `Tab`. Watch the status line. After the read, the list should expand if any body contains the substring.
4. Verify the title shows `[+body]`.
5. Press `Tab` again — `[+body]` disappears, narrower set returns.
6. Press `Tab` again — `[+body]` reappears instantly (cache is warm).
7. Press `Esc` — query and `[+body]` both clear.

(If no TTY: skip the smoke test and note it in the report.)

- [ ] **Step 4: Commit**

```
git add src/ui/mod.rs
git commit -m "feat(ui): LoadBodies synchronously reads and lowercases session bodies"
```

---

## Task 6: Verify whole feature end-to-end

**Files:**
- None modified.

A whole-feature sanity check before finishing.

- [ ] **Step 1: Confirm all tests pass**

```
/Users/satoru/.cargo/bin/cargo test
```

Expected: 29 passed.

- [ ] **Step 2: Confirm lint and format are clean**

```
/Users/satoru/.cargo/bin/cargo fmt --check
/Users/satoru/.cargo/bin/cargo clippy --all-targets -- -D warnings
```

- [ ] **Step 3: Confirm `--list` still works (regression check)**

```
/Users/satoru/.cargo/bin/cargo run --quiet -- --list | head -5
```

Expected: the same five-row preview the v1 implementation produced.

- [ ] **Step 4: Inspect the help line text**

Open `src/ui/list.rs`, find `render_help`, and confirm the help line reads:

```
↑/↓ move   Enter view   r resume   f fork   d delete   / filter   Tab body   p preview   q quit
```

If anything is off, fix and re-commit in a follow-up.

---

## Self-review (run after writing the plan)

- **Spec coverage:** Each spec section maps to a task —
  - §2 UX (typing/Tab/Esc/Enter behaviour): Tasks 4, 5.
  - §3 data shape (FilterState fields): Task 2.
  - §4 control flow (Action::LoadBodies + handler): Tasks 4 (variant) and 5 (handler).
  - §5 filter logic: Task 1 (pure helper) + Task 2 (wiring).
  - §6 UI plumbing (ListView field, title, help): Task 3.
  - §7 Esc/Enter/`/`: Task 4 covers Esc and `/`. Enter behaviour was unchanged (filter exits but `query`/`body_scope` persist) — confirm by inspection in Task 6.
  - §8 error handling (failed load = empty cache entry): Task 5 (`match { Err(_) => String::new() }`).
  - §9 testing: Tasks 1, 2 add the six new tests.
  - §11 acceptance: Task 6 smoke test.
- **Placeholders:** none. Every step has concrete code or a concrete command.
- **Type consistency:** `match_session` signature is defined in Task 1 and used unchanged in Task 2. `FilterState` fields added in Task 2 are referenced consistently. `ListView::body_scope` named the same in Tasks 3 and 4.

