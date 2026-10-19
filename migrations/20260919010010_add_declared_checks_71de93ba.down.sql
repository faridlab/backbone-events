-- Down: drop the declared rule constraints for event module
ALTER TABLE IF EXISTS event.type_mails DROP CONSTRAINT IF EXISTS ck_type_mails_template_kind_max_length__max_length;
ALTER TABLE IF EXISTS event.type_mails DROP CONSTRAINT IF EXISTS ck_type_mails_interval_nbr_non_negative__non_negative;
ALTER TABLE IF EXISTS event.type_mails DROP CONSTRAINT IF EXISTS ck_type_mails_event_type_id_required__required;
ALTER TABLE IF EXISTS event.tickets DROP CONSTRAINT IF EXISTS ck_tickets_seats_max_per_order_range__range;
ALTER TABLE IF EXISTS event.tickets DROP CONSTRAINT IF EXISTS ck_tickets_seats_max_non_negative__non_negative;
ALTER TABLE IF EXISTS event.tickets DROP CONSTRAINT IF EXISTS ck_tickets_name_max_length__max_length;
ALTER TABLE IF EXISTS event.tickets DROP CONSTRAINT IF EXISTS ck_tickets_name_required__required;
ALTER TABLE IF EXISTS event.tickets DROP CONSTRAINT IF EXISTS ck_tickets_event_id_required__required;
ALTER TABLE IF EXISTS event.tags DROP CONSTRAINT IF EXISTS ck_tags_color_max_length__max_length;
ALTER TABLE IF EXISTS event.tags DROP CONSTRAINT IF EXISTS ck_tags_name_max_length__max_length;
ALTER TABLE IF EXISTS event.tags DROP CONSTRAINT IF EXISTS ck_tags_name_required__required;
ALTER TABLE IF EXISTS event.tag_categories DROP CONSTRAINT IF EXISTS ck_tag_categories_name_max_length__max_length;
ALTER TABLE IF EXISTS event.tag_categories DROP CONSTRAINT IF EXISTS ck_tag_categories_name_required__required;
ALTER TABLE IF EXISTS event.stages DROP CONSTRAINT IF EXISTS ck_stages_sequence_non_negative__non_negative;
ALTER TABLE IF EXISTS event.stages DROP CONSTRAINT IF EXISTS ck_stages_name_max_length__max_length;
ALTER TABLE IF EXISTS event.stages DROP CONSTRAINT IF EXISTS ck_stages_name_required__required;
ALTER TABLE IF EXISTS event.slots DROP CONSTRAINT IF EXISTS ck_slots_name_max_length__max_length;
ALTER TABLE IF EXISTS event.slots DROP CONSTRAINT IF EXISTS ck_slots_name_required__required;
ALTER TABLE IF EXISTS event.slots DROP CONSTRAINT IF EXISTS ck_slots_event_id_required__required;
ALTER TABLE IF EXISTS event.registration_answers DROP CONSTRAINT IF EXISTS ck_registration_answers_value_text_max_length__max_length;
ALTER TABLE IF EXISTS event.registration_answers DROP CONSTRAINT IF EXISTS ck_registration_answers_question_id_required__required;
ALTER TABLE IF EXISTS event.registration_answers DROP CONSTRAINT IF EXISTS ck_registration_answers_registration_id_required__required;
DO $$
BEGIN
    IF to_regclass('event.uq_registration_answers_question_id__unique') IS NOT NULL AND to_regclass('event.idx_registration_answers_registration_id_question_id') IS NULL THEN
        ALTER INDEX event.uq_registration_answers_question_id__unique RENAME TO idx_registration_answers_registration_id_question_id;
    END IF;
END $$;

ALTER TABLE IF EXISTS event.registrations DROP CONSTRAINT IF EXISTS ck_registrations_barcode_max_length__max_length;
ALTER TABLE IF EXISTS event.registrations DROP CONSTRAINT IF EXISTS ck_registrations_barcode_required__required;
ALTER TABLE IF EXISTS event.registrations DROP CONSTRAINT IF EXISTS ck_registrations_company_name_max_length__max_length;
ALTER TABLE IF EXISTS event.registrations DROP CONSTRAINT IF EXISTS ck_registrations_phone_max_length__max_length;
ALTER TABLE IF EXISTS event.registrations DROP CONSTRAINT IF EXISTS ck_registrations_email_email__email;
ALTER TABLE IF EXISTS event.registrations DROP CONSTRAINT IF EXISTS ck_registrations_email_max_length__max_length;
ALTER TABLE IF EXISTS event.registrations DROP CONSTRAINT IF EXISTS ck_registrations_name_max_length__max_length;
ALTER TABLE IF EXISTS event.registrations DROP CONSTRAINT IF EXISTS ck_registrations_event_id_required__required;
DO $$
BEGIN
    IF to_regclass('event.uq_registrations_barcode__unique') IS NOT NULL AND to_regclass('event.idx_registrations_barcode') IS NULL THEN
        ALTER INDEX event.uq_registrations_barcode__unique RENAME TO idx_registrations_barcode;
    END IF;
END $$;

ALTER TABLE IF EXISTS event.registration_questions DROP CONSTRAINT IF EXISTS ck_registration_questions_sequence_non_negative__non_negative;
ALTER TABLE IF EXISTS event.registration_questions DROP CONSTRAINT IF EXISTS ck_registration_questions_question_id_required__required;
DO $$
BEGIN
    IF to_regclass('event.uq_registration_questions_question_id_44a7dd90__unique') IS NOT NULL AND to_regclass('event.idx_registration_questions_event_type_id_question_id') IS NULL THEN
        ALTER INDEX event.uq_registration_questions_question_id_44a7dd90__unique RENAME TO idx_registration_questions_event_type_id_question_id;
    END IF;
END $$;

DO $$
BEGIN
    IF to_regclass('event.uq_registration_questions_question_id__unique') IS NOT NULL AND to_regclass('event.idx_registration_questions_event_id_question_id') IS NULL THEN
        ALTER INDEX event.uq_registration_questions_question_id__unique RENAME TO idx_registration_questions_event_id_question_id;
    END IF;
END $$;

ALTER TABLE IF EXISTS event.question_answers DROP CONSTRAINT IF EXISTS ck_question_answers_sequence_non_negative__non_negative;
ALTER TABLE IF EXISTS event.question_answers DROP CONSTRAINT IF EXISTS ck_question_answers_name_max_length__max_length;
ALTER TABLE IF EXISTS event.question_answers DROP CONSTRAINT IF EXISTS ck_question_answers_name_required__required;
ALTER TABLE IF EXISTS event.question_answers DROP CONSTRAINT IF EXISTS ck_question_answers_question_id_required__required;
ALTER TABLE IF EXISTS event.questions DROP CONSTRAINT IF EXISTS ck_questions_question_max_length__max_length;
ALTER TABLE IF EXISTS event.questions DROP CONSTRAINT IF EXISTS ck_questions_question_required__required;
ALTER TABLE IF EXISTS event.mail_slots DROP CONSTRAINT IF EXISTS ck_mail_slots_slot_id_required__required;
ALTER TABLE IF EXISTS event.mail_slots DROP CONSTRAINT IF EXISTS ck_mail_slots_scheduler_id_required__required;
DO $$
BEGIN
    IF to_regclass('event.uq_mail_slots_slot_id__unique') IS NOT NULL AND to_regclass('event.idx_mail_slots_scheduler_id_slot_id') IS NULL THEN
        ALTER INDEX event.uq_mail_slots_slot_id__unique RENAME TO idx_mail_slots_scheduler_id_slot_id;
    END IF;
END $$;

ALTER TABLE IF EXISTS event.mail_registrations DROP CONSTRAINT IF EXISTS ck_mail_registrations_outcome_max_length__max_length;
ALTER TABLE IF EXISTS event.mail_registrations DROP CONSTRAINT IF EXISTS ck_mail_registrations_registration_id_required__required;
ALTER TABLE IF EXISTS event.mail_registrations DROP CONSTRAINT IF EXISTS ck_mail_registrations_scheduler_id_required__required;
DO $$
BEGIN
    IF to_regclass('event.uq_mail_registrations_registration_id__unique') IS NOT NULL AND to_regclass('event.idx_mail_registrations_scheduler_id_registration_id') IS NULL THEN
        ALTER INDEX event.uq_mail_registrations_registration_id__unique RENAME TO idx_mail_registrations_scheduler_id_registration_id;
    END IF;
END $$;

ALTER TABLE IF EXISTS event.mails DROP CONSTRAINT IF EXISTS ck_mails_template_kind_max_length__max_length;
ALTER TABLE IF EXISTS event.mails DROP CONSTRAINT IF EXISTS ck_mails_interval_nbr_non_negative__non_negative;
ALTER TABLE IF EXISTS event.mails DROP CONSTRAINT IF EXISTS ck_mails_event_id_required__required;
ALTER TABLE IF EXISTS event.lead_rule_predicates DROP CONSTRAINT IF EXISTS ck_lead_rule_predicates_rule_id_required__required;
ALTER TABLE IF EXISTS event.lead_rules DROP CONSTRAINT IF EXISTS ck_lead_rules_name_max_length__max_length;
ALTER TABLE IF EXISTS event.lead_rules DROP CONSTRAINT IF EXISTS ck_lead_rules_name_required__required;
ALTER TABLE IF EXISTS event.lead_requests DROP CONSTRAINT IF EXISTS ck_lead_requests_error_detail_max_length__max_length;
ALTER TABLE IF EXISTS event.lead_requests DROP CONSTRAINT IF EXISTS ck_lead_requests_event_id_required__required;
DO $$
BEGIN
    IF to_regclass('event.uq_lead_requests_event_id__unique') IS NOT NULL AND to_regclass('event.idx_lead_requests_event_id') IS NULL THEN
        ALTER INDEX event.uq_lead_requests_event_id__unique RENAME TO idx_lead_requests_event_id;
    END IF;
END $$;

ALTER TABLE IF EXISTS event.lead_provenance_registrations DROP CONSTRAINT IF EXISTS ck_lead_provenance_registrations_registratio_404c9c48__required;
ALTER TABLE IF EXISTS event.lead_provenance_registrations DROP CONSTRAINT IF EXISTS ck_lead_provenance_registrations_provenance__fc819d42__required;
ALTER TABLE IF EXISTS event.lead_provenances DROP CONSTRAINT IF EXISTS ck_lead_provenances_group_key_max_length__max_length;
ALTER TABLE IF EXISTS event.lead_provenances DROP CONSTRAINT IF EXISTS ck_lead_provenances_group_key_required__required;
ALTER TABLE IF EXISTS event.lead_provenances DROP CONSTRAINT IF EXISTS ck_lead_provenances_event_id_required__required;
ALTER TABLE IF EXISTS event.lead_provenances DROP CONSTRAINT IF EXISTS ck_lead_provenances_rule_id_required__required;
ALTER TABLE IF EXISTS event.types DROP CONSTRAINT IF EXISTS ck_types_default_date_tz_max_length__max_length;
ALTER TABLE IF EXISTS event.types DROP CONSTRAINT IF EXISTS ck_types_default_date_tz_required__required;
ALTER TABLE IF EXISTS event.types DROP CONSTRAINT IF EXISTS ck_types_seats_max_non_negative__non_negative;
ALTER TABLE IF EXISTS event.types DROP CONSTRAINT IF EXISTS ck_types_description_max_length__max_length;
ALTER TABLE IF EXISTS event.types DROP CONSTRAINT IF EXISTS ck_types_name_max_length__max_length;
ALTER TABLE IF EXISTS event.types DROP CONSTRAINT IF EXISTS ck_types_name_required__required;
ALTER TABLE IF EXISTS event.events DROP CONSTRAINT IF EXISTS ck_events_event_url_url__url;
ALTER TABLE IF EXISTS event.events DROP CONSTRAINT IF EXISTS ck_events_seats_max_non_negative__non_negative;
ALTER TABLE IF EXISTS event.events DROP CONSTRAINT IF EXISTS ck_events_event_slot_count_range__range;
ALTER TABLE IF EXISTS event.events DROP CONSTRAINT IF EXISTS ck_events_date_tz_max_length__max_length;
ALTER TABLE IF EXISTS event.events DROP CONSTRAINT IF EXISTS ck_events_date_tz_required__required;
ALTER TABLE IF EXISTS event.events DROP CONSTRAINT IF EXISTS ck_events_stage_id_required__required;
ALTER TABLE IF EXISTS event.events DROP CONSTRAINT IF EXISTS ck_events_name_max_length__max_length;
ALTER TABLE IF EXISTS event.events DROP CONSTRAINT IF EXISTS ck_events_name_required__required;
ALTER TABLE IF EXISTS event.booth_bookings DROP CONSTRAINT IF EXISTS ck_booth_bookings_contact_phone_max_length__max_length;
ALTER TABLE IF EXISTS event.booth_bookings DROP CONSTRAINT IF EXISTS ck_booth_bookings_contact_email_email__email;
ALTER TABLE IF EXISTS event.booth_bookings DROP CONSTRAINT IF EXISTS ck_booth_bookings_contact_email_max_length__max_length;
ALTER TABLE IF EXISTS event.booth_bookings DROP CONSTRAINT IF EXISTS ck_booth_bookings_contact_name_max_length__max_length;
ALTER TABLE IF EXISTS event.booth_bookings DROP CONSTRAINT IF EXISTS ck_booth_bookings_event_booth_id_required__required;
ALTER TABLE IF EXISTS event.type_booths DROP CONSTRAINT IF EXISTS ck_type_booths_name_max_length__max_length;
ALTER TABLE IF EXISTS event.type_booths DROP CONSTRAINT IF EXISTS ck_type_booths_name_required__required;
ALTER TABLE IF EXISTS event.type_booths DROP CONSTRAINT IF EXISTS ck_type_booths_booth_category_id_required__required;
ALTER TABLE IF EXISTS event.type_booths DROP CONSTRAINT IF EXISTS ck_type_booths_event_type_id_required__required;
ALTER TABLE IF EXISTS event.booths DROP CONSTRAINT IF EXISTS ck_booths_contact_phone_max_length__max_length;
ALTER TABLE IF EXISTS event.booths DROP CONSTRAINT IF EXISTS ck_booths_contact_email_email__email;
ALTER TABLE IF EXISTS event.booths DROP CONSTRAINT IF EXISTS ck_booths_contact_email_max_length__max_length;
ALTER TABLE IF EXISTS event.booths DROP CONSTRAINT IF EXISTS ck_booths_contact_name_max_length__max_length;
ALTER TABLE IF EXISTS event.booths DROP CONSTRAINT IF EXISTS ck_booths_name_max_length__max_length;
ALTER TABLE IF EXISTS event.booths DROP CONSTRAINT IF EXISTS ck_booths_name_required__required;
ALTER TABLE IF EXISTS event.booths DROP CONSTRAINT IF EXISTS ck_booths_booth_category_id_required__required;
ALTER TABLE IF EXISTS event.booths DROP CONSTRAINT IF EXISTS ck_booths_event_id_required__required;
ALTER TABLE IF EXISTS event.booth_categories DROP CONSTRAINT IF EXISTS ck_booth_categories_description_max_length__max_length;
ALTER TABLE IF EXISTS event.booth_categories DROP CONSTRAINT IF EXISTS ck_booth_categories_name_max_length__max_length;
ALTER TABLE IF EXISTS event.booth_categories DROP CONSTRAINT IF EXISTS ck_booth_categories_name_required__required;
