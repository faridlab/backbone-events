-- Down: drop event.registration_questions table
DROP TABLE IF EXISTS event.registration_questions CASCADE;
DROP FUNCTION IF EXISTS event.registration_questions_audit_timestamp() CASCADE;
