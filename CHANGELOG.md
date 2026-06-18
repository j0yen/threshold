# Changelog

## v0.4.0 — 2026-06-18

threshold brief --hook: synthesized SessionStart briefing, always exits 0, 5s time-bounded, arrival ledger record; shell shim degrades gracefully if binary absent

## v0.3.0 — 2026-06-18

## threshold-verify

Add `threshold verify` subcommand: parses the latest reflective letter via
`recall list` + `recall show`, classifies each line into a `ClaimKind`
(pushed-repo, shipped-prd, daemon-up, peer-present, in-flight-agent,
pending-todo, narrative), then cross-checks each against live ground truth
(git remotes, build manifest, systemctl). Emits `ClaimVerdict` with status
∈ {confirmed, stale, contradicted, unverifiable}. JSON output schema
`[{claim, kind, status, evidence}]`. Contradicted verdicts are never dropped.
53 lib tests + integration: all green. Live smoke (AC6): `threshold verify`
exits 0 on real box, reaches ground truth, produces ≥1 non-unverifiable.

## v0.2.0 — 2026-06-18

threshold-ledger: append-only JSONL question ledger + ask/answer/open subcommands; LedgerSource wires open questions into threshold brief; session-id resolution with agentns probe + hostname:pid fallback; all 8 ACs met; no new clippy warnings.
