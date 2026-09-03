-- Overlay hardening (hand-written; user-owned; the *hardening* glob in
-- metaphor.codegen.yaml).
--
-- One place for everything the generator cannot express, in dependency
-- order after every overlay table exists (20260426220031-39):
--   1. the overlay's enum widenings (the sms channel + error kind, and
--      the overlay audit vocabulary; existing databases take them
--      here; fresh replays get the narrower base types from 20000 and
--      the SAME widening here — one truth for both);
--   2. the four FK constraints the generator's alphabetical file
--      numbering stranded ahead of their referenced tables (forward
--      references moved out of 20031/20034/20035/20037);
--   3. the booth exclusivity family: EBS-1 partial unique (THE wall),
--      EBS-2 line-grain composite unique;
--   4. the CRM idempotence uniques: (rule_id, group_key) on
--      provenance, (provenance_id, registration_id) on the junction;
--   5. the closed-vocabulary predicate shape CHECKs (ECR-4);
--   6. the seam inbox (exactly-once consumption for the sale seam
--      verbs — PK (consumer, external_id));
--   7. the EP-2 catalog arm (tickets.product_id ALTER + the
--      event_linked_products view over tickets + booth_categories
--      product refs, DISTINCT — the compose-time read for the billing
--      policy default / exclusion predicate; the catalog module is
--      never mutated).

-- 1. Enum widenings (idempotent both for fresh replays and upgrades).
ALTER TYPE event_notification_channel ADD VALUE IF NOT EXISTS 'sms';
ALTER TYPE event_mail_error_kind ADD VALUE IF NOT EXISTS 'sms_enqueue_refused';
-- The overlay audit vocabulary: the schema enum declares the variants
-- (SSOT); the base CREATE TYPE in 20000 predates them and existing
-- databases never re-run it, so the SAME values land here — one truth
-- for both, the notification_channel pattern above.
ALTER TYPE event_audit_event ADD VALUE IF NOT EXISTS 'registration_updated';
ALTER TYPE event_audit_event ADD VALUE IF NOT EXISTS 'sale_seam_confirmed';
ALTER TYPE event_audit_event ADD VALUE IF NOT EXISTS 'sale_seam_cancelled';
ALTER TYPE event_audit_event ADD VALUE IF NOT EXISTS 'sale_seam_paid';
ALTER TYPE event_audit_event ADD VALUE IF NOT EXISTS 'booth_created';
ALTER TYPE event_audit_event ADD VALUE IF NOT EXISTS 'booth_updated';
ALTER TYPE event_audit_event ADD VALUE IF NOT EXISTS 'booth_deleted';
ALTER TYPE event_audit_event ADD VALUE IF NOT EXISTS 'booth_confirmed';
ALTER TYPE event_audit_event ADD VALUE IF NOT EXISTS 'booth_released';
ALTER TYPE event_audit_event ADD VALUE IF NOT EXISTS 'booth_booking_created';
ALTER TYPE event_audit_event ADD VALUE IF NOT EXISTS 'booth_booking_deleted';
ALTER TYPE event_audit_event ADD VALUE IF NOT EXISTS 'lead_rule_created';
ALTER TYPE event_audit_event ADD VALUE IF NOT EXISTS 'lead_rule_updated';
ALTER TYPE event_audit_event ADD VALUE IF NOT EXISTS 'lead_relinked';
ALTER TYPE event_audit_event ADD VALUE IF NOT EXISTS 'template_cascade';

-- 2. The stranded FKs (idempotent adds).
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_booth_bookings_event_booth_id') THEN
        ALTER TABLE event.booth_bookings
            ADD CONSTRAINT fk_booth_bookings_event_booth_id
            FOREIGN KEY (event_booth_id) REFERENCES event.booths (id);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_lead_provenance_registrations_provenance_id') THEN
        ALTER TABLE event.lead_provenance_registrations
            ADD CONSTRAINT fk_lead_provenance_registrations_provenance_id
            FOREIGN KEY (provenance_id) REFERENCES event.lead_provenances (id);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_lead_provenances_rule_id') THEN
        ALTER TABLE event.lead_provenances
            ADD CONSTRAINT fk_lead_provenances_rule_id
            FOREIGN KEY (rule_id) REFERENCES event.lead_rules (id);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_lead_rule_predicates_rule_id') THEN
        ALTER TABLE event.lead_rule_predicates
            ADD CONSTRAINT fk_lead_rule_predicates_rule_id
            FOREIGN KEY (rule_id) REFERENCES event.lead_rules (id);
    END IF;
END
$$;

-- 3. Booth exclusivity.
-- EBS-1: THE wall — at most ONE confirmed booking per booth. The
-- availability pre-check in the confirm verb is a nice-error pre-check;
-- this index is what actually holds under concurrency (the loser's
-- unique violation maps to the typed booth_already_confirmed refusal).
CREATE UNIQUE INDEX IF NOT EXISTS booths_confirmed_exclusivity
    ON event.booth_bookings (event_booth_id) WHERE status = 'confirmed';

-- EBS-2: the booking grain of upstream's checked-in/out columns, kept
-- SQL at its own grain — one row per (line, booth). NULL lines (pending
-- intents) do not collide.
CREATE UNIQUE INDEX IF NOT EXISTS booth_bookings_line_grain
    ON event.booth_bookings (sale_order_line_id, event_booth_id);

-- 4. CRM idempotence uniques.
-- The (rule, registration-set) grain: re-runs of the at-least-once
-- generation job upsert onto the same row instead of duplicating.
CREATE UNIQUE INDEX IF NOT EXISTS lead_provenances_rule_group
    ON event.lead_provenances (rule_id, group_key);

CREATE UNIQUE INDEX IF NOT EXISTS lead_provenance_registrations_pair
    ON event.lead_provenance_registrations (provenance_id, registration_id);

-- 5. The closed-vocabulary predicate shape (ECR-4): the question ref
--    rides the question_answer axis ONLY; value_ids is ALWAYS a JSON
--    array (uuid strings, validated at the verb boundary).
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'lead_rule_predicates_axis_shape') THEN
        ALTER TABLE event.lead_rule_predicates
            ADD CONSTRAINT lead_rule_predicates_axis_shape CHECK (
                (axis = 'question_answer' AND question_id IS NOT NULL)
                OR (axis <> 'question_answer' AND question_id IS NULL)
            );
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'lead_rule_predicates_values_array') THEN
        ALTER TABLE event.lead_rule_predicates
            ADD CONSTRAINT lead_rule_predicates_values_array CHECK (jsonb_typeof(value_ids) = 'array');
    END IF;
END
$$;

-- 6. The seam inbox: exactly-once consumption per (consumer, external
--    delivery id). The sale seam verbs claim their delivery here
--    INSIDE the verb transaction (INSERT .. ON CONFLICT DO NOTHING —
--    no row, no effect: the redelivery is a typed no-op).
CREATE TABLE IF NOT EXISTS event.seam_inbox (
    consumer TEXT NOT NULL,
    external_id TEXT NOT NULL,
    consumed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (consumer, external_id)
);

-- 7. The EP-2 catalog linkage + exclusion view (read-only compose hook
--    — the catalog seat's ratification point; nothing in the catalog
--    module is written). tickets.product_id lands HERE, not in
--    20007: the pinned generator never edits an existing CREATE TABLE
--    migration on a column add (entities regenerate; DDL does not), so
--    the column ALTER lives with the hardening family — same one-truth
--    pattern as the enum widenings above (fresh replays and existing
--    databases take the same statement; booth_categories.product_id
--    needs no ALTER — its table is new at 20032 and born with it).
ALTER TABLE event.tickets ADD COLUMN IF NOT EXISTS product_id UUID;

CREATE OR REPLACE VIEW event.event_linked_products AS
SELECT DISTINCT product_id FROM (
    SELECT product_id FROM event.tickets WHERE product_id IS NOT NULL
    UNION
    SELECT product_id FROM event.booth_categories WHERE product_id IS NOT NULL
) linked;
