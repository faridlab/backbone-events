-- Hand-authored (user-owned). Not regenerated.
--
-- Reverse the tenancy strip: restore the module-native shared_blank fence the module
-- declared before ADR-0029 (nullable company_id; NULL rows visible to every session).
-- Rows the decorator moved to org_unit_id keep their org anchor — this down file only
-- re-adds the column and the policy; it does not move data back.

ALTER TABLE event.events        ADD COLUMN IF NOT EXISTS company_id UUID;
ALTER TABLE event.registrations ADD COLUMN IF NOT EXISTS company_id UUID;

DROP POLICY IF EXISTS events_company_isolation ON event.events;
CREATE POLICY events_company_isolation ON event.events
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL);

DROP POLICY IF EXISTS registrations_company_isolation ON event.registrations;
CREATE POLICY registrations_company_isolation ON event.registrations
    FOR ALL
    USING      (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL)
    WITH CHECK (company_id = NULLIF(current_setting('app.company_id', true), '')::uuid OR company_id IS NULL);
