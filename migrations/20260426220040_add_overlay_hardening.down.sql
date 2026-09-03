-- Down: drop the overlay hardening family in reverse. The enum
-- widenings cannot be rolled back in PostgreSQL (ALTER TYPE .. DROP
-- VALUE does not exist); reverting the code path is the real down for
-- those — the extra enum values are inert without the v0.2.0 verbs.

DROP VIEW IF EXISTS event.event_linked_products;

DROP TABLE IF EXISTS event.seam_inbox;

ALTER TABLE event.lead_rule_predicates DROP CONSTRAINT IF EXISTS lead_rule_predicates_values_array;
ALTER TABLE event.lead_rule_predicates DROP CONSTRAINT IF EXISTS lead_rule_predicates_axis_shape;

DROP INDEX IF EXISTS event.lead_provenance_registrations_pair;
DROP INDEX IF EXISTS event.lead_provenances_rule_group;

DROP INDEX IF EXISTS event.booth_bookings_line_grain;
DROP INDEX IF EXISTS event.booths_confirmed_exclusivity;

ALTER TABLE event.lead_rule_predicates DROP CONSTRAINT IF EXISTS fk_lead_rule_predicates_rule_id;
ALTER TABLE event.lead_provenances DROP CONSTRAINT IF EXISTS fk_lead_provenances_rule_id;
ALTER TABLE event.lead_provenance_registrations DROP CONSTRAINT IF EXISTS fk_lead_provenance_registrations_provenance_id;
ALTER TABLE event.booth_bookings DROP CONSTRAINT IF EXISTS fk_booth_bookings_event_booth_id;
