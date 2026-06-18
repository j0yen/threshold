# threshold

Session arrival briefing synthesizer for wintermute.

Gathers scattered signals from multiple sources and synthesizes them into a
single prioritized arrival briefing — one concise summary instead of the raw
firehose of ten independent SessionStart hooks (~21 KB of unsynthesized text).

## Installation

```sh
cargo build --release
cp target/release/threshold ~/.local/bin/threshold
```

Or use the install script:

```sh
bash scripts/install.sh
```

## Usage

```sh
# Text mode (default) — human-readable, ≤ 4 KB
threshold brief

# JSON mode — machine-readable, documented schema
threshold brief --format json

# Limit items per section
threshold brief --max-items 5

# Use a fixture directory (testing seam — see Testing below)
threshold brief --source-root /path/to/fixtures
```

## Sources

`threshold brief` queries six signal sources. Each source degrades gracefully
to an empty contribution if its backing data is missing or malformed.

### RecallSource

Reads the last 5 lines of `~/.local/share/recall/reflective.log`. Emits
`Owed` signals for recent reflective recall entries.

### GossipSource

Reads the last 500 bytes of `~/wintermute/autobuilder/notes/gossip.md`.
Emits `InFlight` signals for recent gossip notes.

### BuildManifestSource

Reads `~/.claude/skills/build/state/manifest.json`. Emits:
- `InFlight` signals for PRDs with `status: "in_progress"`
- `Owed` signals for PRDs with `status: "blocked"`

### GitSource

Scans `~/wintermute/*/` for git repositories. For each repo, emits `Changed`
signals when:
- The working tree has uncommitted changes (`git status --short`)
- There are unpushed commits (`git log @{u}..HEAD`)

### DocketSource

Reads `~/wintermute/autobuilder/notes/docket.md`. Lines starting with
`- [ ]` are open findings; each becomes an `Owed` signal.

### ReviewDueSource

Checks for the flag file `~/.claude/skills/build/state/review-due`. If
present, emits one `Owed` signal reminding to run `/self-review`.

## JSON Schema

`threshold brief --format json` outputs a `Briefing` object with
`schema: "threshold.briefing.v1"`:

```json
{
  "schema": "threshold.briefing.v1",
  "generated_at": "<ISO 8601 timestamp>",
  "sections": {
    "mid_flight": [ <BriefingItem>, ... ],
    "owed_to_you": [ <BriefingItem>, ... ],
    "changed_since_last": [ <BriefingItem>, ... ],
    "dont_redo": [ <BriefingItem>, ... ]
  },
  "total_items": <integer>,
  "sources_queried": [ "<source_name>", ... ]
}
```

Each `BriefingItem`:

```json
{
  "kind": "in_flight" | "owed" | "changed" | "dont_redo",
  "title": "<string, ≤80 chars recommended>",
  "body": "<string>",
  "priority": <0-100>,
  "source": "<source_name>",
  "freshness_secs": <integer or null>
}
```

### Sections

| Section | Kind | Description |
|---------|------|-------------|
| `mid_flight` | `in_flight` | In-progress tasks and PRDs |
| `owed_to_you` | `owed` | Blocked items, open findings, reviews due |
| `changed_since_last` | `changed` | Dirty repos, unpushed commits |
| `dont_redo` | `dont_redo` | Already-completed / already-failed items |

## Testing

### Unit tests

```sh
cargo test
```

### `--source-root` testing seam

All sources accept a `--source-root` path that replaces the real filesystem
root with a fixture directory. With `--source-root /path/to/root`:

| Source | Reads from |
|--------|-----------|
| RecallSource | `<root>/recall/reflective.log` |
| GossipSource | `<root>/wintermute/autobuilder/notes/gossip.md` |
| BuildManifestSource | `<root>/.claude/skills/build/state/manifest.json` |
| GitSource | `<root>/wintermute/` (scans for `.git` dirs) |
| DocketSource | `<root>/wintermute/autobuilder/notes/docket.md` |
| ReviewDueSource | `<root>/.claude/skills/build/state/review-due` |

This seam lets acceptance tests produce deterministic output without touching
real data:

```sh
threshold brief --source-root tests/fixtures/
```

### FakeSource (library tests)

The `threshold::sources::FakeSource` type implements `SignalSource` with a
fixed `Vec<Signal>` supplied at construction time. Use it for unit and
integration tests that call the synthesizer directly:

```rust
use threshold::sources::FakeSource;
use threshold::{Signal, SignalKind, synthesize};

let src = FakeSource::new("test", vec![
    Signal::new(SignalKind::InFlight, "task-A", "detail", 80, "test"),
]);
let signals = src.collect().unwrap();
let briefing = synthesize(signals, 10);
```

## Architecture

```
[SignalSource impls]  → Vec<Signal>
        ↓
  synthesize()        (pure function, no I/O)
        ↓
    Briefing          (sectioned, priority-ordered, size-capped)
        ↓
render_text() / serde_json::to_string_pretty()
```

The synthesizer (`src/synthesizer.rs`) is a pure function with no I/O:
dedup → section assignment → priority sort → max-items cap → `Briefing`.

## Out of scope (later PRDs)

- **threshold-verify**: claim verification against real data
- **threshold-ledger**: ask/answer ledger
- **threshold-hook**: hook wiring to replace existing SessionStart hooks

## License

MIT OR Apache-2.0
