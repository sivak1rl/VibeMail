import { Mailbox } from "../stores/mailboxes";

export interface MailboxTreeNode {
  id: string | null;
  name: string;
  fullName: string;
  children: MailboxTreeNode[];
  mailbox: Mailbox | null;
}

export function buildMailboxTree(mailboxes: Mailbox[]): MailboxTreeNode[] {
  const root: MailboxTreeNode[] = [];
  const map: Record<string, MailboxTreeNode> = {};

  // Sort mailboxes by name length to ensure parents are processed before or at least correctly
  const sorted = [...mailboxes].sort((a, b) => a.name.localeCompare(b.name));

  for (const mailbox of sorted) {
    const delimiter = mailbox.delimiter || "/";
    const parts = mailbox.name.split(delimiter);
    let currentLevel = root;
    let currentPath = "";

    for (let i = 0; i < parts.length; i++) {
      const part = parts[i];
      currentPath = currentPath ? `${currentPath}${delimiter}${part}` : part;
      const isLast = i === parts.length - 1;

      let node = currentLevel.find((n) => n.name === part);

      if (!node) {
        node = {
          id: isLast ? mailbox.id : null,
          name: part,
          fullName: currentPath,
          children: [],
          mailbox: isLast ? mailbox : null,
        };
        currentLevel.push(node);
      } else if (isLast) {
        // If the node already exists (as a virtual parent), but now we found the actual mailbox
        node.id = mailbox.id;
        node.mailbox = mailbox;
      }

      currentLevel = node.children;
    }
  }

  return root;
}
