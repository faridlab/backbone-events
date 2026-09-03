-- Down: drop event.lead_rule_predicates table
DROP TABLE IF EXISTS event.lead_rule_predicates CASCADE;
DROP FUNCTION IF EXISTS event.lead_rule_predicates_audit_timestamp() CASCADE;
