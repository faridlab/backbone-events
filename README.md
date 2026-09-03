# backbone-events

The events domain module: registration engine, communication
schedulers, the sale-order seam, booths, typed CRM lead rules, the
sms arm, and the attendance desk. Schema YAML is the single source of
truth; the tree is generator-emitted with hand-owned verbs declared in
`metaphor.codegen.yaml`.

## Documentation map

| doc | what it holds |
|---|---|
| `docs/spec.md` | the core contract (v0.1.x): models, state machine, seat engine, scheduler, capabilities, /ics + my-tickets, banned shapes, register dispositions, gates |
| `docs/spec-overlay.md` | the overlay contract (v0.2.0): sale seam, booths, lead rules, sms, desk, catalog read — and the register rows for every deliberate divergence |
| `metaphor.codegen.yaml` | the regen-safety contract: every hand file, with the reason it is hand-owned |

## Model inventory (19 schema models)

Core (14): `event`, `event_type`, `stage`, `slot`, `ticket`,
`registration`, `registration_answer`, `question`, `tag`, `mail`,
`mail_registration`, `mail_slot`, `type_mail`, `event_audit_log`.

Overlay (5): `booth`, `booth_booking`, `lead_rule`, `lead_request`,
`lead_provenance`.

Five child tables are declared inside their parent's model yaml (they
carry migrations + entities, no model file of their own):
`tag_category` (of tag), `booth_category` + `type_booth` (of the booth
family), `lead_rule_predicate` (of lead_rule),
`lead_provenance_registration` (of lead_provenance). Two hand DDL
objects are NOT models: `event.seam_inbox` (exactly-once consumption)
and `event.event_linked_products` (the catalog read view).

## The verb surface (hand services)

| service | verbs |
|---|---|
| `seat_service` | the ONE seat aggregation |
| `registration_service` | register (lock-first), state verbs, sync-from-partner, archive |
| `event_service` | create (template-apply once), publish/unpublish, mark_done, done sweep, linked products |
| `scheduler_service` | the self-arming pass (mail + sms channel branch), `on_template_deleted` (the one cascade verb) |
| `desk_service` | `register_attendee` — the frozen branch order, exact-match barcode |
| `sale_seam_service` | `on_order_confirmed` / `on_order_paid` / `on_order_cancelled` — exactly-once through the seam inbox |
| `booth_command_service` | booth create/patch/delete fences, booking lifecycle, confirm/release |
| `lead_command_service` | typed rules over the closed predicate vocabulary, answer bridge, read model, relink |
| `lead_generation_service` | the self-arming lead pass (leased claims — one walker per event, batch caps, grouping strategy, host sink) |
| `capability` / `ics_service` / `my_tickets_service` / `intake_service` | Tier A tokens, the /ics gate, attendee reads, the guarded funnel verb |

## Host ports (compose-time wiring)

`EventTemplateRenderer` (template store), `EventMailQueue` (mail
transport), `EventSmsQueue` (sms transport — mail's
gateway-sms-http), `EventLeadSink` (lead create/update). Every port
ships a refusing default that parks loudly — an unwired host is a
typed failure, never a silent skip.

## Probes

`tests/probes/` — fail-hard, one disposable scratch database each on
127.0.0.1:5433 (`EVENT_TEST_ADMIN_URL` overrides), migrations applied
by a raw filename-order runner. Families: seat races (zero oversell),
/ics refusals, scheduler arming + re-open, state-machine fences,
sale-seam mint/heal/cancel, booth exclusivity, typed CRM rules, the
sms overlay, desk branch order.

## Config knobs

`EVENT_CAPABILITY_SECRET` (fail-closed), `EVENT_MAIL_BATCH` /
`EVENT_MAIL_CRON_LIMIT`, `EVENT_LEAD_BATCH` / `EVENT_LEAD_CRON_LIMIT`,
`EVENT_TRUSTED_PROXY`. Declared in the host env templates; no silent
defaults.

## Regeneration safety

`metaphor schema generate --force` regenerates everything except the
`user_owned` globs in `metaphor.codegen.yaml` and the `// <<< CUSTOM`
marker blocks. Never hand-edit a generated file outside a marker —
list the file instead. The manifest's comments state each hand file's
reason (including the two frozen generator defects).
