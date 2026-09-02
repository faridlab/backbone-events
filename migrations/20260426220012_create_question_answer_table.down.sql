-- Down: drop event.question_answers table
DROP TABLE IF EXISTS event.question_answers CASCADE;
DROP FUNCTION IF EXISTS event.question_answers_audit_timestamp() CASCADE;
