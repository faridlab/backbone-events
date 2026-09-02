-- Down: drop event.questions table
DROP TABLE IF EXISTS event.questions CASCADE;
DROP FUNCTION IF EXISTS event.questions_audit_timestamp() CASCADE;
