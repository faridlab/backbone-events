-- Down: drop event.lead_rules table
DROP TABLE IF EXISTS event.lead_rules CASCADE;
DROP FUNCTION IF EXISTS event.lead_rules_audit_timestamp() CASCADE;
