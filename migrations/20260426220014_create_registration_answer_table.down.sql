-- Down: drop event.registration_answers table
DROP TABLE IF EXISTS event.registration_answers CASCADE;
DROP FUNCTION IF EXISTS event.registration_answers_audit_timestamp() CASCADE;
