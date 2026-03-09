/// Structured output parsing for AI tool calls
use crate::db::models::ExtractedAction;
use anyhow::Result;
use serde_json::Value;
use std::collections::HashSet;

/// Parse action extraction JSON from AI response
pub fn parse_extracted_actions(raw: &str) -> Result<Vec<ExtractedAction>> {
    // Try to find JSON array in the response (model may wrap it in text)
    let json_start = raw.find('[').unwrap_or(0);
    let json_end = raw.rfind(']').map(|i| i + 1).unwrap_or(raw.len());
    let json_str = &raw[json_start..json_end];

    let value: Value = serde_json::from_str(json_str).unwrap_or(Value::Array(vec![]));
    let arr = value.as_array().cloned().unwrap_or_default();

    let actions = arr
        .iter()
        .filter_map(|item| {
            Some(ExtractedAction {
                kind: item["type"].as_str().unwrap_or("todo").to_string(),
                text: item["text"].as_str()?.to_string(),
                date: item["date"].as_str().map(|s| s.to_string()),
                priority: item["priority"].as_str().map(|s| s.to_string()),
            })
        })
        .collect();

    Ok(actions)
}

/// Parse triage score from AI response (expects a number 0-1 or 0-10)
pub fn parse_triage_score(raw: &str) -> f64 {
    // Look for a number in the response
    for word in raw.split_whitespace() {
        let clean = word.trim_matches(|c: char| !c.is_ascii_digit() && c != '.');
        if let Ok(score) = clean.parse::<f64>() {
            // Normalize to 0-1
            return if score > 1.0 { score / 10.0 } else { score };
        }
    }
    0.5 // default medium priority
}

/// Parse a single category label from AI response.
pub fn parse_category_label(raw: &str, allowed_categories: &[String]) -> String {
    if allowed_categories.is_empty() {
        return "updates".to_string();
    }

    let allowed = allowed_categories
        .iter()
        .map(|item| item.to_ascii_lowercase())
        .collect::<HashSet<_>>();
    let fallback = allowed_categories[0].clone();

    let normalized = raw
        .trim()
        .trim_matches(|c| c == '"' || c == '\'' || c == '`')
        .to_ascii_lowercase();

    let compact = normalized.split_whitespace().collect::<Vec<_>>().join(" ");

    if (compact.contains("receipt")
        || compact.contains("invoice")
        || compact.contains("billing")
        || compact.contains("payment"))
        && allowed.contains("receipt")
    {
        return "receipt".to_string();
    }
    if (compact.contains("newsletter") || compact.contains("digest"))
        && allowed.contains("newsletter")
    {
        return "newsletter".to_string();
    }
    if (compact.contains("social")
        || compact.contains("friend")
        || compact.contains("community")
        || compact.contains("network"))
        && allowed.contains("social")
    {
        return "social".to_string();
    }
    if (compact.contains("update")
        || compact.contains("notification")
        || compact.contains("automated")
        || compact.contains("system"))
        && allowed.contains("updates")
    {
        return "updates".to_string();
    }

    for category in allowed_categories {
        let lowered = category.to_ascii_lowercase();
        if compact == lowered || compact.contains(&lowered) {
            return category.clone();
        }
    }

    fallback
}

pub const SYSTEM_SUMMARIZE: &str = r#"You are an email assistant. Summarize the email thread in 2-4 sentences. Cover: the main topic, any decisions made, open questions, and the most important action item. Be factual and concise — no filler phrases like "This email thread discusses..."."#;

pub const SYSTEM_DRAFT: &str = r#"You are an expert email writer. Draft a reply to the email thread below.

Rules:
- Match the tone exactly: if the thread is casual, be casual; if formal, be formal.
- Be direct and concise — no padding, no throat-clearing openers like "I hope this email finds you well."
- You may use markdown formatting (bullet lists, bold) where it aids clarity.
- Do NOT include a subject line, greeting label ("Hi John,"), sign-off, or signature — those are added separately.
- Output only the reply body text, starting directly with the first sentence of your response."#;

pub const SYSTEM_EXTRACT: &str = r#"You are an email assistant. Extract all action items, deadlines, and follow-up tasks from the email thread. Return a JSON array with objects having these fields: {"type": "todo"|"date"|"followup", "text": "...", "date": "YYYY-MM-DD or null", "priority": "high"|"medium"|"low"}. Output only the JSON array, nothing else."#;

pub const SYSTEM_TRIAGE: &str = r#"You are an email triage assistant. Score the importance of this email on a scale from 0 to 1, where 1 is most important (requires immediate action, from a VIP, time-sensitive) and 0 is least important (newsletters, automated notifications, spam). Consider: sender importance, urgency signals, action requirements. Reply with only a single decimal number between 0 and 1."#;

pub fn build_categorize_system_prompt(category_schema_json: &str) -> String {
    format!(
        "You are an email categorization assistant.
Choose exactly one category label for the email thread.
Allowed categories and examples are provided as JSON below.
Only use one category from that JSON.
Reply with only the category label text and nothing else.

CATEGORY_SCHEMA_JSON:
{}",
        category_schema_json
    )
}
