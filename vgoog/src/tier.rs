// Trust tiers: whose mailbox an account is, and so whether vgoog may send from it.
//
// A `user` account is a person's own. The assistant works in it on their behalf — reads, labels,
// archives, drafts — but mail that goes out under a person's name is theirs to send. An `ai`
// account is the assistant's own mailbox, and sending from it is the point of having one.
//
// Google's scopes cannot draw that line. `gmail.modify` — the scope that labels, archives and
// trashes — also sends mail, and no scope does one without the other. So the tier is enforced
// here, at the one place every request passes through, before anything leaves the machine. The
// scopes back it up where Google can: a `user` token never carries compose or settings.sharing,
// so it cannot touch forwarding, send-as or delegates.

use crate::error::{Result, VgoogError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// A person's own account: everything except sending mail. The default — including for
    /// accounts saved before tiers existed, because nothing should send as a person until they
    /// have said it may.
    #[default]
    User,
    /// The assistant's own mailbox: everything, sending included.
    Ai,
}

impl Tier {
    pub fn label(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Ai => "ai",
        }
    }
}

/// Scopes Google lets send mail. `gmail.modify` is here because it can.
const SEND_SCOPES: &[&str] = &[
    "https://mail.google.com/",
    "https://www.googleapis.com/auth/gmail.send",
    "https://www.googleapis.com/auth/gmail.compose",
    "https://www.googleapis.com/auth/gmail.modify",
];

/// Does this request put mail in someone's inbox? `messages.send` (plain or upload) and `drafts.send`.
pub fn is_send(url: &str) -> bool {
    let path = url.split('?').next().unwrap_or(url);
    path.contains("/gmail/v1/users/") && (path.ends_with("/messages/send") || path.ends_with("/drafts/send"))
}

/// Refuse a request the account is not trusted with, before it leaves the machine.
///
/// The refusal on a `user` account says what to do instead, not how to lift it. Whoever reads it
/// is usually the assistant, and the tier is the owner's decision to change, not the assistant's.
pub fn check(account: &str, tier: Tier, scopes: &[String], url: &str) -> Result<()> {
    if !is_send(url) {
        return Ok(());
    }
    if tier != Tier::Ai {
        return Err(VgoogError::Denied(format!(
            "account '{account}' is a user account, and vgoog never sends mail from it. \
             Save it as a draft instead (gmail create_draft) so its owner can send it."
        )));
    }
    // No recorded scopes means an OAuth login from before scopes were recorded. That login asked
    // for compose, so leave the last word to Google.
    if scopes.is_empty() || scopes.iter().any(|scope| SEND_SCOPES.contains(&scope.as_str())) {
        return Ok(());
    }
    Err(VgoogError::Denied(format!(
        "account '{account}' is an ai account, but none of its scopes can send mail. \
         Log it in again: vgoog login --account {account} --tier ai"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEND: &str = "https://gmail.googleapis.com/gmail/v1/users/me/messages/send";
    const DRAFT_SEND: &str = "https://gmail.googleapis.com/gmail/v1/users/me/drafts/send";
    const UPLOAD_SEND: &str = "https://gmail.googleapis.com/upload/gmail/v1/users/me/messages/send?uploadType=multipart";

    fn scopes(list: &[&str]) -> Vec<String> {
        list.iter().map(|scope| scope.to_string()).collect()
    }

    #[test]
    fn every_way_of_sending_is_recognised() {
        assert!(is_send(SEND));
        assert!(is_send(DRAFT_SEND));
        assert!(is_send(UPLOAD_SEND));
    }

    #[test]
    fn things_that_merely_mention_sending_are_not_sends() {
        assert!(!is_send("https://gmail.googleapis.com/gmail/v1/users/me/settings/sendAs"));
        assert!(!is_send("https://gmail.googleapis.com/gmail/v1/users/me/messages/abc/trash"));
        assert!(!is_send("https://gmail.googleapis.com/gmail/v1/users/me/drafts"));
        assert!(!is_send("https://www.googleapis.com/drive/v3/files/messages/send"));
    }

    #[test]
    fn only_ai_sends() {
        let modify = scopes(&["https://www.googleapis.com/auth/gmail.modify"]);
        assert!(check("eliot", Tier::Ai, &modify, SEND).is_ok());
        assert!(check("uri", Tier::User, &modify, SEND).is_err());
        assert!(check("uri", Tier::User, &modify, DRAFT_SEND).is_err());
        assert!(check("uri", Tier::User, &modify, UPLOAD_SEND).is_err());
    }

    #[test]
    fn the_refusal_points_at_a_draft() {
        let error = check("uri", Tier::User, &[], SEND).unwrap_err().to_string();
        assert!(error.contains("create_draft"), "{error}");
        assert!(!error.contains("--tier"), "the assistant is not told how to lift it: {error}");
    }

    #[test]
    fn a_user_account_still_does_everything_else() {
        let base = "https://gmail.googleapis.com/gmail/v1/users/me";
        for path in ["/messages/abc/modify", "/drafts", "/messages/abc/trash", "/labels"] {
            assert!(check("uri", Tier::User, &[], &format!("{base}{path}")).is_ok(), "{path}");
        }
    }

    #[test]
    fn ai_without_a_send_scope_is_refused_up_front() {
        let readonly = scopes(&["https://www.googleapis.com/auth/gmail.readonly"]);
        assert!(check("eliot", Tier::Ai, &readonly, SEND).is_err());
        assert!(check("eliot", Tier::Ai, &[], SEND).is_ok(), "an unrecorded legacy login is left to Google");
    }
}
