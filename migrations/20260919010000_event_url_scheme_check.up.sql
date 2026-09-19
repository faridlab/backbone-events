-- Hand-written DDL hardening (user-owned; see metaphor.codegen.yaml).
--
-- The event URL guarantee (EBG-7): an external event page is only reachable
-- when it carries a scheme and a host. The rule was declared as a database
-- CHECK in both the specification and the model description, but no constraint
-- was ever created, so 'example.com/party' could be stored and every surface
-- that renders it as a link produced a dead relative URL.
--
-- Shape: scheme + authority, matching the rule this module ports. The scheme
-- itself is not narrowed to http(s): the field holds whatever address the
-- officer publishes, it only has to be an absolute one.
--
-- Existing rows are checked. A stored value with no scheme is exactly the
-- defect this constraint exists to stop, and there is no safe automatic repair
-- for it (prefixing a scheme invents a destination), so such a row must be
-- corrected by its owner before this migration applies.
ALTER TABLE event.events
    ADD CONSTRAINT events_url_scheme
    CHECK (event_url IS NULL
           OR event_url ~ '^[A-Za-z][A-Za-z0-9+.-]*://[^/?#[:space:]]+');
