# The overlay increment (v0.2.0) — contract + register

The events core (v0.1.x, `docs/spec.md`) closed the registration engine,
the scheduler, capabilities, and the /ics + my-tickets reads. This
increment lands the overlay around that core: the sale seam, booths,
the typed CRM lead rules, the sms arm of the scheduler, the desk scan
verb, and the events-owned catalog read. Everything here consumes the
core's frozen seams (§4/§8 of the core spec); nothing reopens them.

The register rows (§7) are the audit surface for the module council —
every deliberate divergence from upstream's event addon family is a
row, cited by flag.

---

## 1. The sale seam (ES-3 / ES-4 / ES-8)

**Shape.** `SaleSeamService` (`sale_seam_service.rs`) exposes three
delivery-driven verbs — `on_order_confirmed`, `on_order_paid`,
`on_order_cancelled` — each exactly-once through the seam inbox
(`event.seam_inbox`, PK `(consumer, external_id)`, claimed with
`INSERT .. ON CONFLICT DO NOTHING` INSIDE the verb transaction: no
row, no effect — a redelivery is a typed no-op). The inbox key is the
host's `delivery_id` when the bridge supplies one, else
`"<verb>:<order_id>"` — the delivery id is the TRUE key: a
re-delivered fact consumes its own row whatever the order has been
through since, where the fallback would swallow a second fact of the
same verb+order (a cancelled order re-confirmed) as already applied.

**The mint fork (ES-3).** `SalesOrderConfirmed` with a zero grand
total mints registrations **born open** (`free`) and armed (the
after_sub engines queue immediately). A non-zero total mints them
**born draft** held at `(sale, to_pay)`, NOT armed — seats are
reserved but nothing sends. An unparseable total classifies PAID (the
fail-safe holds seats rather than arming unpaid).

**The heal (ES-4).** The draft→open transition's arm rule is the
core's (§4.3); the seam's paid verb supplies the fact that gates it:
`on_order_paid` recomputes FORWARD-ONLY — `draft` or `cancel` rows
still at `to_pay` lift to `open` + `sold` + armed; rows already past
`open` never regress; `sale_status` lands `sold` at PAID and only at
PAID. Manual officer verbs stay authoritative — the heal touches only
rows the sale state machine holds.

**The cancel cascade.** `on_order_cancelled` mirrors the cancellation
onto every registration of the order (state→cancel) while KEEPING
`sale_status` (the money truth is the billing module's). A late paid
fact on a cancelled order heals forward (cancel→open+sold): the
recompute never strands a paid attendee.

**The booth latch.** The paid verb also latches `is_paid` on every
booth booked by the order's lines — one-way, survives release.

**Producer note.** The sale outbox producer is HOST-side (the
selling/billing side owns emission); this module owns only the
consumption verbs. See §8 for the two carriers that do not exist yet.

---

## 2. Booths (EBS family)

- **Lifecycle** — a booth is born `available`; a booking is born
  `pending`; `confirm_booking` is the five-write verb (booking
  confirmed, booth `unavailable`, fill-if-empty contact name/email/
  phone); `release_booth` is the only path back to `available` and it
  deletes the booking rows.
- **EBS-1 exclusivity** — a partial unique index
  (`booths_confirmed_exclusivity ON event.booth_bookings
  (event_booth_id) WHERE status='confirmed'`) is THE wall. The
  availability pre-check in the confirm verb is demoted to a
  nice-error; the loser under concurrency maps the unique violation
  to the typed `booth_already_confirmed` (409) carrying the booth id,
  and the losing intent row SURVIVES as pending — a human decides.
- **EBS-2** — one row per (order line, booth): composite unique at the
  booking grain, SQL.
- **EBS-4** — one order line books booths of ONE event (typed
  cross-event refusal).
- **Delete fences** — a booth with booking rows refuses ("release
  first"); a booth ever linked to a sale line refuses outright (the
  sale link is history).
- **Template apply** — the type's `type_booths` rows copy into event
  booths ONCE at event create, whitelisted to `name` +
  `booth_category_id`; booking state, contacts, sale links, and
  `is_paid` never template; later type rows never re-propagate.
- **SO-cancel does not free a booth** (register row, §7): the cancel
  cascade touches registrations only; `release_booth` stays a human
  verb.

---

## 3. The typed CRM lead rules (ECR-4 narrowed / ECS-1 / ECS-2)

**Stored domains are BANNED** (the core's §12.4 ban, total). A rule
carries typed predicate rows over a CLOSED vocabulary —
`event_lead_predicate_axis ∈ {event, event_type, company,
question_answer}` — each row holding `value_ids` as a JSON array of
uuid strings (+ `question_id` on the question_answer axis ONLY; a DB
CHECK enforces the shape). DTOs are `deny_unknown_fields`; the four
axes outside the vocabulary are refused at parse with the
closed-vocabulary message.

**The answer-to-rule bridge.** `from_answer` mints a rule whose single
predicate is the typed (question, [answer]) pair — the operator
"create a lead rule from this answer" verb.

**The queue.** `event.lead_requests` (unique per event) arms at rule
create/activate and at every registration mint/confirm that matches an
armed rule — including the sale seam's mints. Posture `self_arming`,
claim = the LEASE (one UPDATE stamps `claimed_at` per row; SKIP LOCKED
keeps simultaneous claims from waiting inside the statement, and the
lease column holds the exclusion for the walker's WHOLE per-event pass
— the cron pass and the officer run verb share it; finish clears the
lease, a dead walker's lease expires at 900s and the next tick re-runs
at-least-once), batch 200 (`EVENT_LEAD_BATCH`), pass
cap 1000 (`EVENT_LEAD_CRON_LIMIT`), commit per request.

**The pass.** For each claimed request: per active rule, walk eligible
registrations (state open/done, active, anti-joined against the
rule's provenance) in batches until drained; group; sink. **Grouping
is strategy, not schema (ECS-1):** sale-linked registrations group
`per_order` (group_key = the sale order uuid), walk-ins group
`per_event_day` (`{event_id}:{YYYY-MM-DD}`) — both grains in one
pass, surfaced separately in the rule read model (ECS-2). No grouping
column exists.

**Provenance + the sink.** `event.lead_provenances` (unique
`(rule_id, group_key)`) upserts the group; the junction table
(`(provenance_id, registration_id)` unique) makes re-runs walk only
NEW rows. A first-seen group calls the host sink's `create_lead` and
records the returned lead id; a grown group calls `update_lead`. **A
sink refusal parks the request loudly** — `done` stays false,
`error_detail` carries the reason, the next tick retries; never a
silent skip, never a dropped group. The sink is a declared host port
(`EventLeadSink`); the refusing default is the unwired-host arm. **No
lead schema is mutated from this module** — the host's merge calls the
seam's `relink_lead(old, new)` verb, which moves provenance rows in
one update.

**Import exemption** — bulk imports bypass lead arming at the declared
bulkops path (register row): mass data loads do not queue CRM work.

---

## 4. The sms arm (mail-gateway-direct / ESM-2 / EVM2-2+ESM-1)

Mail's `gateway-sms-http` transport is the host's; this module routes
sms through a declared port (`EventSmsQueue`), never through the mail
queue. Scheduler rows carry `notification_channel` (derived from the
template kind at template-apply); the scheduler's receipt walk branches
on it — mail rows enqueue (email, unchanged core behavior), sms rows
enqueue `(phone, body_text)` through the sms port. `'sent'`=queued on
both arms (the core §8.6 paragraph).

**The unconfigured gateway parks loudly (ESM-2):** the refusing
default records the typed failure `sms_enqueue_refused`, leaves the
receipt unsent, and keeps the scheduler open — the row is parked, not
dropped, and the next tick after wiring delivers it. A registration
with no phone is the typed `recipient_invalid` failure.

**ONE template cascade verb** for both channels (EVM2-2 + ESM-1,
collapsed): `on_template_deleted(template_kind, template_ref)` deletes
`event.mails` + `event.type_mails` rows of the (kind, ref) pair,
set-based, audited. This is a DECLARED deviation from a literal
DB-level `ON DELETE CASCADE`: the template store lives across the
module boundary (no FK can exist there), so the cascade is a verb the
template side declares and calls — one verb, both channels, kind-
discriminated (register row, §7).

---

## 5. The desk (EBT-3 / EBG-1 / EVM2-6)

`register_attendee(barcode, event_id, actor)` is an authenticated
officer verb (the upstream bare-`@api` shape is not ported — core
§12.12). **The branch order is FROZEN** (upstream verbatim, the
load-bearing order):

```
unknown/inactive barcode -> DeskInvalidTicket
cancel                  -> DeskCanceledRegistration
draft                   -> DeskUnconfirmedRegistration   (no write)
event not ongoing       -> DeskNotOngoingEvent           (kanban done/cancel or date_end past)
event mismatch          -> DeskNeedManualConfirmation    (no write)
done                    -> DeskAlreadyRegistered
open                    -> the ONE write: done via the transition verb (+ date_closed)
```

A done attendee at a finished event reports not-ongoing, never
already-registered; the draft and cross-event branches write NOTHING.

**The barcode carrier (EBG-1):** urandom decimal digits, GLOBAL
`UNIQUE(barcode)`, minted in-crate at registration — self-contained,
no framework barcode crate, no codec. The desk lookup is EXACT-MATCH
(`find_by_barcode(trimmed)`); a prefix or near-miss is an invalid
ticket, never a LIKE hit. Upstream's nomenclature machinery (sequence
per event + display formatting) is FENCED OUT — register row, §7.

---

## 6. The catalog read (EP-2) + posture map (EVM-15)

**EP-2 — events-owned linkage, zero catalog mutation.**
`event.event_linked_products` (a VIEW: DISTINCT product refs of
tickets + booth_categories) plus the `linked_products()` read are the
events-owned side. The catalog module is NEVER mutated from here; the
billing-policy default and the exclusion predicate ratify at compose
time against this read (the catalog seat's contract).

**EVM-15 posture map** — `shared_blank` on Event, Registration,
Ticket; `none` on the twelve config/child models (types, stages,
slots, tags, questions + answers, mails family, booths family, lead
family, audit log). Ticket is COMPANY-LESS: upstream's ticket
company-level rule is not ported — the shared_blank posture is the
attendee-context default, and a company column may ride a later
additive migration when a consumer exists (owner-ratified at the
v0.2.0 tag review). The three global ir.rules port as DECLARED POLICY
at the host's compose (never RLS rows) — register row, §7.

---

## 7. Register rows (this increment's deviations + decisions)

| flag | disposition |
|---|---|
| EBS-3 | **inverted** — first-confirm-wins + the loud typed loser (intent kept pending). Upstream's collateral competitor-order-cancel (confirming one booking CANCELS the competitor's whole sale order) is NOT ported: an event module never writes another module's sale documents. |
| EBS-1 | **ported** — the partial unique index is THE wall; the availability pre-check is demoted to a nice-error (§2). |
| EBS-2 | **ported** — SQL composite unique at the booking grain (§2). |
| SO-CANCEL-BOOTH | **not-ported-by-decision** — SO-cancel cascades registrations only; `release_booth` stays a human verb (§2). |
| ECR-4 | **narrowed** — typed predicate rows over the closed four-axis vocabulary; stored domain TEXT + eval banned totally (§3; core §12.4). |
| ECR-3 | **dropped** — the upstream lead-crm sale-order wrapper is not ported; the seam's mint/heal verbs carry the sale linkage. |
| LEAD-TYPE-SPLIT | **dropped** — no lead-side model exists; the linkage lives in `event.lead_provenances.lead_id` + the host sink port (§3). Upstream's model census counts one more model than this port builds for exactly this reason; the merge-relink verb replaces the split's merge handling. |
| EBT-7 | **dropped** — the desk nomenclature/sequence machinery is not ported; urandom decimal + exact-match (§5). |
| EBT-3 / EVM2-6 | **ported** — the authenticated desk verb with the frozen branch order (§5). |
| EBG-1 | **decided** — barcode = urandom decimal digits, globally unique, exact-match lookup; NO framework barcode crate, NO codec (self-contained in-crate; §5). |
| EVM2-2 + ESM-1 | **collapsed + deviated** — ONE template-cascade VERB for mail+sms (kind-discriminated), declared INSTEAD of a literal DB-level `ON DELETE CASCADE` (no FK can cross the template-store boundary; §4). |
| ESM-2 | **ported** — unconfigured gateway parks loudly: typed `sms_enqueue_refused`, receipt unsent, scheduler open, retried (§4). |
| ECS-1 | **ported as strategy** — grouping is code, not a stored column (§3). |
| ECS-2 | **ported** — both grouping grains surfaced in the rule read model (§3). |
| IMPORT-EXEMPT | **declared** — the bulkops import path bypasses lead arming (§3). |
| TICKET-POSTURE | **owner-RATIFIED at the v0.2.0 tag review** — Ticket carries `shared_blank` posture COMPANY-LESS (the attendee's company context), diverging from upstream's event-global company blanket; a company column may ride a later additive migration when a consumer exists (§6). |
| REGISTRATION-COMPANY | **declared writable** — `registration.company_id` stays an officer-writable column (the upstream computed-from-partner shape is not ported); identity columns remain guarded by the intake allowlist (core §7). |
| RLS-VS-POLICY | **declared** — the three global ir.rules port as declared host compose POLICY, never as RLS rows in this module's migrations (§6). The ambiguity upstream (ir.rule vs record rule) resolves to policy here. |
| MODEL-COUNT | **declared** — 19 schema models at this increment (14 core + 5 overlay: booth, booth_booking, lead_rule, lead_request, lead_provenance), plus four generator child tables (booth_category, type_booth, lead_rule_predicate, lead_provenance_registration) and two hand DDL objects (seam_inbox TABLE, event_linked_products VIEW — not models). The census's +1 is the dropped lead_type split (LEAD-TYPE-SPLIT row). |

---

## 8. Cross-module prerequisites (verbs now, wiring at compose)

Two declared carriers do not exist yet on the producing side — the
consumption verbs land NOW and compose later changes no schema:

1. **The billing paid hook** — `backbone-billing` currently declares
   `events: {}` (no outbox emission). `on_order_paid` waits on that
   producer; the heal it drives is inert until then (forward-only,
   so late wiring is safe by construction).
2. **The lead declared seam** — `backbone-lead` currently declares
   `events: {}`. The sink port + provenance rows are ready; until the
   host composes a sink, requests park loudly with the refusing
   default's reason (§3).

Both are host-compose work, recorded here so the council tracks the
wiring as its own step.

---

## 9. Not in this increment (unchanged from core §11)

The W8 surfaces (funnel, cart, track/quiz scoring, visitors), template
seeds, and UTM attribution remain named re-entries. The sale outbox
PRODUCER is host-side by design (§1). The sms gateway transport
(host `gateway-sms-http`) is mail's, not this module's.
