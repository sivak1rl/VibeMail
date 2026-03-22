# Troubleshooting

Common issues and their solutions.

---

## "Port 7887 already in use"

Another process is using the OAuth redirect port. Kill it:
```bash
lsof -i :7887 | grep -v PID | awk '{print $2}' | xargs kill -9
```

---

## IMAP connection timeout (Linux)

IPv6 may be broken in your environment. The app prefers IPv4, but if it still fails:
```bash
# Disable IPv6 for testing:
echo 1 | sudo tee /proc/sys/net/ipv6/conf/all/disable_ipv6
```

---

## "No tokens for \<email\>; re-auth required"

OAuth token has expired or been revoked. Click sync or restart the app to trigger a token refresh. If that doesn't work, remove the account and re-add it.

---

## Ollama not responding

Ensure Ollama is running:
```bash
# Check if service is up
curl http://localhost:11434/api/tags

# Or start it:
ollama serve
```

---

## Search not working

The Tantivy search index is built during sync. If search returns no results:
1. Trigger a full sync from the sidebar.
2. Restart the app if the index still seems empty.

---

## Gmail drafts not appearing

Gmail stores drafts in `[Gmail]/Drafts`. VibeMail detects this automatically via the `folder_role` system. If drafts aren't showing:
1. Ensure the Drafts folder appears in your sidebar after sync.
2. Click the Drafts folder — both local auto-saved drafts and IMAP-synced drafts will appear.

---

## Sync seems stuck or slow

- Check the sidebar for progress indicators.
- First sync downloads your entire mailbox history and may take a few minutes depending on volume.
- Subsequent syncs use the "sliding window" strategy and are much faster.
- IMAP IDLE provides real-time push for new mail between syncs.

---

## Database errors or "ON CONFLICT" issues

Go to **Settings → Danger Zone** and use "Reset Database Schema" to drop and recreate tables. This clears all local data but preserves account credentials.
