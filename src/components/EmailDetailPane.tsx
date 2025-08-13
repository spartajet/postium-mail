import React from "react";
import {
  makeStyles,
  tokens,
  shorthands,
  Text,
  Caption1,
  Body1,
  Button,
  Avatar,
  Divider,
  Toolbar,
  ToolbarButton,
  ToolbarDivider,
  Badge,
  Menu,
  MenuTrigger,
  MenuPopover,
  MenuList,
  MenuItem,
} from "@fluentui/react-components";
import {
  ArrowReply20Regular,
  ArrowReplyAll20Regular,
  ArrowForward20Regular,
  Delete20Regular,
  Archive20Regular,
  Print20Regular,
  MoreHorizontal20Regular,
  Star20Regular,
  Star20Filled,
  Flag20Regular,
  Flag20Filled,
  AttachRegular,
  ArrowDownload20Filled,
  bundleIcon,
} from "@fluentui/react-icons";
import { format } from "date-fns";
import { useEmailStore } from "../stores/useEmailStore";

const StarIcon = bundleIcon(Star20Filled, Star20Regular);
const FlagIcon = bundleIcon(Flag20Filled, Flag20Regular);

const useStyles = makeStyles({
  root: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
    backgroundColor: tokens.colorNeutralBackground1,
    overflow: "hidden",
  },
  toolbar: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    ...shorthands.padding(tokens.spacingVerticalS, tokens.spacingHorizontalM),
    borderBottom: `1px solid ${tokens.colorNeutralStroke1}`,
    minHeight: "48px",
  },
  content: {
    flex: 1,
    overflow: "auto",
    ...shorthands.padding(tokens.spacingVerticalL, tokens.spacingHorizontalL),
  },
  header: {
    marginBottom: tokens.spacingVerticalL,
  },
  subject: {
    marginBottom: tokens.spacingVerticalM,
  },
  sender: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    marginBottom: tokens.spacingVerticalM,
  },
  senderInfo: {
    display: "flex",
    alignItems: "center",
    ...shorthands.gap(tokens.spacingHorizontalM),
  },
  senderDetails: {
    display: "flex",
    flexDirection: "column",
  },
  recipients: {
    marginBottom: tokens.spacingVerticalM,
  },
  recipientList: {
    display: "flex",
    flexWrap: "wrap",
    ...shorthands.gap(tokens.spacingHorizontalXS),
    marginTop: tokens.spacingVerticalXS,
  },
  labels: {
    display: "flex",
    ...shorthands.gap(tokens.spacingHorizontalXS),
    marginBottom: tokens.spacingVerticalM,
  },
  body: {
    marginTop: tokens.spacingVerticalL,
    lineHeight: "1.6",
  },
  attachments: {
    marginTop: tokens.spacingVerticalL,
    ...shorthands.padding(tokens.spacingVerticalM),
    backgroundColor: tokens.colorNeutralBackground2,
    ...shorthands.borderRadius(tokens.borderRadiusMedium),
  },
  attachmentList: {
    display: "flex",
    flexDirection: "column",
    ...shorthands.gap(tokens.spacingVerticalS),
    marginTop: tokens.spacingVerticalS,
  },
  attachment: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    ...shorthands.padding(tokens.spacingVerticalS, tokens.spacingHorizontalS),
    backgroundColor: tokens.colorNeutralBackground1,
    ...shorthands.borderRadius(tokens.borderRadiusMedium),
    cursor: "pointer",
    "&:hover": {
      backgroundColor: tokens.colorNeutralBackground1Hover,
    },
  },
  attachmentInfo: {
    display: "flex",
    alignItems: "center",
    ...shorthands.gap(tokens.spacingHorizontalS),
  },
  emptyState: {
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    height: "100%",
    color: tokens.colorNeutralForeground3,
  },
});

export const EmailDetailPane: React.FC = () => {
  const styles = useStyles();
  const {
    currentEmail,
    toggleStar,
    toggleFlag,
    deleteEmails,
    archiveEmails,
    createDraft,
  } = useEmailStore();

  if (!currentEmail) {
    return (
      <div className={styles.root}>
        <div className={styles.emptyState}>
          <Text size={400}>Select an email to view</Text>
        </div>
      </div>
    );
  }

  const handleReply = () => {
    createDraft({
      to: [currentEmail.from.email],
      subject: `Re: ${currentEmail.subject}`,
      inReplyTo: currentEmail.messageId,
      references: [currentEmail.messageId],
    });
  };

  const handleReplyAll = () => {
    const recipients = [
      currentEmail.from.email,
      ...currentEmail.to.map((c) => c.email),
      ...(currentEmail.cc?.map((c) => c.email) || []),
    ].filter((email, index, self) => self.indexOf(email) === index);

    createDraft({
      to: recipients,
      subject: `Re: ${currentEmail.subject}`,
      inReplyTo: currentEmail.messageId,
      references: [currentEmail.messageId],
    });
  };

  const handleForward = () => {
    createDraft({
      subject: `Fwd: ${currentEmail.subject}`,
      body: `\n\n---------- Forwarded message ----------\nFrom: ${currentEmail.from.name || currentEmail.from.email}\nDate: ${format(new Date(currentEmail.date), "PPP p")}\nSubject: ${currentEmail.subject}\n\n${currentEmail.body}`,
    });
  };

  const formatFileSize = (bytes: number) => {
    if (bytes < 1024) return bytes + " B";
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + " KB";
    return (bytes / (1024 * 1024)).toFixed(1) + " MB";
  };

  return (
    <div className={styles.root}>
      <Toolbar className={styles.toolbar} size="small">
        <div style={{ display: "flex", gap: tokens.spacingHorizontalXS }}>
          <ToolbarButton icon={<ArrowReply20Regular />} onClick={handleReply}>
            Reply
          </ToolbarButton>
          <ToolbarButton
            icon={<ArrowReplyAll20Regular />}
            onClick={handleReplyAll}
          >
            Reply All
          </ToolbarButton>
          <ToolbarButton
            icon={<ArrowForward20Regular />}
            onClick={handleForward}
          >
            Forward
          </ToolbarButton>
          <ToolbarDivider />
          <ToolbarButton
            icon={<Archive20Regular />}
            onClick={() => archiveEmails([currentEmail.id])}
          />
          <ToolbarButton
            icon={<Delete20Regular />}
            onClick={() => deleteEmails([currentEmail.id])}
          />
          <ToolbarDivider />
          <ToolbarButton
            icon={currentEmail.isStarred ? <StarIcon /> : <Star20Regular />}
            onClick={() => toggleStar(currentEmail.id)}
          />
          <ToolbarButton
            icon={currentEmail.isFlagged ? <FlagIcon /> : <Flag20Regular />}
            onClick={() => toggleFlag(currentEmail.id)}
          />
        </div>
        <div style={{ display: "flex", gap: tokens.spacingHorizontalXS }}>
          <ToolbarButton icon={<Print20Regular />} />
          <Menu>
            <MenuTrigger disableButtonEnhancement>
              <ToolbarButton icon={<MoreHorizontal20Regular />} />
            </MenuTrigger>
            <MenuPopover>
              <MenuList>
                <MenuItem>Mark as unread</MenuItem>
                <MenuItem>Move to folder</MenuItem>
                <MenuItem>Add label</MenuItem>
                <MenuItem>Mark as spam</MenuItem>
                <MenuItem>Block sender</MenuItem>
              </MenuList>
            </MenuPopover>
          </Menu>
        </div>
      </Toolbar>

      <div className={styles.content}>
        <div className={styles.header}>
          <Text size={600} weight="semibold" className={styles.subject}>
            {currentEmail.subject}
          </Text>

          {currentEmail.labels && currentEmail.labels.length > 0 && (
            <div className={styles.labels}>
              {currentEmail.labels.map((label) => (
                <Badge
                  key={label.id}
                  size="small"
                  appearance="tint"
                  style={{
                    backgroundColor: label.color + "20",
                    color: label.color,
                  }}
                >
                  {label.name}
                </Badge>
              ))}
            </div>
          )}

          <div className={styles.sender}>
            <div className={styles.senderInfo}>
              <Avatar
                name={currentEmail.from.name || currentEmail.from.email}
                size={48}
                color="colorful"
              />
              <div className={styles.senderDetails}>
                <Text weight="semibold">
                  {currentEmail.from.name || currentEmail.from.email}
                </Text>
                <Caption1>&lt;{currentEmail.from.email}&gt;</Caption1>
                <Caption1>
                  {format(new Date(currentEmail.date), "PPP p")}
                </Caption1>
              </div>
            </div>
            {currentEmail.isImportant && (
              <Badge color="important">Important</Badge>
            )}
          </div>

          <div className={styles.recipients}>
            <Caption1>
              <strong>To:</strong>{" "}
              {currentEmail.to.map((c) => c.name || c.email).join(", ")}
            </Caption1>
            {currentEmail.cc && currentEmail.cc.length > 0 && (
              <Caption1>
                <strong>Cc:</strong>{" "}
                {currentEmail.cc.map((c) => c.name || c.email).join(", ")}
              </Caption1>
            )}
          </div>
        </div>

        <Divider />

        <div className={styles.body}>
          {currentEmail.bodyHtml ? (
            <div dangerouslySetInnerHTML={{ __html: currentEmail.bodyHtml }} />
          ) : (
            <Body1>{currentEmail.body}</Body1>
          )}
        </div>

        {currentEmail.attachments && currentEmail.attachments.length > 0 && (
          <div className={styles.attachments}>
            <Text weight="semibold">
              Attachments ({currentEmail.attachments.length})
            </Text>
            <div className={styles.attachmentList}>
              {currentEmail.attachments.map((attachment) => (
                <div key={attachment.id} className={styles.attachment}>
                  <div className={styles.attachmentInfo}>
                    <AttachRegular />
                    <div>
                      <Text size={300}>{attachment.filename}</Text>
                      <Caption1>{formatFileSize(attachment.size)}</Caption1>
                    </div>
                  </div>
                  <Button
                    appearance="subtle"
                    icon={<ArrowDownload20Filled />}
                    size="small"
                  />
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
