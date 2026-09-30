//! Regression guard: the unguarded `all_crud_routes()` composer must NOT mount
//! generic CRUD on the machine-bearing models — Booth, Question and
//! Registration carry hand_set lifecycle fields (booth_state, question state,
//! registration_state): their state columns move only through the module's
//! validated verbs, and a generic full-row PATCH would bypass the declared
//! transition set and the side effects the verbs carry (mail merges, attendee
//! flows, bulk-done shapes). The generator narrows these mounts to the read
//! surface on its own (the default route fn for a hand_set entity is the
//! read-routes builder); this test reads `src/lib.rs` and fails the build if a
//! regen ever re-adds a generic write mount for them, turning a silent reopen
//! into a loud CI failure. Reads stay exposed via the read mounts and
//! `readonly_routes()`.

const LIB_RS: &str = include_str!("../src/lib.rs");

/// The machine-bearing models whose generic write mounts are deliberately
/// excluded from `all_crud_routes`. Each entry is the exact write-mount call
/// site (function + the service field it would be called with).
const EXCLUDED_MACHINE_OWNED_ROUTE_MOUNTS: &[&str] = &[
    "create_booth_routes(self.booth_service",
    "create_question_routes(self.question_service",
    "create_registration_routes(self.registration_service",
];

#[test]
fn all_crud_routes_excludes_machine_owned_models() {
    for mount in EXCLUDED_MACHINE_OWNED_ROUTE_MOUNTS {
        assert!(
            !LIB_RS.contains(mount),
            "regression: `all_crud_routes` mounts a machine-bearing model's write route ({mount}). \
             A schema regen has re-added it. The state columns of Booth/Question/Registration move \
             only through the module's validated verbs.",
        );
    }
}
