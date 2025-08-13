import React, { useMemo, useCallback } from "react";
import { useTranslation } from "react-i18next";
import {
  makeStyles,
  tokens,
  shorthands,
  Text,
  Caption1,
  Checkbox,
  Button,
  Badge,
  Avatar,
  Toolbar,
  ToolbarButton,
  ToolbarDivider,
  Spinner,
  Tooltip,
} from "@fluentui/react-components";
import {
  Mail20Regular,
  Mail20Filled,
  MailRead20Regular,
  MailRead20Filled,
  Star20Regular,
  Star20Filled,
  Flag20Regular,
  Flag20Filled,
  Delete20Regular,
  Archive20Regular,
  AttachRegular,
  bundleIcon,
} from "@fluentui/react-icons";
import { FixedSizeList as List } from "react-window";
import { format, isToday, isYesterday } from "date-fns";
import { useEmailStore } from "../stores/useEmailStore";

const MailIcon = bundleIcon(Mail20Filled, Mail20Regular);
const MailReadIcon = bundleIcon(MailRead20Filled, MailRead20Regular);
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
  toolbarActions: {
    display: "flex",
    alignItems: "center",
    ...shorthands.gap(tokens.spacingHorizontalXS),
  },
  listContainer: {
    flex: 1,
    overflow: "hidden",
  },
  emailItem: {
    display: "flex",
    alignItems: "flex-start",
    ...shorthands.padding(tokens.spacingVerticalM, tokens.spacingHorizontalM),
    cursor: "pointer",
    borderBottom: `1px solid ${tokens.colorNeutralStroke1}`,
    transition: "background-color 0.1s ease",
    "&:hover": {
      backgroundColor: tokens.colorNeutralBackground1Hover,
    },
  },
  emailItemSelected: {
    backgroundColor: tokens.colorBrandBackground2,
    "&:hover": {
      backgroundColor: tokens.colorBrandBackground2Hover,
    },
  },
  emailItemUnread: {
    backgroundColor: tokens.colorNeutralBackground2,
    "&:hover": {
      backgroundColor: tokens.colorNeutralBackground2Hover,
    },
  },
  checkbox: {
    marginTop: "2px",
    marginRight: tokens.spacingHorizontalS,
  },
  avatar: {
    marginRight: tokens.spacingHorizontalM,
    flexShrink: 0,
  },
  emailContent: {
    flex: 1,
    minWidth: 0,
    display: "flex",
    flexDirection: "column",
    ...shorthands.gap(tokens.spacingVerticalXS),
  },
  emailHeader: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    ...shorthands.gap(tokens.spacingHorizontalS),
  },
  emailSender: {
    display: "flex",
    alignItems: "center",
    ...shorthands.gap(tokens.spacingHorizontalXS),
    flex: 1,
    minWidth: 0,
  },
  senderName: {
    ...shorthands.overflow("hidden"),
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  emailMeta: {
    display: "flex",
    alignItems: "center",
    ...shorthands.gap(tokens.spacingHorizontalXS),
    flexShrink: 0,
  },
  emailSubject: {
    ...shorthands.overflow("hidden"),
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    marginBottom: tokens.spacingVerticalXS,
  },
  emailPreview: {
    ...shorthands.overflow("hidden"),
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    color: tokens.colorNeutralForeground3,
  },
  emailLabels: {
    display: "flex",
    alignItems: "center",
    ...shorthands.gap(tokens.spacingHorizontalXS),
    marginTop: tokens.spacingVerticalXS,
  },
  emptyState: {
    display: "flex",
    flexDirection: "column",
    alignItems: "center",
    justifyContent: "center",
    height: "100%",
    ...shorthands.padding(tokens.spacingVerticalXXL),
  },
  loadingState: {
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    height: "100%",
  },
});

export const EmailListPane: React.FC = () => {
  const styles = useStyles();
  const { t } = useTranslation();
  const {
    emails,
    currentAccountId,
    currentEmail,
    selectedEmailIds,
    isLoading,
    selectEmail,
    toggleEmailSelection,
    selectAllEmails,
    clearSelection,
    markAsRead,
    markAsUnread,
    toggleStar,
    toggleFlag,
    deleteEmails,
    archiveEmails,
  } = useEmailStore();

  const currentAccountEmails = useMemo(() => {
    if (!currentAccountId) return [];
    return emails.get(currentAccountId) || [];
  }, [currentAccountId, emails]);

  const isAllSelected = useMemo(() => {
    return (
      currentAccountEmails.length > 0 &&
      selectedEmailIds.size === currentAccountEmails.length
    );
  }, [currentAccountEmails, selectedEmailIds]);

  const isSomeSelected = useMemo(() => {
    return (
      selectedEmailIds.size > 0 &&
      selectedEmailIds.size < currentAccountEmails.length
    );
  }, [currentAccountEmails, selectedEmailIds]);

  const formatEmailDate = (date: Date | string) => {
    const emailDate = new Date(date);
    if (isToday(emailDate)) {
      return format(emailDate, "h:mm a");
    } else if (isYesterday(emailDate)) {
      return "Yesterday";
    } else if (emailDate.getFullYear() === new Date().getFullYear()) {
      return format(emailDate, "MMM d");
    } else {
      return format(emailDate, "MMM d, yyyy");
    }
  };

  const handleSelectAll = () => {
    if (isAllSelected) {
      clearSelection();
    } else {
      selectAllEmails();
    }
  };

  const handleMarkAsRead = useCallback(async () => {
    const ids = Array.from(selectedEmailIds);
    if (ids.length > 0) {
      await markAsRead(ids);
      clearSelection();
    }
  }, [selectedEmailIds, markAsRead, clearSelection]);

  const handleMarkAsUnread = useCallback(async () => {
    const ids = Array.from(selectedEmailIds);
    if (ids.length > 0) {
      await markAsUnread(ids);
      clearSelection();
    }
  }, [selectedEmailIds, markAsUnread, clearSelection]);

  const handleDelete = useCallback(async () => {
    const ids = Array.from(selectedEmailIds);
    if (ids.length > 0) {
      await deleteEmails(ids);
      clearSelection();
    }
  }, [selectedEmailIds, deleteEmails, clearSelection]);

  const handleArchive = useCallback(async () => {
    const ids = Array.from(selectedEmailIds);
    if (ids.length > 0) {
      await archiveEmails(ids);
      clearSelection();
    }
  }, [selectedEmailIds, archiveEmails, clearSelection]);

  const EmailRow = React.memo(
    ({ index, style }: { index: number; style: React.CSSProperties }) => {
      const email = currentAccountEmails[index];
      if (!email) return null;

      const isSelected = selectedEmailIds.has(email.id);
      const isCurrent = currentEmail?.id === email.id;

      return (
        <div
          style={style}
          className={`${styles.emailItem} ${
            isCurrent ? styles.emailItemSelected : ""
          } ${!email.isRead ? styles.emailItemUnread : ""}`}
          onClick={() => selectEmail(email)}
        >
          <Checkbox
            className={styles.checkbox}
            checked={isSelected}
            onChange={() => {
              toggleEmailSelection(email.id);
            }}
            onClick={(e) => e.stopPropagation()}
          />

          <Avatar
            className={styles.avatar}
            name={email.from.name || email.from.email}
            size={36}
            color="colorful"
          />

          <div className={styles.emailContent}>
            <div className={styles.emailHeader}>
              <div className={styles.emailSender}>
                <Text
                  className={styles.senderName}
                  weight={email.isRead ? "regular" : "semibold"}
                  size={300}
                >
                  {email.from.name || email.from.email}
                </Text>
                {email.to.length > 1 && (
                  <Caption1>+{email.to.length - 1}</Caption1>
                )}
              </div>
              <div className={styles.emailMeta}>
                {email.hasAttachments && (
                  <AttachRegular
                    fontSize={16}
                    color={tokens.colorNeutralForeground3}
                  />
                )}
                {email.isImportant && (
                  <Badge color="important" size="extra-small">
                    Important
                  </Badge>
                )}
                <Caption1>{formatEmailDate(email.date)}</Caption1>
              </div>
            </div>

            <Text
              className={styles.emailSubject}
              weight={email.isRead ? "regular" : "semibold"}
              size={300}
            >
              {email.subject}
            </Text>

            <Caption1 className={styles.emailPreview}>
              {email.preview || email.body.substring(0, 100)}
            </Caption1>

            {email.labels && email.labels.length > 0 && (
              <div className={styles.emailLabels}>
                {email.labels.slice(0, 3).map((label) => (
                  <Badge
                    key={label.id}
                    size="extra-small"
                    appearance="tint"
                    style={{
                      backgroundColor: label.color + "20",
                      color: label.color,
                    }}
                  >
                    {label.name}
                  </Badge>
                ))}
                {email.labels.length > 3 && (
                  <Caption1>+{email.labels.length - 3}</Caption1>
                )}
              </div>
            )}
          </div>

          <div
            onClick={(e) => e.stopPropagation()}
            style={{ display: "flex", gap: "4px" }}
          >
            <Tooltip
              content={email.isStarred ? "Unstar" : "Star"}
              relationship="label"
            >
              <Button
                appearance="subtle"
                icon={email.isStarred ? <StarIcon /> : <Star20Regular />}
                size="small"
                onClick={(e) => {
                  e.stopPropagation();
                  toggleStar(email.id);
                }}
              />
            </Tooltip>
            <Tooltip
              content={email.isFlagged ? "Unflag" : "Flag"}
              relationship="label"
            >
              <Button
                appearance="subtle"
                icon={email.isFlagged ? <FlagIcon /> : <Flag20Regular />}
                size="small"
                onClick={(e) => {
                  e.stopPropagation();
                  toggleFlag(email.id);
                }}
              />
            </Tooltip>
          </div>
        </div>
      );
    },
  );

  if (isLoading) {
    return (
      <div className={styles.root}>
        <div className={styles.loadingState}>
          <Spinner size="medium" label="Loading emails..." />
        </div>
      </div>
    );
  }

  if (currentAccountEmails.length === 0) {
    return (
      <div className={styles.root}>
        <div className={styles.emptyState}>
          <MailIcon
            style={{ fontSize: 48, color: tokens.colorNeutralForeground3 }}
          />
          <Text
            size={400}
            weight="semibold"
            style={{ marginTop: tokens.spacingVerticalL }}
          >
            No emails found
          </Text>
          <Caption1 style={{ marginTop: tokens.spacingVerticalS }}>
            Your mailbox is empty or no emails match your current filters
          </Caption1>
        </div>
      </div>
    );
  }

  return (
    <div className={styles.root}>
      <Toolbar className={styles.toolbar} size="small">
        <div className={styles.toolbarActions}>
          <Checkbox
            checked={isAllSelected || isSomeSelected}
            onChange={handleSelectAll}
          />
          {selectedEmailIds.size > 0 && (
            <>
              <Caption1>{selectedEmailIds.size} selected</Caption1>
              <ToolbarDivider />
              <ToolbarButton
                icon={<MailReadIcon />}
                onClick={handleMarkAsRead}
              />
              <ToolbarButton icon={<MailIcon />} onClick={handleMarkAsUnread} />
              <ToolbarButton
                icon={<Archive20Regular />}
                onClick={handleArchive}
              />
              <ToolbarButton
                icon={<Delete20Regular />}
                onClick={handleDelete}
              />
            </>
          )}
        </div>
        <Caption1>
          {t("emailList.totalEmails", { count: currentAccountEmails.length })}
        </Caption1>
      </Toolbar>

      <div className={styles.listContainer}>
        <List
          height={window.innerHeight - 120}
          itemCount={currentAccountEmails.length}
          itemSize={100}
          width="100%"
          overscanCount={5}
        >
          {EmailRow}
        </List>
      </div>
    </div>
  );
};
