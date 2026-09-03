-- Down: drop event.lead_requests table
DROP TABLE IF EXISTS event.lead_requests CASCADE;
DROP FUNCTION IF EXISTS event.lead_requests_audit_timestamp() CASCADE;
