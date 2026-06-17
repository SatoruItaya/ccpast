# Deferred full-text search — Design Document

- Date: 2026-06-17
- Status: design approved (pre-implementation)
- Target: extends v1 (PR #3) — adds a body-scope toggle to the existing `/` filter.

## 1. Goal

The current `/` filter on the List screen matches only on the session **title** (first user utterance) and **cwd**. Sessions whose content is interesting but whose first prompt is generic (e.g. `update README`) are unreachable by filter today. This spec adds a **deferred full-text search** that the user opts into per filter session.

Design principle: keep startup fast. Bodies are read **only when the user explicitly escalates a filter**, and the result is cached in memory for the rest of the program's lifetime.

## 2. UX

```
1. User presses `/` and starts typing — filter narrows by title and cwd as today.
2. User presses Tab:
   - First time in this program lifetime: status bar shows "reading bodies…",
     the program synchronously reads every session's body (lowercased), then
     the list title changes to "/foo [+body]" and the filter now matches body too.
   - Subsequent times: instantly toggles body scope on/off (cache stays warm).
3. User keeps typing — re-filters using the cached bodies.
4. Enter exits filter input but keeps both the query AND body scope.
5. Esc clears the query AND turns body scope off (cache stays).
```

### Visual indicators

- List block title: `" Sessions   /foo "` → `" Sessions   /foo [+body] "` when body scope is on.
- Status line during the synchronous read: `"reading bodies…"` (replaces the normal status).
- Status line after the read completes: `"body search enabled (N sessions)"`.
- Help line gains `Tab body` (only meaningful while filter is active, but kept always-visible for discoverability).

## 3. Data shape

`FilterState` (in `src/ui/mod.rs`) gains two fields:

```rust
struct FilterState {
    active: bool,
    query: String,
    body_scope: bool,                              // NEW
    body_cache: Option<HashMap<PathBuf, String>>,  // NEW — values are lowercased bodies
}
```

- `body_cache` stays `None` until the user presses Tab the first time.
- On first Tab, every session's body is loaded and lowercased once, stored in the map keyed by `SessionMeta::path`.
- Subsequent program-level operations (filter changes, navigation, Reader open/close) do not touch the cache.
- The cache is dropped only on program exit.

## 4. Control flow

A new `Action::LoadBodies` is added to the existing `Action` enum (the one already used for `Resume`):

```rust
enum Action {
    None,
    Resume { fork: bool },
    LoadBodies,        // NEW
}
```

`handle_key` (filter-active branch):

```rust
KeyCode::Tab => {
    if app.filter.body_cache.is_none() {
        return Action::LoadBodies;
    } else {
        app.filter.body_scope = !app.filter.body_scope;
        return Action::None;
    }
}
```

`run` loop handler for `Action::LoadBodies`:

```rust
Action::LoadBodies => {
    app.status = Some(format!("reading bodies ({} sessions)…", app.sessions.len()));
    terminal.draw(|f| render(f, &mut app))?;            // show the message immediately
    let mut cache = HashMap::with_capacity(app.sessions.len());
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
    let n = cache.len();
    app.filter.body_cache = Some(cache);
    app.filter.body_scope = true;
    app.status = Some(format!("body search enabled ({n} sessions)"));
}
```

The `terminal.draw(...)` before the load lets the user see the in-progress message; the load itself is synchronous and blocking. Acceptable trade-off — typical totals are <100 sessions and the load completes in well under a second on local disk.

## 5. Filter logic

`App::filtered_indices` becomes:

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
            let title_hit = m.title.to_lowercase().contains(&q);
            let cwd_hit = m
                .cwd
                .as_deref()
                .map(|c| c.to_lowercase().contains(&q))
                .unwrap_or(false);
            let body_hit = self.filter.body_scope
                && self
                    .filter
                    .body_cache
                    .as_ref()
                    .and_then(|c| c.get(&m.path))
                    .map(|b| b.contains(&q))
                    .unwrap_or(false);
            title_hit || cwd_hit || body_hit
        })
        .map(|(i, _)| i)
        .collect()
}
```

Lowercasing the cached body once at load time means each subsequent keystroke does a plain `contains(&q_lower)` against an already-lowercase string.

## 6. UI plumbing

`ListView` gains `body_scope: bool` and the title format becomes:

```rust
let title = match (view.filter_input, view.body_scope) {
    (Some(q), false) => format!(" Sessions   /{q} "),
    (Some(q), true)  => format!(" Sessions   /{q} [+body] "),
    (None, _)        => " Sessions ".into(),
};
```

The help line is extended:

```text
↑/↓ move   Enter view   r resume   f fork   d delete   / filter   Tab body   p preview   q quit
```

## 7. Esc / Enter behaviour

- **Esc** (while filter is active): clears `query`, sets `body_scope = false`, leaves `body_cache` intact.
- **Enter** (while filter is active): exits filter input but preserves `query` and `body_scope`.
- **`/`** (entering filter mode): resets `query = ""` and `body_scope = false`, keeps `body_cache`.

## 8. Error handling

- A failing `load_turns` (corrupt or unreadable file) inserts an **empty string** into the cache. Filter checks will return `false` for that entry — it cannot match anything, but it doesn't crash.
- No new panic surfaces. No `unwrap` / `expect` / `panic!` in production paths.

## 9. Testing

Unit-test the filter logic by exposing a testable helper:

- Add a `App::filtered_indices` test path using a hand-crafted `Vec<SessionMeta>` and a manually built `body_cache`. (May need to expose `App` with `#[cfg(test)]` or extract a free function `fn match_session(meta, query, body_scope, cache) -> bool` that the tests call directly.)
- Cases:
  - Body scope OFF: body hit alone does not match.
  - Body scope ON: body hit alone matches.
  - Body scope ON: title hit still matches.
  - Body scope ON, empty cache entry: never matches.
  - Empty query: returns all indices (existing behaviour, regression check).

Manual verification:
- Press Tab without a query → behaviour is currently to toggle scope; verify nothing crashes (the filter is still active so Tab is intercepted; cache load proceeds even when query is empty).
- Press Tab with a query that only matches a body block → row appears.
- Press Tab a second time → `[+body]` indicator disappears; filter narrows.
- Esc → query and scope both reset.

## 10. Out of scope (deferred)

- Disk persistence of the body cache (would touch `~/.cache/ccpast/`; defer until users complain).
- Fuzzy / regex matching — `nucleo-matcher` or `regex` integration belongs in a separate proposal.
- Progressive (non-blocking) reads via a background thread — acceptable for a future revision; the simple blocking version is fine at typical scale.

## 11. Acceptance criteria

- Pressing `/` then `Tab` after a substring query expands the visible matches to include sessions whose body contains the substring.
- The list block title shows `[+body]` while body scope is on.
- The status line reports the load progress in the simple `reading bodies (N sessions)…` form during load.
- A subsequent Tab toggles body scope off without re-reading.
- Esc clears the query and turns body scope off. `/` again starts fresh.
- All existing tests still pass; new tests cover the four body-scope filter cases above.
