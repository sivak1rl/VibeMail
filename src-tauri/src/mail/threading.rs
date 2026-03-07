/// JWZ threading algorithm (RFC 5256)
/// Groups messages into threads based on Message-ID, References, and In-Reply-To headers.
use crate::db::models::{Message, Thread};
use std::collections::{BTreeSet, HashMap};
use uuid::Uuid;

struct ThreadNode {
    message_id: Option<String>,
    message: Option<Message>,
    parent: Option<String>,
    children: Vec<String>,
}

pub fn build_threads(messages: Vec<Message>, account_id: &str) -> Vec<Thread> {
    // Step 1: Build id_table mapping message-id -> node
    let mut id_table: HashMap<String, ThreadNode> = HashMap::new();
    let mut root_ids: Vec<String> = Vec::new();

    for msg in messages {
        let mid = msg
            .message_id
            .clone()
            .unwrap_or_else(|| format!("synthetic-{}", msg.id));

        let node = id_table.entry(mid.clone()).or_insert_with(|| ThreadNode {
            message_id: Some(mid.clone()),
            message: None,
            parent: None,
            children: Vec::new(),
        });
        node.message = Some(msg.clone());

        // Process references chain
        let mut refs: Vec<String> = msg.references_ids.clone();
        if let Some(irt) = &msg.in_reply_to {
            if !refs.contains(irt) {
                refs.push(irt.clone());
            }
        }

        // Link references chain parent→child
        for i in 0..refs.len() {
            let ref_id = &refs[i];
            id_table
                .entry(ref_id.clone())
                .or_insert_with(|| ThreadNode {
                    message_id: Some(ref_id.clone()),
                    message: None,
                    parent: None,
                    children: Vec::new(),
                });

            if i + 1 < refs.len() {
                let child_id = &refs[i + 1];
                id_table
                    .entry(child_id.clone())
                    .or_insert_with(|| ThreadNode {
                        message_id: Some(child_id.clone()),
                        message: None,
                        parent: None,
                        children: Vec::new(),
                    });
                // Only link if child has no parent (avoid cycles)
                let child_has_parent = id_table[child_id].parent.is_some();
                if !child_has_parent {
                    let child = id_table.get_mut(child_id).unwrap();
                    child.parent = Some(ref_id.clone());
                    let parent = id_table.get_mut(ref_id).unwrap();
                    if !parent.children.contains(child_id) {
                        parent.children.push(child_id.clone());
                    }
                }
            }
        }

        // Link the message itself to the last reference
        if let Some(parent_ref) = refs.last() {
            let msg_has_parent = id_table[&mid].parent.is_some();
            if !msg_has_parent {
                let node = id_table.get_mut(&mid).unwrap();
                node.parent = Some(parent_ref.clone());
                let parent = id_table.get_mut(parent_ref).unwrap();
                if !parent.children.contains(&mid) {
                    parent.children.push(mid.clone());
                }
            }
        }
    }

    // Step 2: Collect roots (nodes with no parent)
    for (mid, node) in &id_table {
        if node.parent.is_none() {
            root_ids.push(mid.clone());
        }
    }

    // Step 3: Build Thread objects from roots
    let mut threads = Vec::new();

    for root_id in root_ids {
        let thread_id = Uuid::new_v4().to_string();
        let mut thread_messages = Vec::new();
        collect_messages(&id_table, &root_id, &thread_id, &mut thread_messages);

        if thread_messages.is_empty() {
            continue;
        }

        let subject = thread_messages
            .iter()
            .find_map(|m: &Message| m.subject.clone());
        let last_date = thread_messages.iter().filter_map(|m| m.date).max();
        let last_from = thread_messages
            .iter()
            .max_by_key(|m| m.date)
            .and_then(|m| m.from.first())
            .map(|a| a.email.clone());
        let unread_count = thread_messages
            .iter()
            .filter(|m| !m.flags.iter().any(|f| f == "\\Seen"))
            .count() as u32;
        let is_flagged = thread_messages
            .iter()
            .any(|m| m.flags.iter().any(|f| f == "\\Flagged"));

        let mut participants: Vec<_> = thread_messages
            .iter()
            .flat_map(|m| m.from.iter().chain(m.to.iter()))
            .cloned()
            .collect();
        participants.dedup_by(|a, b| a.email == b.email);

        let count = thread_messages.len() as u32;
        let labels = thread_messages
            .iter()
            .flat_map(|m| m.flags.iter())
            .filter_map(|flag| flag.strip_prefix("VibeMail/").map(|s| s.to_string()))
            .filter(|label| !label.is_empty())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();

        threads.push(Thread {
            id: thread_id.clone(),
            account_id: account_id.to_string(),
            subject,
            participants,
            message_count: count,
            unread_count,
            is_flagged,
            last_date,
            last_from,
            triage_score: thread_messages
                .iter()
                .filter_map(|m| m.triage_score)
                .reduce(f64::max),
            labels,
            messages: Some(thread_messages),
        });
    }

    // Sort by most recent
    threads.sort_by(|a, b| b.last_date.cmp(&a.last_date));
    threads
}

fn collect_messages(
    table: &HashMap<String, ThreadNode>,
    id: &str,
    thread_id: &str,
    out: &mut Vec<Message>,
) {
    if let Some(node) = table.get(id) {
        if let Some(mut msg) = node.message.clone() {
            msg.thread_id = Some(thread_id.to_string());
            out.push(msg);
        }
        for child in &node.children {
            collect_messages(table, child, thread_id, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::EmailAddress;
    use chrono::Utc;

    fn make_msg(id: &str, subject: &str, refs: Vec<&str>, irt: Option<&str>) -> Message {
        Message {
            id: id.to_string(),
            account_id: "acc1".to_string(),
            mailbox_id: "mb1".to_string(),
            uid: 1,
            message_id: Some(format!("<{}>", id)),
            thread_id: None,
            subject: Some(subject.to_string()),
            from: vec![EmailAddress {
                name: None,
                email: "test@example.com".to_string(),
            }],
            to: vec![],
            cc: vec![],
            date: Some(Utc::now()),
            body_text: None,
            body_html: None,
            references_ids: refs.into_iter().map(|r| format!("<{}>", r)).collect(),
            in_reply_to: irt.map(|r| format!("<{}>", r)),
            flags: vec![],
            has_attachments: false,
            triage_score: None,
            ai_summary: None,
        }
    }

    #[test]
    fn test_simple_thread() {
        let msgs = vec![
            make_msg("msg1", "Hello", vec![], None),
            make_msg("msg2", "Re: Hello", vec!["msg1"], Some("msg1")),
            make_msg("msg3", "Re: Hello", vec!["msg1", "msg2"], Some("msg2")),
        ];
        let threads = build_threads(msgs, "acc1");
        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].message_count, 3);
    }
}
