-- Down: drop event.lead_provenances table
DROP TABLE IF EXISTS event.lead_provenances CASCADE;
DROP FUNCTION IF EXISTS event.lead_provenances_audit_timestamp() CASCADE;
