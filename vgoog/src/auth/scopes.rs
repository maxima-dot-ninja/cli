// Every scope vgoog can ask for, in one place.
//
// Two consumers need the same list and must never disagree: the OAuth consent URL the login flow
// opens, and the domain-wide-delegation entry pasted into the Workspace admin console. A scope
// missing from one of those is a call that fails at runtime with a bare 403 and no clue why.
//
// Every string here was checked against Google's consent endpoint — a typo'd scope is rejected as
// invalid_scope at authorise time, which is a much better place to find out than in production.

/// A group of scopes, so a login can ask for Gmail without also asking for Drive.
pub struct ScopeGroup {
    pub service: &'static str,
    pub scopes: &'static [&'static str],
}

pub const GROUPS: &[ScopeGroup] = &[
    ScopeGroup {
        service: "gmail",
        scopes: &[
            "https://www.googleapis.com/auth/gmail.modify",
            "https://www.googleapis.com/auth/gmail.compose",
            "https://www.googleapis.com/auth/gmail.settings.basic",
            "https://www.googleapis.com/auth/gmail.settings.sharing",
        ],
    },
    ScopeGroup { service: "calendar", scopes: &["https://www.googleapis.com/auth/calendar"] },
    ScopeGroup { service: "drive", scopes: &["https://www.googleapis.com/auth/drive"] },
    ScopeGroup { service: "sheets", scopes: &["https://www.googleapis.com/auth/spreadsheets"] },
    ScopeGroup { service: "docs", scopes: &["https://www.googleapis.com/auth/documents"] },
    ScopeGroup { service: "slides", scopes: &["https://www.googleapis.com/auth/presentations"] },
    ScopeGroup {
        service: "forms",
        scopes: &[
            "https://www.googleapis.com/auth/forms.body",
            "https://www.googleapis.com/auth/forms.responses.readonly",
        ],
    },
    ScopeGroup { service: "tasks", scopes: &["https://www.googleapis.com/auth/tasks"] },
    ScopeGroup { service: "contacts", scopes: &["https://www.googleapis.com/auth/contacts"] },
    ScopeGroup {
        service: "apps_script",
        scopes: &[
            "https://www.googleapis.com/auth/script.projects",
            "https://www.googleapis.com/auth/script.processes",
        ],
    },
    // Workspace-only, and only reachable through a service account with domain-wide delegation.
    // Asking for these on a consumer OAuth consent screen simply fails.
    ScopeGroup {
        service: "admin",
        scopes: &[
            "https://www.googleapis.com/auth/admin.directory.user",
            "https://www.googleapis.com/auth/admin.directory.group",
        ],
    },
];

/// Services that only work under domain-wide delegation — excluded from an OAuth login.
pub const WORKSPACE_ONLY: &[&str] = &["admin"];

pub fn service_names() -> Vec<&'static str> {
    GROUPS.iter().map(|group| group.service).collect()
}

/// The scopes for a set of services. An unknown name is ignored rather than fatal — the caller
/// lists what it got, and a typo costs a missing service, not a failed login.
pub fn for_services(services: &[String]) -> Vec<&'static str> {
    GROUPS
        .iter()
        .filter(|group| services.iter().any(|name| name == group.service))
        .flat_map(|group| group.scopes.iter().copied())
        .collect()
}

/// Everything an interactive OAuth login can legitimately request.
pub fn oauth_default() -> Vec<&'static str> {
    GROUPS
        .iter()
        .filter(|group| !WORKSPACE_ONLY.contains(&group.service))
        .flat_map(|group| group.scopes.iter().copied())
        .collect()
}

/// Everything, including the Workspace-admin scopes a delegated service account can reach.
pub fn all() -> Vec<&'static str> {
    GROUPS.iter().flat_map(|group| group.scopes.iter().copied()).collect()
}
