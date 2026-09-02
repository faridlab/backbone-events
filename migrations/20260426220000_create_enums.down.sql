-- Down: drop enum types for event module
DROP TYPE IF EXISTS event_sale_status CASCADE;
DROP TYPE IF EXISTS event_sale_order_state CASCADE;
DROP TYPE IF EXISTS event_registration_state CASCADE;
DROP TYPE IF EXISTS event_question_kind CASCADE;
DROP TYPE IF EXISTS event_mail_error_kind CASCADE;
DROP TYPE IF EXISTS event_notification_channel CASCADE;
DROP TYPE IF EXISTS event_interval_kind CASCADE;
DROP TYPE IF EXISTS event_interval_unit CASCADE;
DROP TYPE IF EXISTS event_audit_event CASCADE;
DROP TYPE IF EXISTS event_badge_format CASCADE;
DROP TYPE IF EXISTS event_kanban_state CASCADE;
