# backbone-events — the module specification (events core: seat truth, registrations, /ics, the self-arming communication scheduler)

This document is the build contract for the `backbone-events` module: every
decision in it is frozen. An implementer executes this file without re-deciding
anything; where a choice is deliberately left to a later increment, the file
says so explicitly and names the increment's re-entry condition. The upstream
reference is the Odoo events cohort (`docs/odoo/marketing/events/` in this
repository, register IDs EBB / EVM / EVM2 / EV / EBF / EBG / EP / ES); every
deviation from upstream behavior is recorded with its reason. The governing
product-side records are the website pillar section
`docs/plan/08-pillar-website.md` §WB-4 and the module-council condition rows
W7-C16/W7-C17 (`docs/council/2026-09-01-module-w7-p0-platform-pass.md`).

Identity, in one line each:

- **Crate** `backbone-events`, version `0.1.0`, Postgres **schema `event`**,
  HTTP mount base **`/api/v1/event`** (both spellings verified unoccupied in
  the host tree — §0; the module itself mounts nothing — the host nests the
  exported routers). The schema and route names follow the module council's
  one-crate ruling verbatim: "ONE crate backbone-events (schema: event) …
  Route base /api/v1/event per the schema-name ruling" (W7-C17). A workspace
  briefing circulating at authoring time said schema `events` / `/api/v1/events`;
  that wording is superseded by the council record, which is the adopted
  ruling. `backbone-events` (plural crate, the chain of eight absorbed addons)
  owns schema `event` (singular, the upstream Odoo addon name) exactly as
  `backbone-website` owns schema `website` and `backbone-portal` owns `portal`.
- **One crate, eight arms**: the whole upstream chain
  `event → event_product → event_sale → event_booth → event_booth_sale →
  event_crm → event_crm_sale → event_sms` ports as internal arms of this ONE
  crate (council adjudication A9 / W7-C17). This increment builds the CORE arm:
  the cycle-10 `event` addon's load-bearing set — event + registration + seat
  truth + `/ics` + the self-arming mail scheduler. The chain remainder
  (event_product, the event_sale composition, booths, crm, the sms overlay) is
  the next increment (§11); nothing in this spec quietly builds any of it.
- **Upstream edges** (each verified against the working trees on 2026-09-02):
  - `backbone-mail` — the ONE sibling module edge, a direct unconditional
    git-tag pin, **same version the host pins**. Healed world: **v0.2.7**. The
    mail dual-resolve heal (council condition W7-C16, §8.6) is a BLOCKING
    precondition at this increment's open: backbone-notification releases past
    its v0.2.6-carrying v0.4.7 (its working tree already carries the staged
    0.4.8 bump + mail v0.2.7 re-pin, uncommitted), the host re-pins mail
    v0.2.7 in the same change, pin-probe leg 3b's carve-out is rescoped in
    that same change. **Recorded fallback** if that release slips: this module
    pins mail **v0.2.6** — joining the existing host island inside the
    EXISTING carve-out — never a third mail copy, never a carve-out extension
    to a new entry. No mail gateway features are enabled here (transport is
    the host's; this module only enqueues).
  - `backbone-notification` — **NO crate edge.** Notification is the heal's
    vehicle, not this module's dependency.
  - `backbone-selling` — **NO crate edge.** The sale seam rides events-owned
    mirror columns + the declared domain-event subscription (selling's
    `SalesOrderConfirmed` / `SalesOrderCancelled`, `selling.hook.yaml:135-140`
    — verified as the mint/cancel carriers). The subscription wiring lands with
    the event_sale arm (§11); the columns it feeds are declared NOW (§4.3).
  - `backbone-sapiens`, `backbone-party` (partners), corporate (company),
    geo (countries/addresses) — **logical uuid references only**
    (`@exclude_from_foreign_key_check`), never cross-schema FKs, never crate
    edges.
  - `backbone-website` — **NO crate edge.** The `/ics` guard consumes the
    publish-contract PATTERN (stored flag + derived visibility + explicit
    fence verbs), re-declared module-side (§6.1); it does not call website
    code. WB-4's place after WB-3 in the build ladder is pattern consumption,
    not a code edge.
  - `backbone-framework` — all five framework crates pinned **v2.7.11**
    exactly (single-rev baseline; no branch floats, no `[patch]`).
  - `backbone-messaging` — compile-only, emitted by the pinned generator's
    event scaffolding imports (the recorded backbone-website deviation; zero
    runtime use, no outbox entry).
- **Company fence**: `shared_blank` declared (ADR-0014 posture 2) — see §2.2
  for the per-model posture map. Upstream `event.event.company_id` is
  `required=False` with global `company_ids + [False]` record rules (EVM-15
  "company is soft"): a strict fence would fragment global stages and
  schedulers, which is a behavior change, not a hardening.
- **No durable events at core**: the module stages no outbox events and gets
  NO entry in the host's `outbox_schemas` producer sets. The scheduler arms
  itself through its own rows (§8); the sale-side subscription (next
  increment) CONSUMES the staged outbox, it does not add a producer. Adding a
  producer entry later requires a real consumer + an `outbox_schemas` decision
  in the same change.

Family conventions this module follows verbatim (verified against
backbone-website v0.1.0 and backbone-foundation-ext v0.1.0):

- Schema/model YAML is the source of truth; every hand-written file is declared
  under `user_owned:` in `metaphor.codegen.yaml` **in the same change that
  lands it**.
- Migrations carry no `GRANT`s (owner-role DDL). The composing host re-runs
  `apps/serpa-service/scripts/rls_app_role.sql` as owner after
  `metaphor migration run-all`; its blanket per-schema grants cover `event`.
- Enum types are created UNQUALIFIED in schema `public`, census-checked for
  collisions before landing. **The `event_` prefix is CONTESTED**:
  backbone-calendar already owns `event_attendee_state`,
  `event_exception_kind`, `event_privacy`, `event_recurrence_freq`
  (`migrations/20260828140000_create_event_family_enums.up.sql` — the calendar
  event family, a different "event"). Every enum this module creates avoids
  those four exact names (§2.4 lists them); the DB schema `event` itself is
  free (no `CREATE SCHEMA event` anywhere in the module family).
- Cross-module references are LOGICAL: indexed uuid columns with
  `@exclude_from_foreign_key_check`, never a `FOREIGN KEY` across schema
  boundaries. Intra-module references (registration→event, →slot, →ticket,
  answer→question…) are REAL FKs inside schema `event`.
- The clippy bar is a gate-time CLI flag:
  `cargo clippy --all-targets -- -D clippy::expect_used` → EXIT=0.
- Probes are fail-hard: one disposable scratch Postgres per test
  (`localhost:5433`, user/password `postgres`/`postgres`, overridable via
  `EVENT_TEST_ADMIN_URL`), raw-SQL migration runner in filename order; a
  missing scratch server PANICS the suite. The live dev database on 5432 is
  never touched by tests.

---

## 0. Pre-flight gates (run before any schema work; exit codes quoted)

Executed 2026-09-02 from the workspace root
(`/Users/faridlab/startapp/products/serpa-workspace`):

```
$ grep -rn "api/v1/events" apps/serpa-service/src
(no matches)                                            → exit 1   [base free]

$ grep -rn "api/v1/event" apps/serpa-service/src
(no matches — covers the singular base AND every event_* prefix) → exit 1

$ ls -d modules/backbone-events
(no such path — absent from the consumer workspace)     → exit 1

$ ls -d /Users/faridlab/startapp/frameworks/metaphora/modules/backbone-events
(existed only after this spec created docs/; NO code, NO Cargo.toml,
 NO schema/ — the build seat scaffolds those)           → n/a (docs-only)
```

The mount base is free under BOTH spellings; the module does not exist in
either tree. Both facts are preconditions for §9's mount plan.

---

## 1. The first falsifier — the `split` lifecycle declaration at the schema gate

**This is the cheapest early failure this increment is designed to surface, and
it runs BEFORE any service code is written.** If it fails, reporting the gap IS
a valid outcome of the increment; do not work around it silently.

The contract (pillar §WB-4, W7-C17): `registration.state` is declared ONCE, in
this crate, with ADR-0016 `lifecycle: split`, covering BOTH branches — the bare
branch (no sale linkage: hand-set, default `open`, the four verbs) and the
sale-composed branch (`event_sale` REDECLARES state upstream as an editable
compute from (SO state, amount); the port expresses that composition as the
split, with NO cross-crate field redeclaration anywhere).

Why this is a real test and not a rubber stamp — three facts verified against
the pinned schema plugin (`metaphor-plugin-schema/src/ast/model.rs`):

1. The `lifecycle:` field key EXISTS (`pub lifecycle: Option<Lifecycle>`), the
   `Split` shape exists, and `driver: Option<String>` is documented as **"The
   field (same model) that drives a `split` or `projection` shape"** — the
   driver must live on the SAME ROW. That is exactly why the council ruled the
   sale seam rides "events-owned rows + declared seams": the registration row
   carries its own mirror columns of the sale state (§4.3), so the driver is
   same-model by construction.
2. The lifecycle declaration is "metadata-only in v1: preserved for ports and
   review, with structural validation" — the question this falsifier answers
   is whether that STRUCTURAL VALIDATION accepts the shape below (a split
   whose driver is a NULLABLE mirror — absent driver = the bare hand-set arm)
   rather than rejecting or silently dropping it.
3. The upstream composition is a derived PAIR (`state` AND `sale_status` move
   jointly per the ES-1 truth table), while the DSL's `driver` names one field
   and the split's taxonomy definition is "names the driver field + the derived
   pair semantics". Whether the pair semantics are expressible/validatable in
   the declaration (vs. only in prose) is precisely the expressibility proof
   demanded.

### 1.1 The declaration shape the build attempts (in `schema/models/registration.model.yaml`)

```yaml
# --- the sale seam (events-owned mirror columns; written ONLY by the
# selling domain-event subscription — the next increment's wiring; §4.3)
sale_order_id:
  type: uuid
  attributes: ["@exclude_from_foreign_key_check", "@indexed"]
  description: "logical ref to the selling module's sale order; nullable (the bare branch)"
sale_order_state:
  type: enum
  attributes: ["@values(draft|sent|sale|cancel)"]
  description: "THE SPLIT DRIVER — events-owned mirror of the linked order's state"
sale_status:
  type: enum
  attributes: ["@values(free|sold|to_pay)"]
  description: "derived-pair member 2 (the payability axis)"
# --- the state itself
state:
  type: enum
  attributes: ["@values(draft|open|done|cancel)", "@default(open)", "@no_copy"]
  lifecycle:
    shape: split
    driver: sale_order_state
    # derived-pair semantics (the ES-1 truth table) — §4.4 is the normative text
```

### 1.2 The gate

At schema freeze, before any `src/` code:

```
cd /Users/faridlab/startapp/frameworks/metaphora/modules/backbone-events
metaphor schema generate --force        # EXIT=0, and byte-stable on re-run
metaphor lint check                     # EXIT=0 at the ADR-0015 bar
```

Outcomes, in order of precedence:

- **EXIT=0 with the split preserved in generated output** → the falsifier
  passes; the ADR-0016 vocabulary covers the events composition; proceed.
- **Validation rejects the shape** (nullable driver, pair semantics, split on
  an enum with a default…) → STOP. Report the exact rejection. The honest
  dispositions are then either (a) a `metaphor-plugin-schema` increment adding
  the missing validation arm (framework tree, its own change), or (b) the
  pair-semantics declaration landing as model-file documentation + hooks
  reference while `shape: split` + `driver` carry the structural half. Which
  of (a)/(b) is an owner decision taken on the reported finding — NOT a
  silent downgrade by the build seat.
- **EXIT=0 but the declaration is dropped from output** (accepted-and-ignored)
  → that is a FAILED proof (silence is not expressibility); report it as such.

The registration-state machine ALSO keeps its hooks declaration (§4.2) for the
bare-branch transition surface — ADR-0016: "`state_machines` remains for
pattern-2 transition guards only; a `hand_set` lifecycle may reference a hook
state machine" — the split subsumes the hand-set arm as its bare branch.

---

## 2. Table set (schema `event`)

All tables live in schema `event` (migrations emit `CREATE SCHEMA event`).
Soft-delete (`active`) only where named; the module-level `config:` block
mirrors backbone-website's (audit on, default timestamps on, generators as
emitted).

### 2.1 Tables

| table | grain | one-liner (upstream model) |
|---|---|---|
| `event.events` | one event | `event.event` — identity, dates/TZ, venue (logical party address ref), two-axis lifecycle fields, seats config, publication pair, `is_multi_slots` |
| `event.stages` | stage row | `event.stage` — name/sequence/fold/`pipe_end` (the only semantic) |
| `event.types` | event template | `event.type` — the template: default mail lines / tickets / questions / seats+tz defaults |
| `event.tags` + `event.tag_categories` | tags | `event.tag` / `event.tag.category` — composition sugar; NO random color (EVM-13 not ported: `color` nullable, officer-set, default NULL) |
| `event.tickets` | ticket row | `event.event.ticket` — name, `seats_max` (0 = unlimited), per-order cap, sale window (read-time lazy, §2.3) |
| `event.slots` | slot row | `event.slot` — dates/hours inside the event range; NO own cap (measured against `events.seats_max`) |
| `event.questions` | shared-library question | `event.question` — M2M-linked to events/types; `is_default`⇒`is_reusable` CHECK (EBG-12); identity types create no answer rows |
| `event.question_answers` | suggested answers | `event.question.answer` — choice values for `simple_choice` |
| `event.registration_questions` | M2M rel | the ONE rel table behind both `general`/`specific` domain surfaces (EVM-12: one rel, two declared views — never two tables) |
| `event.registrations` | the attendee row | `event.registration` — §4 |
| `event.registration_answers` | answer grain | `event.registration.answer` — one row per (registration, question), value XOR CHECK (EBG-13) |
| `event.mails` | scheduler row | `event.mail` — §8 (one row, three engines) |
| `event.mail_registrations` | per-attendee receipt | `event.mail.registration` — §8.3, UNIQUE `(scheduler_id, registration_id)` (EVM2-3 fixed) |
| `event.mail_slots` | per-slot child | `event.mail.slot` — lazily materialized slot-grain scheduler children |
| `event.type_mails` | template mail line | `event.type.mail` — copied onto events by template-apply |
| `event.event_audit_log` | audit vocabulary | the family's audit table (website/portal pattern) |

Deliberately ABSENT from core (each named in §11): everything the chain
remainder owns — sale-order line extensions, booth tables, crm rule tables,
sms templates; plus UTM attribution columns (no utm module exists in the
composed graph; columns would be dead weight — re-entry: the attribution
increment with the storefront), and visitor/tracking surfaces (the W8
website_event family; this module's visitor exposure is none).

### 2.2 RLS posture — `shared_blank` declared, per-model posture map

Module-level declaration (the digest-amendment precedent — a module-level
value plus a truthful per-model map):

```yaml
company_fence: shared_blank
```

| model | company column | policy |
|---|---|---|
| `events` | `company_id` uuid NULLABLE (logical ref, corporate) | ADR-0014 posture-2 policy: `company_id = current_setting(...) OR company_id IS NULL` on both clauses — NULL rows are first-class "visible to all" (the port of Odoo's `[False]` escape) |
| `registrations` | `company_id` NULLABLE, derived from its event at write | same policy (upstream: related stored company) |
| everything else (stages, types, tags, tickets, slots, questions, answers, the whole scheduler family, audit) | NO column | no policy — company-shared globals, exactly upstream (ADR-0014 names "event stages" as a shared_blank exemplar) |

EVM-15 is thereby ported faithfully as a DECLARED posture (soft company), not
synthesized into a strict fence; making tenancy hard is a recorded additive
decision for a future owner, not a default.

### 2.3 The sale window is read-time lazy (zero crons)

Ticket `start_sale_datetime` / `end_sale_datetime` gate saleability as a pure
read predicate (computed in the event TZ against stored UTC instants — §4.6);
no cron flips anything; an expired ticket is simply unsellable (upstream shape
preserved, EVM-5's wall-clock virtual). Declared `read_time_lazy` where
postures are declared.

### 2.4 Enums (public schema, census-checked)

`event_registration_state` (draft|open|done|cancel) · `event_kanban_state`
(normal|done|blocked|cancel) · `event_interval_unit` (now|hours|days|weeks|
months) · `event_interval_kind` (after_sub|before_event|after_event_start|
after_event|before_event_end) · `event_notification_channel` (mail) ·
`event_sale_status` (free|sold|to_pay) · `event_sale_order_state` (draft|sent|
sale|cancel) · `event_question_kind` (simple_choice|text_box|name|email|phone|
company_name) · `event_badge_format` (A4_french_fold|A6|four_per_sheet) ·
`event_audit_event` (…) · `event_mail_error_kind` (§8.5).

Census note (the load-bearing one): backbone-calendar owns
`event_attendee_state`, `event_exception_kind`, `event_privacy`,
`event_recurrence_freq` — none of the names above collide exactly, and the
stems are distinct. The census re-runs over every module migration tree before
the enum migration lands (the portal/website convention).

---

## 3. The two-axis event lifecycle (EVM-1; there is NO `state` column on the event)

Upstream `event.event` has no state field. Its lifecycle is two almost
orthogonal axes, and both port:

### 3.1 Stage axis — `stage_id` with `lifecycle: stage_ref`

Real FK to `event.stages`, `ondelete RESTRICT`. Freely-configured stage rows;
**`pipe_end` is the ONLY semantic** (the terminal marker). Terminal writes
happen ONLY via the explicit `mark_done` verb (writes the first `pipe_end`
stage by sequence) and the done sweep (§3.3). Declared:

```yaml
stage_id:
  lifecycle: { shape: stage_ref }
```

### 3.2 Kanban health axis — `kanban_state`, hybrid with STICKY cancel

Enum `event_kanban_state`, declared:

```yaml
kanban_state:
  lifecycle:
    shape: hybrid          # hand-editable; the compute is a reset guard
    sticky: true           # 'cancel' survives the reset — the ADR-0016 canonical sticky mark
```

Semantics ported verbatim: a stage change RESETS any non-`cancel` value to
`normal`; **`cancel` is sticky and IS the event-cancellation signal**,
consumed by three independent readers — registration-open gating, the
scheduler's cancelled-state derivation, and the scheduler job's claim domain.
EBB-8 ("stage moves silently wipe kanban blocked/done marks") is upstream
BEHAVIOR, carried honestly: the reset is the semantics; the declaration above
puts it on record (officers re-mark; cancel never wipes).

### 3.3 The done sweep — declared job

Upstream `_gc_mark_events_done` rides base autovacuum. Backbone has no shared
base-vacuum host, so the sweep ships as the module's own declared job on the
host wall-clock scheduler (the website visitor-gc precedent), with the
deviation recorded:

```yaml
scheduled_jobs:
  - name: event-mark-done-sweep
    posture: pull                      # deviation: upstream rode @api.autovacuum
                                       # (ADR-0020 autovacuum_ride) — no shared host
                                       # vacuum exists; a daily pull is the truthful
                                       # nearest posture. Named here, not silent.
    schedule: "41 3 * * *"             # daily 03:41 UTC (off the :00/:30 marks)
    commit_policy: commit_per_batch
    pickup_lock: true
```

Body: every event with `date_end < now()` not in a `pipe_end` stage gets the
`mark_done` VERB (full write + audit), never a raw UPDATE. Bounded batches
(upstream's unbounded search is not ported); `FOR UPDATE SKIP LOCKED` claims.

---

## 4. Registration — the attendee row (EVM2-1 family)

### 4.1 Columns that carry weight

- `event_id` (real FK, NOT NULL), `event_slot_id` / `event_ticket_id` (real
  FKs, nullable; must belong to the event — EBG-3/EBG-4 as intra-schema CHECK
  constraints, `enforcement: db`); slot MANDATORY when the event
  `is_multi_slots` (same CHECK).
- `active` boolean default true (archivable). **EVM2-4 fixed by construction:
  EVERY seat count AND every mail eligibility filter in this module reads
  `state IN ('open','done') AND active`** — archived rows leave both, together.
- Attendee identity: `name` / `email` / `phone` / `company_name` (plain stored
  fields) + `partner_id` (logical party ref, nullable, "booked by"). The
  upstream one-shot partner-sync computes (fill-only-when-empty) port as
  explicit verb behavior: the create/patch verbs accept attendee fields
  directly; a `sync_from_partner` action fills empty identity fields from the
  linked partner's contact address ONCE, never clobbering hand-edited values.
  **EBB-5 NOT ported**: there is no chatter hook and no cross-event
  partner-sweep — `partner_id` is set by explicit verbs only.
- `state` + the split (§1, §4.3–4.4), `date_closed` (the two-field-split
  companion: stamps now() when state becomes `done` IF empty; never overwrites
  a manual value; hand-clearable).
- `barcode` string — decimal of 8 urandom bytes little-endian (Code128C
  compact), minted at create, readonly, copy=False. **EBG-1: UNIQUE(barcode)
  GLOBAL across all events — `enforcement: db`** (the constraint deliberately
  does NOT include event_id: one scanner desk serves every event). The scan
  DESK verb itself is the next increment's arm (§11) — the column, the
  constraint, and the branch-order table (§11.2) are fixed NOW so that arm
  changes nothing under it.
- The sale seam mirror columns (§1.1): `sale_order_id`, `sale_order_state`,
  `sale_status` — all NULLABLE, all inert until the event_sale arm wires the
  subscription (§11.1).

### 4.2 The bare-branch state machine (upstream's whole machine, preserved)

Hand-set, default `open`; **there is NO `auto_confirm` concept anywhere** (no
flag, no column, no setting — the register row is explicit and this spec does
not reintroduce one); `draft` is the sale-flow special case ONLY. Four
one-liner verbs, any→any, no monotonicity guard, no label inversion:
`set_draft`, `confirm` (→open), `set_done`, `cancel`. The hooks
`state_machines` block declares exactly this surface (initial `open`, the four
transition triggers) and is referenced by the split's bare branch. Reactive
consequences only: seat buckets (open→reserved, done→used), mail eligibility,
`date_closed`.

The ONE auto-advancing path to `done` is the (next-increment) barcode scan;
nothing in core writes `done` except the explicit verb.

### 4.3 The split's sale-composed branch — the truth table (normative)

The compute jointly derives `state` AND `sale_status` from the mirrored
driver, per upstream ES-1. Normative table (driver = `sale_order_state` +
the order's zero-amount fact mirrored at write):

| `sale_order_state` | amount | `sale_status` | `state` |
|---|---|---|---|
| `cancel` | — | (kept) | **`cancel`** (whole linked group) |
| any | ≈ 0 | `free` | draft → **`open`** (heal forward) |
| `sale` | > 0 | **`sold`** | draft/cancel → **`open`** (heal forward) |
| `draft`/`sent` | > 0 | `to_pay` | held **`draft`** |

Two subtleties preserved exactly: (a) **heal-forward only** — `open`/`done`
rows are NEVER demoted to draft by the compute (ES-2); (b) the group
semantics — the mirror columns update per linked order, and the derivation
runs over that order's linked registrations as a group.

**DRAFT-HEAL arms the schedulers**: any transition of a row INTO `open` from
`draft`/`cancel` (create-default-open included) arms the after_sub engines
(§8.2). Re-confirming an already-`open` row never re-triggers; `open`→`done`
never re-runs the arm. The arm rule is stated once, in the registration verb,
and the sale-composed heal rides the same rule (upstream's ES-4 interception
folded into the verb's post-write hook — no flush-ordering dependence).

### 4.4 Registration create/confirm — the seat-taking verb (EBB-1, the core contract)

ONE verb family (`register` — the guarded intake §7 and every internal caller
share it; a second seat-taking path anywhere is a review refusal):

```
BEGIN;
  SELECT * FROM event.events     WHERE id = $event     FOR UPDATE;   -- the event row lock
  -- when the event is_multi_slots AND the registration carries a slot:
  SELECT * FROM event.slots      WHERE id = $slot      FOR UPDATE;   -- the slot row lock too
  count = seat_count(event|slot|ticket)   -- §5: state IN ('open','done') AND active
  refuse typed `event_seats_exhausted` if limited AND count >= cap
  refuse typed `event_sale_window_closed` if the ticket window is shut (WE-4's family, closed module-side at the verb)
  INSERT registration (state per §4.2/§4.3 — default open);
  arm after_sub schedulers for rows entering open;               -- §8.2, in-tx
COMMIT;
```

Count-then-insert inside ONE transaction, lock FIRST. The upstream post-insert
ORM fence and its plain-count race are NOT ported (§12). The lock ordering is
fixed (event → slot → ticket row if capped per-ticket) and documented — every
writer in the module takes them in that order.

### 4.5 Enforcement declarations (ADR-0015)

| guard | port | enforcement |
|---|---|---|
| EBG-1 barcode globally unique | UNIQUE(barcode) | `db` |
| EBG-3/EBG-4 slot/ticket ∈ event (+slot-mandatory-on-multi-slot) | intra-schema CHECKs | `db` |
| EBG-12 default question ⇒ reusable | CHECK | `db` |
| EBG-13 answer value XOR | CHECK | `db` |
| EBG-5 date_end ≥ date_begin; EBG-10 sale-date order; EBG-8 slot hours | CHECKs | `db` |
| EBG-6/EBG-9 slots inside event range | service (set-based) + row-level CHECK for the row arm | `both` |
| EBG-11 per-order cap ≤ min(seats_max, 30) | service pre-check + column CHECK (≥0, ≤30) | `both` |
| EBG-14..18 ondelete protections | real FKs `ondelete RESTRICT` (answers CASCADE where upstream cascades) | `db` |
| EBG-19 question kind immutable once answered | service write-guard (procedural: it guards a WRITE PATH, not a value) | `service` + justification note |
| seat availability (EBB-1) | the verb's FOR UPDATE count-then-insert | `service` + justification note (§5.2) |

EVM-4 ("zero SQL constraints in the whole set") is the upstream state; the
port inverts it — the cluster's SQL-constraint mindset is the default here,
per the pillar. Where `service` is declared, the YAML carries the
justification note ADR-0015 requires.

### 4.6 Time postures (EBB-7 / EBB-11 closed)

`date_begin`/`date_end`/slot datetimes are stored `timestamptz` (UTC
instants) + the event carries `date_tz` (an IANA name) for presentation and
window derivation. ONE derivation path exists: windows (ongoing/finished,
sale windows, scheduler anchors) compare the stored instants against
`now()` in UTC; presentation converts to `date_tz`. The upstream
compute-in-event-TZ vs search-in-naive-UTC split (EBB-7) has no analogue —
there is no second comparison path. No naive `now()` anywhere (EBB-11); no
DST-ambiguity assumption in slot materialization beyond what timestamptz
guarantees (upstream EVM2-7's fold-picks-first behavior is not ported into
any local-time arithmetic — everything is instants).

---

## 5. Seat truth (EBB-1) — ONE aggregation service

### 5.1 The service

ONE seat aggregation service, keyed by FK, is the ONLY seat counter in the
module (upstream EVM-3's triple-copied raw-SQL quartets on event / ticket /
slot are NOT ported — §12):

```
seat_count(scope) =
  COUNT(*) FROM event.registrations
  WHERE <scope predicate>            -- event_id = $e | event_ticket_id = $t | event_slot_id = $s
    AND state IN ('open','done')     -- the counting domain (C17, verbatim)
    AND active                       -- archived rows never count
```

Derived read model (never separately stored): `reserved` = count(open),
`used` = count(done), `taken` = reserved + used, `available` = cap − taken
**only when cap > 0** — **0 (or negative) cap = UNLIMITED, everywhere**
(event, ticket, slot). Multi-slot availability multiplies per-slot capacity by
`event_slot_count` (upstream shape). The event-level "remaining" shown to
operators = min(event cap, all ticket caps) (`available_min`). `is_sold_out`
propagates event → every ticket exactly as upstream. Every surface (verbs,
reads, /ics, the W8 funnel read models §9.4) consumes this ONE service; a
second counter — in this module, a sibling module, or the webapp — is the
review refusal W7-C20 freezes.

### 5.2 The residual, honestly

The seat invariant is enforced in the verb (lock + count-then-insert). A raw
SQL INSERT into `event.registrations` bypasses the verb — the platform-wide
raw-SQL-out-of-contract note applies (such writes stage no event, fire no
arm, and are nobody's contract). Because the truth service always recomputes
from rows, a raw write degrades to availability drift (visible at the next
read), never to a silent oversell THROUGH the verb. This is the ADR-0015
`service` declaration's justification note; the C17 adjudication (lock inside
the verb, no trigger demanded) is the ruling on record.

---

## 6. `/ics` and `my_tickets` (EBB-6) — the public capability surface

The module's ONLY public surface. Two routes, both token-carried, both
throttled, neither id-enumerable (ADR-0018 Tier A + ADR-0019):

### 6.1 Publication — the module-side publish contract

Upstream's `is_published` exists only when website_event installs (W8); the
core addon references it defensively. The port moves the axis INTO the event
row now, because `/ics` needs it at this increment (W7-C17: "publication-
checked against WB-3's publish contract"):

- `events.is_published` boolean default false + `events.date_publish`
  timestamptz nullable — stored, publish-fenced (PATCH carrying either is a
  typed 422 refusal).
- `publish` / `unpublish` are the ONLY writers (explicit fence verbs, the
  WB-3 pattern module-side; audit rows `event_published`/`event_unpublished`/
  `publish_refused`).
- Derived visibility (reads): published AND date-past. The W8 website_event
  surfaces consume this same pair — they do not re-declare a second flag.

### 6.2 `GET /api/v1/event/public/ics/{capability}`

- **Token-carried, never id-carried**: the route segment is a Tier A
  capability, not an event id. Sequential event ids yield NOTHING (the
  negative probe asserts it).
- Capability = HMAC-SHA256(secret, purpose `"event-ics-access"`, payload =
  event_id [+ optional slot scope] + expiry), base64url, constant-time
  verified, **expiry + rotation per the ADR-0018 default** (rotation =
  re-issue from the officer surface; outstanding links die at expiry).
- **Publication-checked**: unpublished, archived, or missing event → one
  uniform typed 404 (`event_not_published` — same body shape for all three;
  no oracle). Published → the iCal body (optionally slot-scoped; slot probing
  requires the SAME token carrying the slot scope — C17).
- WE-2's pairing, ported as ruled: for a PUBLISHED event the token'd URL is
  the gate — visibility is this capability, never a listing fence; there is
  no "unlisted but URL-live" middle state to keep honest.
- Throttled per-identity AND per-IP (fixed windows, `Retry-After`) — a
  capability-check route is an enumeration surface regardless of token
  strength.
- Safe-method clean (ADR-0019): GET serves bytes; it writes nothing.

### 6.3 `GET /api/v1/event/public/my-tickets/{capability}`

The attendee's ticket/badge surface (responsive ticket + badge payloads over
the API; the webapp renders). Capability = HMAC-SHA256(secret, purpose
`"event-registration-ticket-report-access"`, payload = (event_id, sorted
registration_ids)), timing-safe compare — the upstream design kept, with its
ONE documented deviation: **links never expire and are not session-scoped**
(the W7-C17 wording: "my_tickets HMAC capability (consteq, never-expiring
documented)"). Possession of the link IS the access. This is a recorded
deviation from ADR-0018 Tier A's mandatory-expiry rule, kept because the
capability is bound to a ticket already lawfully held and re-issue churn
(OD EBB-upstream: "invitations sent before publishing") buys no security; the
cost (rotation invalidates outstanding links) is documented at the knob.
Throttled like §6.2.

### 6.4 The secret

`EVENT_CAPABILITY_SECRET` (§10.3) — one secret; the two purpose strings
domain-separate the surfaces. Fail-closed: unset → both routes answer typed
503 `event_capability_secret_not_configured`; boot WARN. Never printed.

---

## 7. The guarded intake verb (the W8 funnel contract)

NO public event surface ships in this wave beyond §6 — but the module EXPORTS
the guarded intake verb the W8 funnel will consume (W7-C20: "W8 consumes ONLY
the guarded intake verb — never direct row inserts or a second seat counter").
It exists at core, probe-exercised, so W8 composes rather than invents:

`POST /api/v1/event/intake/{event_id_or_capability}` → the `register` verb
(§4.4) behind:

1. **Typed field allowlist** at parse (`#[serde(deny_unknown_fields)]` — an
   unknown key is a 422, not a dropped column). Admitted: attendee identity
   fields, optional slot/ticket ids, answers. **Identity/authority columns
   are NEVER admitted from intake** — no `partner_id`, no `state`, no
   `sale_*`, no `active` (WE-3's class: crafted rows must not stick).
2. **No partner minting from public input** (WE-6's ban): intake never
   creates party rows; `partner_id` stays NULL until an officer or the sale
   flow links one. Attendee email/name are stored on the registration row
   (upstream shape).
3. **Throttles** — per-identity AND per-IP fixed windows with `Retry-After`
   (the client-IP posture mirrors the family rule: socket address, or the
   RIGHTMOST forwarded hop only under `EVENT_TRUSTED_PROXY` — bool-tolerant
   string, fail-closed default false).
4. **The seat verb** — the same ONE verb, same lock, same refusals.
5. The turnstile/siteverify arm is NOT built here (W8's funnel brings it; the
   website intake engine's four-answer posture is the pattern to consume
   then). Named re-entry in §11.

The intake verb is NOT mounted publicly at core (no W7 public funnel): it
lives on the exported router, exercised by probes, and the host mounts it when
W8 arms the funnel. The officer/admin registration verbs (§9.2) are separate
and company-authed.

---

## 8. The self-arming mail scheduler (ADR-0020; EBB-2/3/4/9/10, EVM2-3/4/11/12)

### 8.1 One row, three engines

`event.mails` carries the scheduler: `interval_nbr` + `interval_unit` +
`interval_kind` (the enum §2.4) + `scheduled_date` + `mail_done` +
`error_datetime`/`error_kind` + the event-based cursor
(`last_registration_id`) + `notification_channel` (single-value `mail` — the
channel seam the sms overlay widens later; the 'sent'=queued contract below is
DOCUMENTED now for both arms) + the template seam (§8.7). Dispatch:

1. Template validity sweep (§8.7) — a dead template ref is a typed visible
   failure, never a silent skip.
2. `after_sub` → the attendee-based engine (per-registration fan-out over
   `event.mail_registrations`).
3. Event-based kinds on a `is_multi_slots` event → **the slot engine**
   (EVM2-11's redirect is DECLARED, not discovered: the same row fans out per
   slot; the parent's `scheduled_date` is recomputed from pending children so
   it is never event-level dead weight — EBB-9 closed).
4. Otherwise → the event-based cursor walk (batched, §8.4).

The before-event window guard ports with its behavior made honest: a
before-event mail whose event is already over is DROPPED, and the drop is
recorded (a receipt row with `dropped_window_closed` — upstream's silent drop
becomes a visible one).

### 8.2 Self-arming (the posture)

```yaml
scheduled_jobs:
  - name: event-mail-scheduler
    posture: self_arming
    # named trigger sources (the interval — daily — is a FLOOR, not the contract):
    #   T1 scheduler-row scheduled_date compute (arm at write/recompute)
    #   T2 registration entering 'open' (create-default-open + draft/cancel→open heal)
    #   T3 batch overflow re-arm (§8.4)
    #   T4 slot-child materialization (slot grain arms like the parent)
    schedule: "*/20 * * * *"     # the floor tick; exact dispatch is event-driven
    commit_policy: commit_per_batch
    pickup_lock: true            # FOR UPDATE SKIP LOCKED claim — lint-enforced
```

The job's claim domain: schedulers on active events, `kanban_state != cancel`,
due (`scheduled_date <= now`), not done — and `after_sub` dies with the event
(`date_end > now`). **Async mode is the ONLY mode** (the upstream
inline-vs-cron ICP switch is not ported): registration verbs ARM in-transaction
(cheap row updates) and NEVER execute the scheduler inline — the inline
SUPERUSER execution path (EVM2-12) is banned (§12); the verb never blocks on
mail (EBB-4's posture).

### 8.3 Receipts, not heuristics (EBB-2 closed)

`event.mail_registrations` is the per-recipient RECEIPT grain for ALL engines:
`(scheduler_id, registration_id)` **UNIQUE** (EVM2-3 fixed — `enforcement:
db`), + `scheduled_date` + `mail_sent` + outcome. Eligibility for receipts and
sends: `state IN ('open','done') AND active` (EVM2-4 closed — archived rows
receive nothing).

- **after_sub**: child rows lazily created for eligible registrations without
  one (chunked, capped); `scheduled_date = registration.created_at +
  interval` — the REAL timing (the parent's own date is the recomputed
  earliest-pending child, §8.1 — EBB-9's nominal anchor is gone).
- **event-based / slot-based**: a send writes the receipt; the cursor
  (`last_registration_id`, per-slot on the slot children) advances past sent
  batches.
- **Completion**: `mail_done` = derived from receipts — TRUE exactly when NO
  eligible registration lacks a sent/settled receipt under the cursor AND the
  cursor is exhausted. **A late registrant re-opens the scheduler**: a new
  eligible row flips completion false again, and the next pass sends to it.
  The upstream seat-count inference (`total_sent >= seats_taken`) is BANNED
  (§12) — completion is registration-state truth read through receipts.
- Cancellation propagation: pending (unsent) receipt rows of registrations
  that left eligibility are DELETED at the next scheduler pass (upstream
  unlink-at-next-pass shape), audited.

### 8.4 Batching and commits (EBB-10 declared)

`commit_policy: commit_per_batch` — batches of `EVENT_MAIL_BATCH` (default 50)
inside a `EVENT_MAIL_CRON_LIMIT` (default 1000) cap per pass; after each
batch: sends staged, cursor advanced, receipts written, COMMIT; overflow →
re-arm (T3) and stop. The at-least-once window per batch is the declared
trade-off (ADR-0020 §5); consumers are idempotent regardless (the receipt
unique + `mail_sent` make re-sends convergent).

### 8.5 Failures typed and visible (EBB-4 closed)

Per-scheduler failures are TYPED (`event_mail_error_kind`: e.g.
`template_unresolved`, `enqueue_refused`, `render_failed`,
`recipient_invalid`) and visible on the row (`error_datetime`, `error_kind`,
cleared on the next successful pass) — one scheduler's failure never stops
the pass (per-scheduler try/catch → record → CONTINUE). The upstream
1/hour notify throttle + chatter degradation is NOT ported (§12): every
distinct failure is recorded; operator notification rides the host's
notification surface at the host's discretion (a WARN, never silence).

### 8.6 'sent' = queued (EBB-3 preserved and documented)

The scheduler hands each mail to the mail queue and marks the receipt
`mail_sent = true` AT ENQUEUE. **'sent' means QUEUED** — downstream
SMTP/queue failures are the mail queue's truth (`MailQueueWriteService`
`mark_sent`/`mark_failed`/`requeue`), not this module's; the scheduler never
retries a transport failure itself and `mail_state`-style reads still report
'sent'. This contract is documented here for the mail arm AND (for the
next-increment sms overlay, same row family) the sms arm — one paragraph, both
channels, no second semantics.

### 8.7 The template seam (EVM2-2 closed)

`template_ref` is a LOGICAL uuid (+ kind, for the future sms arm) — indexed,
no cross-schema FK, no cascade machinery. Resolution goes through a declared,
host-installed port:

```rust
#[async_trait]
pub trait EventTemplateRenderer: Send + Sync {
    /// Render (subject, body) for a scheduler + registration context, or a
    /// typed refusal. The host installs the adapter (over whichever template
    /// store the deployment composes — notification's today).
    async fn render(&self, req: &RenderRequest) -> Result<RenderedParts, TemplateRefused>;
}
```

Unwired → every send refuses typed `template_renderer_not_composed` (a
visible scheduler failure, §8.5) — never a silent skip, never a registration
blocker. There is no hand-rolled cascade to orphan: a deleted template is a
dead uuid that the validity sweep reports. Author fallback chain for the
enqueued mail: organizer → event company → responsible officer (upstream
order). Default seeded templates are NOT shipped (installs inert — the family
rule); officers configure template refs; named in §11.

### 8.8 The substrate dependency (why the heal gates this increment)

Every send lands on `backbone-mail`'s queue (`MailQueueWriteService::enqueue`
— the verified public surface; per-mail custom headers included, which is
also what lets a producer attach RFC 8058 header pairs — that consumer is the
digest/mailing family's leg, NOT this module's: event comms carry no
one-click unsubscribe pair). This is the ONE sibling crate edge (§Identity),
pinned tag-equal to the host's mail pin — healed v0.2.7, fallback v0.2.6 per
W7-C16, never a third copy. The scheduler probes assert against the queue's
staged rows (queued = sent per §8.6).

---

## 9. Routes, mounts, and the exported surface

### 9.1 Mount plan

ONE base: `/api/v1/event`. Two exported pure routers, host-nested (the
website pattern):

- **Public tree** — `event_public_routes(PublicState) -> Router`: §6's two
  capability routes (+ the intake verb's declaration, mounted when the W8
  funnel arms). Mounted BARE of `company_auth` — the capability + throttle
  are the fence.
- **Admin tree** — `event_admin_routes(AdminState) -> Router`: behind
  `company_auth` + `ModuleWriteGate::new(pool, "event")` innermost
  (the foundation_ext pattern verbatim). Authorities: `write:event` /
  `delete:event` / supersets.

The host mounts at its router table (the website mount precedent,
`main.rs` nest); NO double-registration of any base (the boot-panic class).

### 9.2 Admin route table (exhaustive)

```
GET    /admin/events?…                    list (officer sight incl. unpublished + seats read model)
POST   /admin/events                      create (write) — template-apply semantics §3/EVM-2 below
GET    /admin/events/:id                  detail (seat read model included)
PATCH  /admin/events/:id                  typed patch (write) — is_published/date_publish refused (§6.1)
POST   /admin/events/:id/publish          (write) — the fence verb
POST   /admin/events/:id/unpublish        (write)
POST   /admin/events/:id/mark-done        (write) — the pipe_end verb (§3.1)
GET    /admin/events/:id/seats            the ONE seat read model (§5.1) — the only seat numbers anywhere
POST   /admin/registrations               officer create — the register verb (write)
GET    /admin/registrations?event_id=
GET    /admin/registrations/:id
PATCH  /admin/registrations/:id           attendee fields only (write) — state/seam columns refused
POST   /admin/registrations/:id/confirm|set-draft|set-done|cancel   (write) — the four verbs
POST   /admin/registrations/:id/sync-from-partner                   (write) — the one-shot identity fill
GET    /admin/mails?event_id=             scheduler rows + derived mail_state + error state
POST   /admin/mails                       create scheduler row (write)
PATCH  /admin/mails/:id                   interval/template edits (write)
POST   /admin/mails/:id/run               manual trigger (write) — same body as the job, same claims
GET    /admin/stages|types|tags|tickets|slots|questions…             generated CRUD reads
POST/PATCH/DELETE …                        generated writes behind the gate (enforcement: db backs them)
GET    /public/ics/{capability}           §6.2 (no auth nest)
GET    /public/my-tickets/{capability}    §6.3 (no auth nest)
```

### 9.3 Derived reads (virtual lifecycles, declared)

`mail_state` (running/scheduled/sent/error/cancelled) is a NON-STORED derived
read over (`error_datetime`, event `kanban_state`, `mail_done`, per-attendee
progress) — declared `lifecycle: virtual`, exactly upstream's pattern-7
virtual; the stored truth is the receipt/error pair. Event
`is_ongoing`/`is_finished` similarly derive from stored instants (§4.6 — one
path). No second source of truth for any of them.

### 9.4 The exported surface (what W8 / the chain arms / the portal consume)

`EventSurface` (in `src/application/service/event_surface.rs`, re-exported
through `exports/services.rs` — the bulkops/website exports-first precedent:
the block is filled BEFORE any host wiring consumes it):

```rust
#[async_trait]
pub trait EventSurface: Send + Sync {
    /// The ONE seat read model (§5.1). W8's funnel and every family read model
    /// consume THIS — a second seat counter anywhere is the frozen refusal.
    async fn seat_availability(&self, event: Uuid, scope: SeatScope) -> EventResult<SeatAvailability>;
    /// Publication state for URL-gating surfaces (§6.1 pair).
    async fn publication_state(&self, event: Uuid) -> EventResult<PublicationState>;
    /// The guarded intake verb (§7) — the W8 funnel's ONLY write path in.
    async fn register_intake(&self, cmd: IntakeCommand) -> EventResult<RegistrationReceipt>;
    /// Registration state + sale_status read (the split's derived pair) —
    /// the read model the W8 seam contract freezes.
    async fn registration_state(&self, registration: Uuid) -> EventResult<RegistrationStateView>;
    /// Capability minting for officer-issued links (ics scope; my_tickets pairs).
    async fn mint_capability(&self, scope: CapabilityScope, ttl: Option<Duration>) -> EventResult<String>;
    /// The scheduler run verb (the manual trigger and the job share it).
    async fn run_due_schedulers(&self) -> EventResult<RunSummary>;
}
```

Plus the typed ids (EventId, RegistrationId, …) and the two ports
(`EventTemplateRenderer` §8.7; the mail queue is a direct crate call, not a
port). No portal, website, selling, or sapiens type crosses the boundary.

### 9.5 Host compose (the build seat's wiring, mirroring verified patterns)

- seam: `src/infrastructure/seams/event_compose.rs` (website_compose
  precedent): state compose + renderer-port install + wall-clock spawns for
  the two declared jobs (§3.3, §8.2 — feature-gated `compose-events` while
  the pin is dev-path, stripped at the tag swap; `[lints.rust]
  unexpected_cfgs = "deny"` + zero-match grep + probe `--list` count make an
  inert image deterministic-fail — the recorded conversion-integrity gate).
- env-presence WARN loop at boot for `EVENT_CAPABILITY_SECRET`.
- pin: `backbone-events = { git = "https://github.com/faridlab/backbone-events", tag = "v0.1.0" }`
  — unconditional git-tag pin at the swap, added to `COMPOSED_PIN_SET` in
  `scripts/pin-probe.sh` in the same change; the probe's marketing no-edge
  closure must NOT mark `backbone-events → backbone-mail` (events is not a
  marketing-family subject; if the closure's subject set needs updating, that
  is the same change, loudly).
- workspace `metaphor.yaml`: module entry + `serpa-service.depends_on`
  addition; metaphora root manifest the three-line module entry.
- `rls_app_role.sql` re-run as owner after `metaphor migration run-all`.
- NO `outbox_schemas` entry (§Identity). The relay producer sets in BOTH
  `config/application.yml` and `config/application-prod.yml` are untouched.

---

## 10. Migrations, crons, config, Cargo.toml

### 10.1 Migration list (raw-SQL runner order; names/one-liners are the contract)

```
create_event_enums.{up,down}.sql          public-schema enums §2.4 (census re-run pre-landing)
create_event_schema.{up,down}.sql         CREATE SCHEMA event
create_stage_table / create_event_table   stages; events (lifecycle axes, seats, publication pair)
create_event_type_table + create_type_mail_table
create_tag_tables                         tags + tag_categories
create_ticket_table / create_slot_table   caps, windows, per-order cap CHECK
create_question_tables + create_registration_question_rel
create_registration_table                 state/split columns, barcode, identity, seam mirrors
create_registration_answer_table          XOR CHECK
create_mail_table / create_mail_registration_table (UNIQUE (scheduler_id, registration_id))
create_mail_slot_table
add_event_hardening_constraints           seat-CHECK family, belonging CHECKs, cursor/unique polish
add_audit_triggers.up.sql                 metadata timestamp triggers (up-only, family shape)
```

### 10.2 Crons (declared jobs — exactly two)

`event-mail-scheduler` (self_arming, §8.2) and `event-mark-done-sweep`
(pull, §3.3). The sale window is read-time lazy (§2.3); `mail_state` is a
derived read; nothing else owns a clock.

### 10.3 Config knobs (module-owned env, host-declared in BOTH
`deployment/.env.dev.example` and `apps/serpa-service/.env.prod.example`)

| knob | type | default | env var |
|---|---|---|---|
| capability secret | string (secret) | unset → §6 routes 503 fail-closed, boot WARN | `EVENT_CAPABILITY_SECRET` |
| mail batch size | string-parsed u64 | `50` | `EVENT_MAIL_BATCH` |
| mail per-pass cap | string-parsed u64 | `1000` | `EVENT_MAIL_CRON_LIMIT` |
| trusted reverse proxy | bool-tolerant string | unset/false — socket address; `true` resolves the RIGHTMOST forwarded hop | `EVENT_TRUSTED_PROXY` |

Exactly one boolean knob, string-typed with the bool-tolerant parse (the
recorded boot-fix lesson — `${VAR:default}` substitutes text; a bare bool
field crash-loops).

### 10.4 Cargo.toml (dependency block semantics)

```toml
[package]
name = "backbone-events"
version = "0.1.0"
edition = "2021"

[dependencies]
# Framework — single-rev, tag-equal, no branch floats, no [patch]:
backbone-core   = { git = "https://github.com/faridlab/backbone-framework", tag = "v2.7.11", features = ["postgres"] }
backbone-orm    = { git = "https://github.com/faridlab/backbone-framework", tag = "v2.7.11" }
backbone-auth   = { git = "https://github.com/faridlab/backbone-framework", tag = "v2.7.11" }
backbone-rate-limit = { git = "https://github.com/faridlab/backbone-framework", tag = "v2.7.11" }
# The ONE sibling module edge — tag-equal with the HOST's mail pin.
# Healed: v0.2.7 (after the notification-release + host-re-pin train).
# Recorded fallback: v0.2.6 (the existing host island) — NEVER a third copy.
# No gateway features: this module enqueues; transport belongs to the host.
backbone-mail   = { git = "https://github.com/faridlab/backbone-mail", tag = "v0.2.7" }
# backbone-messaging: compile-only, generator-emitted scaffolding import
# (the backbone-website recorded deviation; zero runtime use, no outbox entry).
```

NOT consumed by design — recorded so no later change adds them casually:
`backbone-notification` (the heal vehicle, not a dependency),
`backbone-selling` (seam = domain-event subscription, next increment),
`backbone-sapiens` / `backbone-party` / corporate / geo (logical refs),
`backbone-website` (pattern consumption only), every marketing-tree sibling.

---

## 11. Out of core scope — the named increments (nothing here is quietly built)

1. **event_product + the event_sale composition** — the sellable-ticket arm
   and the seam WIRING: the host subscribes selling's
   `SalesOrderConfirmed`/`SalesOrderCancelled` (the verified carriers) and
   writes the mirror columns (§4.3's table is then live); registrations mint
   through the sale flow (paid backend confirmations mint DRAFT — the
   deliberate upstream rule); the split's sale-composed branch activates on
   rows carrying linkage. The DECLARATION (§1) and the columns land NOW; the
   wiring then changes no schema.
2. **The barcode desk** — `register_attendee` as an authenticated desk verb
   (company_auth + module gate; upstream's bare-`@api`-model ACL-only shape
   is NOT ported — EVM2-6). Branch order is FIXED now (upstream verbatim, the
   load-bearing order): unknown → `invalid_ticket`; `cancel` →
   `canceled_registration`; `draft` → `unconfirmed_registration` (no write);
   finished event → `not_ongoing_event`; event mismatch →
   `need_manual_confirmation` (no write); `open`+match → `set_done` +
   `confirmed_registration`; `done` → `already_registered`. (A done attendee
   at a finished event reports `not_ongoing_event`, never
   `already_registered`.) The barcode carrier decision (framework crate vs
   module-local codec) rides this arm.
3. **Booths, crm, the sms overlay** — per the pillar: booth exclusivity as a
   real partial unique/reservation; crm stored domains as declarative typed
   predicate rows (the `literal_eval` ban is total — §12); sms rides mail's
   `gateway-sms-http` feature directly (the council's mail-gateway-direct
   ruling; the `'sent'`=queued paragraph §8.6 already covers the arm).
4. **W8 surfaces** — the funnel (turnstile arms the intake verb), the
   price/cart tuple, track/quiz server-side scoring, visitors. The seam
   contract: W8 consumes ONLY §9.4's surface (guarded intake, seat read
   model, publication state, the state+sale_status pair) — never direct row
   inserts, never a second seat counter.
5. **Template seeds** — no seeded templates/scheduler lines at core (installs
   inert); officers configure. Re-entry: a seed increment with the
   template-store decision recorded.
6. **UTM attribution** — columns absent (no utm module composed); re-entry
   with the storefront attribution increment.

---

## 12. Banned shapes — verified absent, review refusals

Each is an upstream defect this port REFUSES; a probe or grep asserts absence
where trivial:

1. **The triple-copied raw-SQL seat quartets** (EVM-3) — ONE aggregation
   service (§5.1); no `SELECT … GROUP BY <fk>, state` seat copy exists in any
   other surface.
2. **The post-insert ORM seat fence** (EBB-1) — no seat check runs after an
   unlocked insert; the lock-first verb (§4.4) is the only shape.
3. **The seat-count mail completion heuristic** (EBB-2) — completion reads
   receipts; `>= seats_taken` appears nowhere.
4. **`literal_eval`'d stored domains** (ECR-4) — no stored domain TEXT, no
   eval, ever (the crm arm lands typed predicate rows or not at all).
5. **The honor-system leaderboard** (WT-7) — no client-scored surface (W8's
   note, carried here so the family reads one ban list).
6. **The chatter partner sweep** (EBB-5) — no cross-event mass assignment of
   `partner_id`; explicit verbs only.
7. **Sync HTTP inside computes** (EBB-12 / EVM2-5) — no network I/O in any
   derive/verb path; the map-URL compute is dropped outright.
8. **Inline SUPERUSER scheduler execution** (EVM2-12) — async-only (§8.2).
9. **The 1/hour error-throttle silence** (EBB-4) — typed visible failures
   (§8.5).
10. **The mail domain ignoring `active`** (EVM2-4) — eligibility always
    includes it.
11. **Naive local time** (EBB-11) and the TZ compute/search split (EBB-7) —
    instants + one derivation path (§4.6).
12. **The bare-`@api` public mutate** (EVM2-6) — every verb authenticated or
    capability-gated.
13. **A second seat counter** — anywhere (module, sibling, webapp) — the
    frozen W8 seam refusal.
14. **`auto_confirm`** — no such flag exists; default-open is the machine
    (§4.2).

---

## 13. Register dispositions (the P4 floor rows this increment owns)

The audit surface for the module council; every row cites its floor entry in
`docs/plan/w7-register-deltas.md` §WB-4.

| flag | disposition in this increment |
|---|---|
| EBB-1 🔴 | **ported-closed** — §4.4/§5: ONE aggregation + FOR UPDATE (event row; slot row when multi-slot) INSIDE the verb, count-then-insert one tx |
| EBB-2 | **ported-closed** — §8.3 receipts; late registrants re-open |
| EBB-3 | **preserved+documented** — §8.6 'sent'=queued, both channels |
| EBB-4 | **ported-closed** — §8.5 typed failures; no 1/h throttle |
| EBB-5 | **not-ported-by-decision** — §12.6 |
| EBB-6 🔴 | **ported-closed** — §6: publication-checked, Tier A token, not id-enumerable; slot probing same token |
| EBB-7 / EBB-11 | **ported-closed** — §4.6 instants, one path |
| EBB-8 | **ported-declared** — §3.2 sticky/hybrid; reset is upstream semantics, on record |
| EBB-9 | **ported-closed** — §8.1/§8.3 parent date recomputed from children |
| EBB-10 | **declared** — §8.4 commit_per_batch |
| EBB-12 | **not-ported-by-decision** — §12.7 |
| EVM-1 | **ported** — §3 two axes, no state column |
| EVM-2 | **ported** — template-apply as create-time sync, never re-propagates, destructive-sync protections verbatim |
| EVM-3 | **replaced** — §5.1 (the ONE service) |
| EVM-4 | **inverted** — §4.5 SQL-constraint mindset is the default |
| EVM-5 | **declared read_time_lazy** — §2.3 |
| EVM-6 | **ported-declared** — §3.3 (pull + named deviation) |
| EVM-7 | **moot** — §4.6 one derivation path |
| EVM-8 | **not-ported** — no address_search pseudo-field |
| EVM-9 | **fixed** — `seats_limited` is an explicit boolean, never integer truthiness |
| EVM-10/11 | **not-ported** — `event_url` is an officer-set field with the scheme CHECK (EBG-7 → db); no venue-wipe trap, no one-way compute |
| EVM-12 | **ported** — ONE rel table, two declared views |
| EVM-13 | **not-ported** — §2.1 tags (no random color) |
| EVM-14 | **n/a-core** — chatter ACL carve-out has no chatter surface; desk ACL lands with §11.2 |
| EVM-15 | **declared** — §2.2 shared_blank + posture map |
| EVM2-1/2 | **ported** — §4.2/§8: hand-set default open; the mail coupling preserved (arming at draft→open) |
| EVM2-3 | **fixed** — §8.3 UNIQUE(scheduler_id, registration_id), enforcement: db |
| EVM2-4 | **fixed** — eligibility includes `active` everywhere |
| EVM2-5 | **not-ported** — §12.7 |
| EVM2-6 | **deferred-to-desk-arm** — §11.2 authenticated verb; the branch tree frozen now |
| EVM2-7 | **closed-by-instants** — §4.6 |
| EVM2-8 | **= EBB-2** |
| EVM2-9 | **= EBB-9** |
| EVM2-10 | **= EBB-1** |
| EVM2-11 | **declared** — §8.1 slot redirect explicit |
| EVM2-12 | **not-ported** — §8.2 async-only, no SUPERUSER |
| MAIL-SCHED-SELF-ARMING | **landed** — §8.2 posture + named triggers + SKIP LOCKED + commit_per_batch; gated on the W7-C16 heal (§8.8) |
| DRAFT-HEAL | **landed** — §4.3 the arm rule |
| EP-1..12, ES-1..8 | **census-transcribed at this increment's fleet; BUILT at the next** — the split declaration (§1) is ES-1's schema-gate proof; the remainder (EP saleable tickets, ES seam wiring) is §11.1 |
| EV-1..13 | **floor** — the website_event visitor family; W8's to dispose (no build here) |
| EBF-1/2 | **not-ported** — the dead `portal` dep and unreferenced `base_setup` die with the addon manifest (no dead deps in the Cargo block, §10.4) |
| CRON-POSTURES | **declared** — §10.2 both jobs carry posture |
| CHAIN-CENSUS | **open** — the cohort reads end-to-end at this increment's fleet; findings beyond this spec file as riders |

---

## 14. Gates (the build proves these with exit codes, never output text)

1. **§1 falsifier first**: `metaphor schema generate --force` EXIT=0 +
   byte-stable re-run + `metaphor lint check` EXIT=0 with the split
   declaration present in output — or the reported finding (§1.2).
2. Module suite on scratch 5433: EXIT=0, fail-hard probes, incl.
   `concurrent_registration_zero_oversell` (8+ parallel `register` calls at a
   4-seat event → exactly 4 rows, the rest typed refusals — the pillar DoD
   probe), `/ics` refusal family (unpublished → 404; bad/expired token →
   uniform refusal; sequential-id probing yields nothing), the receipt
   re-open probe (complete scheduler + new late registration → re-opens and
   sends), the arm rule probe (re-confirm does not re-arm; open→done does not
   re-arm), the EVM2-4 probe (archived registration: no seats, no mails).
3. `cargo clippy --all-targets -- -D clippy::expect_used` EXIT=0, zero
   `^error` lines.
4. Byte-stability regen: DRIFT=0.
5. Pin probe (release mode) EXIT=0: `backbone-events` resolves exactly once,
   host-declared, tag-equal with ls-remote; mail single-resolve one copy
   healed / exactly the carved two-copy shape under the fallback (W7-C16's
   own probe wording).
6. Framework single-rev: every framework source line in the lock at v2.7.11.
7. Host compose gates: `cargo check --all-targets` EXIT=0; compose probes on
   scratch; conversion-integrity (zero `compose-events` matches repo-wide
   after the strip; probe `--list` count).
8. Migrations as owner + `rls_app_role.sql` as owner (the dev-grant lesson);
   the app role verified USAGE on schema `event`.
9. Live wire (dev, after the train): `/health` 200; `/api/v1/event/public/ics/<unpublished>`
   → typed 404; `/api/v1/event/admin/…` unauth → 401.
10. version == tag verified INSIDE the tag-cut step (the standing release
    rule); fences declared (§2.2) and ADR-0015 lint truthful.

---

*End of specification. The build seat scaffolds the module tree around this
file's contract; the council audits against §13; nothing outside §2–§10 is
built before its named increment.*
