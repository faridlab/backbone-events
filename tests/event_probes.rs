//! The fail-hard probe suite (hand-written; user-owned; see
//! `metaphor.codegen.yaml`).
//!
//! Every probe runs on its own DISPOSABLE scratch database on the
//! local scratch Postgres (127.0.0.1:5433 — NEVER the live dev
//! database on 5432). A probe that cannot reach the scratch server
//! PANICS — a skipped probe is a failed probe.
//!
//! The named gates (docs/spec.md §14):
//! - concurrent_registration_zero_oversell — the DoD race (8 racing
//!   registers at a 4-seat event: exactly 4 rows, typed refusals).
//! - the /ics refusal family (uniform 404, publication gate, Tier A
//!   tokens, negative enumeration of the public mount).
//! - scheduler arming + mail_done-as-receipt-truth (late re-open,
//!   re-confirm no re-arm, EVM2-4 archive pair, visible window
//!   drops, typed failures recorded, done sweep).
//! - the publish fence (typed refusal; publish/unpublish the only
//!   writers; date_publish stamps once) + my-tickets pinning +
//!   multi-slot/sale-window typed refusals + the once-only
//!   template-apply.

mod probes;
