-- Hand-written DDL hardening (user-owned; see metaphor.codegen.yaml).
--
-- The EBG CHECK family + the cross-table belonging triggers:
--  - date orders (EBG-5/EBG-10): event + slot windows are ordered;
--  - ticket sale-window order + per-order cap range;
--  - slot-mandatory-on-multi-slot + slot/ticket belonging (EBG-3/4)
--    as a trigger (cross-table CHECKs are not expressible);
--  - answer XOR (EBG-13): exactly one value arm per answer;
--  - rel-table exactly-one-axis: a registration question binds to
--    an event OR a type, never both, never neither;
--  - default-question-is-reusable (EBG-12);
--  - the barcode shape (decimal string, Code128C compact).
-- The shared_blank RLS policies on events + registrations are the
-- generated enable_company_rls migration (already the declared
-- posture: company_id = current OR IS NULL).

-- Date orders (EBG-5/EBG-10).
ALTER TABLE event.events
    ADD CONSTRAINT events_date_order CHECK (date_end >= date_begin);
ALTER TABLE event.slots
    ADD CONSTRAINT slots_date_order CHECK (date_end IS NULL OR date_end >= date_begin);
ALTER TABLE event.tickets
    ADD CONSTRAINT tickets_sale_window_order
    CHECK (end_sale_datetime IS NULL OR start_sale_datetime IS NULL
           OR end_sale_datetime >= start_sale_datetime);

-- Multi-slot sanity: at least one slot declared.
ALTER TABLE event.events
    ADD CONSTRAINT events_slot_count_positive CHECK (event_slot_count >= 1);

-- Per-order seat cap range (EBG ticket shape).
ALTER TABLE event.tickets
    ADD CONSTRAINT tickets_seats_per_order_range CHECK (seats_max_per_order BETWEEN 0 AND 30);

-- Answer XOR (EBG-13): exactly one value arm.
ALTER TABLE event.registration_answers
    ADD CONSTRAINT registration_answers_value_xor
    CHECK (num_nonnulls(value_text, value_answer_id) = 1);

-- Rel-table exactly-one-axis: event XOR type, never both, never
-- neither.
ALTER TABLE event.registration_questions
    ADD CONSTRAINT registration_questions_one_axis
    CHECK (num_nonnulls(event_id, event_type_id) = 1);

-- A default question must be reusable (EBG-12): defaults attach to
-- many events; non-reusable defaults would orphan on first detach.
ALTER TABLE event.questions
    ADD CONSTRAINT questions_default_reusable
    CHECK (NOT is_default OR is_reusable);

-- Barcode shape: a decimal string, at most 32 digits (Code128C
-- compact; the global UNIQUE index rides the generated migration).
ALTER TABLE event.registrations
    ADD CONSTRAINT registrations_barcode_shape
    CHECK (barcode ~ '^[0-9]{1,32}$');

-- Cross-table belonging (EBG-3/EBG-4 + slot-mandatory-on-multi-slot):
-- a trigger — a cross-table CHECK is not expressible in Postgres.
CREATE OR REPLACE FUNCTION event.assert_registration_belonging() RETURNS trigger AS $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM event.events e WHERE e.id = NEW.event_id) THEN
        RAISE EXCEPTION 'event % does not exist', NEW.event_id;
    END IF;
    IF (SELECT e.is_multi_slots FROM event.events e WHERE e.id = NEW.event_id)
       AND NEW.event_slot_id IS NULL THEN
        RAISE EXCEPTION 'multi-slot event % requires a slot on every registration', NEW.event_id;
    END IF;
    IF NEW.event_slot_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM event.slots s
         WHERE s.id = NEW.event_slot_id AND s.event_id = NEW.event_id) THEN
        RAISE EXCEPTION 'slot % does not belong to event %', NEW.event_slot_id, NEW.event_id;
    END IF;
    IF NEW.event_ticket_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM event.tickets t
         WHERE t.id = NEW.event_ticket_id AND t.event_id = NEW.event_id) THEN
        RAISE EXCEPTION 'ticket % does not belong to event %', NEW.event_ticket_id, NEW.event_id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS registrations_belonging ON event.registrations;
CREATE TRIGGER registrations_belonging
    BEFORE INSERT OR UPDATE OF event_id, event_slot_id, event_ticket_id
    ON event.registrations
    FOR EACH ROW EXECUTE FUNCTION event.assert_registration_belonging();
