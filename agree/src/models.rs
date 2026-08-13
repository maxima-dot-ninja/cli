use crate::money::Money;
use serde::{Deserialize, Serialize};

/// Fields are almost all optional on read: the API omits what does not apply to a
/// given record, and a missing field must never be a parse failure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub company: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
}

impl Contact {
    pub fn label(&self) -> String {
        let name = self.name.clone().unwrap_or_else(|| "(no name)".into());
        let email = self.email.clone().unwrap_or_else(|| "(no email)".into());
        match &self.company {
            Some(company) if !company.is_empty() => format!("{name} <{email}> · {company}"),
            _ => format!("{name} <{email}>"),
        }
    }

    /// Loose match for resolving a spoken name like "Samir" — the API can only
    /// filter contacts by email and company, so first-name lookup happens here.
    pub fn matches(&self, needle: &str) -> bool {
        let needle = needle.to_lowercase();
        let haystack = [&self.name, &self.email, &self.company]
            .iter()
            .filter_map(|f| f.as_ref())
            .map(|s| s.to_lowercase())
            .collect::<Vec<_>>()
            .join(" ");
        haystack.contains(&needle)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Customer {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub business_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invoice {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub amount: Option<Money>,
    #[serde(default)]
    pub due_at: Option<String>,
    #[serde(default)]
    pub scheduled_at: Option<String>,
    #[serde(default)]
    pub paid_at: Option<String>,
    #[serde(default)]
    pub memo: Option<String>,
    #[serde(default)]
    pub invoice_url: Option<String>,
    #[serde(default)]
    pub payment_link: Option<String>,
    #[serde(default)]
    pub agreement_id: Option<String>,
    #[serde(default)]
    pub recurring_sequence: Option<i64>,
    #[serde(default)]
    pub billing_contact: Option<BillingContact>,
    #[serde(default)]
    pub recurring_options: Option<RecurringOptions>,
}

impl Invoice {
    pub fn payer(&self) -> String {
        self.billing_contact
            .as_ref()
            .map(|c| c.name.clone().unwrap_or_else(|| c.email.clone()))
            .unwrap_or_else(|| "—".into())
    }

    pub fn amount_display(&self) -> String {
        self.amount.as_ref().map(|m| m.display()).unwrap_or_else(|| "—".into())
    }

    /// "every week", "every 2 months" — empty when it does not repeat
    pub fn cadence(&self) -> String {
        let Some(options) = &self.recurring_options else {
            return String::new();
        };
        if options.schedule.as_deref() != Some("custom") {
            return String::new();
        }
        let unit = options.repeat_unit.clone().unwrap_or_else(|| "period".into());
        match options.repeat_frequency.unwrap_or(1) {
            1 => format!("every {unit}"),
            n => format!("every {n} {unit}s"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingContact {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub email: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub company: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecurringOptions {
    /// "none" or "custom" — anything repeating needs "custom"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
    /// "week" or "month"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat_unit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat_frequency: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat_on_weekday: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat_on_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat_on_day: Option<i64>,
    /// "never" | "date" | "count"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recurring_end_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recurring_end_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recurring_end_count: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reminder_schedule: Option<String>,
}

impl RecurringOptions {
    /// The shape the API wants for "every N weeks/months, forever".
    ///
    /// Incomplete on its own: a weekly schedule also needs a weekday and a monthly
    /// one needs a day of the month. Call `anchor_to` before sending.
    pub fn every(unit: &str, frequency: i64) -> Self {
        Self {
            schedule: Some("custom".into()),
            repeat_unit: Some(unit.into()),
            repeat_frequency: Some(frequency),
            recurring_end_type: Some("never".into()),
            ..Default::default()
        }
    }

    /// Pin the repeat to a specific date's weekday or day-of-month.
    ///
    /// The API rejects a weekly schedule with a blank `repeat_on_weekday`, and a
    /// monthly one needs `repeat_on_type` plus `repeat_on_day`. Taking both from
    /// the due date means "every week" repeats on the day it is first due, which
    /// is what people mean by it.
    pub fn anchor_to(&mut self, date: chrono::NaiveDate) {
        use chrono::Datelike;

        match self.repeat_unit.as_deref() {
            Some("week") => {
                self.repeat_on_weekday = Some(weekday_name(date));
            }
            Some("month") => {
                self.repeat_on_type = Some("day_of_month".into());
                self.repeat_on_day = Some(date.day() as i64);
            }
            _ => {}
        }
    }

    /// "every week on Wednesday", "every 2 months on the 8th"
    pub fn describe(&self) -> String {
        let Some(unit) = self.repeat_unit.as_deref() else {
            return "one-off".into();
        };
        let every = match self.repeat_frequency.unwrap_or(1) {
            1 => format!("every {unit}"),
            n => format!("every {n} {unit}s"),
        };
        match (unit, &self.repeat_on_weekday, self.repeat_on_day) {
            ("week", Some(day), _) => format!("{every} on {}", capitalise(day)),
            ("month", _, Some(day)) => format!("{every} on day {day}"),
            _ => every,
        }
    }
}

fn weekday_name(date: chrono::NaiveDate) -> String {
    use chrono::Datelike;
    match date.weekday() {
        chrono::Weekday::Mon => "monday",
        chrono::Weekday::Tue => "tuesday",
        chrono::Weekday::Wed => "wednesday",
        chrono::Weekday::Thu => "thursday",
        chrono::Weekday::Fri => "friday",
        chrono::Weekday::Sat => "saturday",
        chrono::Weekday::Sun => "sunday",
    }
    .to_string()
}

fn capitalise(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agreement {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub starts_at: Option<String>,
    #[serde(default)]
    pub ends_at: Option<String>,
    #[serde(default)]
    pub delivery_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
}

/// Read back from the webhook tools, which return raw JSON today.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEndpoint {
    pub id: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub events: Option<Vec<String>>,
    #[serde(default)]
    pub active: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_sparse_invoice() {
        // The API omits fields that do not apply; that must not fail parsing.
        let raw = r#"{"id":"abc","status":"due"}"#;
        let invoice: Invoice = serde_json::from_str(raw).unwrap();
        assert_eq!(invoice.id, "abc");
        assert_eq!(invoice.amount_display(), "—");
        assert_eq!(invoice.payer(), "—");
        assert_eq!(invoice.cadence(), "");
    }

    #[test]
    fn describes_its_cadence() {
        let weekly = Invoice {
            id: "1".into(), name: None, status: None, amount: None, due_at: None,
            scheduled_at: None, paid_at: None, memo: None, invoice_url: None,
            payment_link: None, agreement_id: None, recurring_sequence: None,
            billing_contact: None,
            recurring_options: Some(RecurringOptions::every("week", 1)),
        };
        assert_eq!(weekly.cadence(), "every week");

        let biweekly = Invoice {
            recurring_options: Some(RecurringOptions::every("week", 2)),
            ..weekly
        };
        assert_eq!(biweekly.cadence(), "every 2 weeks");
    }

    #[test]
    fn recurring_shape_matches_the_api() {
        let options = RecurringOptions::every("week", 1);
        let json = serde_json::to_value(&options).unwrap();
        assert_eq!(json["schedule"], "custom");
        assert_eq!(json["repeat_unit"], "week");
        assert_eq!(json["repeat_frequency"], 1);
        assert_eq!(json["recurring_end_type"], "never");
        // Unset fields must be omitted, not sent as null
        assert!(json.get("repeat_on_day").is_none());
    }

    #[test]
    fn resolves_a_first_name() {
        let samir = Contact {
            id: "1".into(),
            name: Some("Samir Patel".into()),
            email: Some("samir@treehaus.io".into()),
            company: Some("Treehaus".into()),
            title: None,
        };
        assert!(samir.matches("samir"));
        assert!(samir.matches("Samir"));
        assert!(samir.matches("treehaus"));
        assert!(!samir.matches("jordan"));
    }
}

#[cfg(test)]
mod recurring_tests {
    use super::*;
    use chrono::NaiveDate;

    fn date(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn weekly_gets_a_weekday_or_the_api_rejects_it() {
        // The exact failure seen live: "repeat_on_weekday: can't be blank"
        let mut options = RecurringOptions::every("week", 1);
        assert!(options.repeat_on_weekday.is_none(), "starts blank");

        options.anchor_to(date("2026-07-08")); // a Wednesday
        assert_eq!(options.repeat_on_weekday.as_deref(), Some("wednesday"));

        let json = serde_json::to_value(&options).unwrap();
        assert_eq!(json["repeat_on_weekday"], "wednesday");
    }

    #[test]
    fn monthly_gets_a_day_of_month() {
        let mut options = RecurringOptions::every("month", 1);
        options.anchor_to(date("2026-07-08"));
        assert_eq!(options.repeat_on_type.as_deref(), Some("day_of_month"));
        assert_eq!(options.repeat_on_day, Some(8));
        // A monthly repeat must not carry a weekday
        assert!(options.repeat_on_weekday.is_none());
    }

    #[test]
    fn every_weekday_maps_correctly() {
        let expected = [
            ("2026-07-06", "monday"),
            ("2026-07-07", "tuesday"),
            ("2026-07-08", "wednesday"),
            ("2026-07-09", "thursday"),
            ("2026-07-10", "friday"),
            ("2026-07-11", "saturday"),
            ("2026-07-12", "sunday"),
        ];
        for (day, name) in expected {
            let mut options = RecurringOptions::every("week", 1);
            options.anchor_to(date(day));
            assert_eq!(options.repeat_on_weekday.as_deref(), Some(name), "{day}");
        }
    }

    #[test]
    fn describes_itself_for_the_review_block() {
        let mut weekly = RecurringOptions::every("week", 1);
        weekly.anchor_to(date("2026-07-08"));
        assert_eq!(weekly.describe(), "every week on Wednesday");

        let mut fortnightly = RecurringOptions::every("week", 2);
        fortnightly.anchor_to(date("2026-07-10"));
        assert_eq!(fortnightly.describe(), "every 2 weeks on Friday");

        let mut monthly = RecurringOptions::every("month", 1);
        monthly.anchor_to(date("2026-07-08"));
        assert_eq!(monthly.describe(), "every month on day 8");
    }
}
