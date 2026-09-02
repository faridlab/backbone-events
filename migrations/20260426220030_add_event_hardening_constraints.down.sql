-- Down: drop the hardening family in reverse.

DROP TRIGGER IF EXISTS registrations_belonging ON event.registrations;
DROP FUNCTION IF EXISTS event.assert_registration_belonging();

ALTER TABLE event.registrations DROP CONSTRAINT IF EXISTS registrations_barcode_shape;
ALTER TABLE event.questions DROP CONSTRAINT IF EXISTS questions_default_reusable;
ALTER TABLE event.registration_questions DROP CONSTRAINT IF EXISTS registration_questions_one_axis;
ALTER TABLE event.registration_answers DROP CONSTRAINT IF EXISTS registration_answers_value_xor;
ALTER TABLE event.tickets DROP CONSTRAINT IF EXISTS tickets_seats_per_order_range;
ALTER TABLE event.events DROP CONSTRAINT IF EXISTS events_slot_count_positive;
ALTER TABLE event.tickets DROP CONSTRAINT IF EXISTS tickets_sale_window_order;
ALTER TABLE event.slots DROP CONSTRAINT IF EXISTS slots_date_order;
ALTER TABLE event.events DROP CONSTRAINT IF EXISTS events_date_order;
