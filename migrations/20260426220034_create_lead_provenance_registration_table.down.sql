-- Down: drop event.lead_provenance_registrations table
DROP TABLE IF EXISTS event.lead_provenance_registrations CASCADE;
DROP FUNCTION IF EXISTS event.lead_provenance_registrations_audit_timestamp() CASCADE;
