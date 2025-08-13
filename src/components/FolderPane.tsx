import React from "react";
import { useTranslation } from "react-i18next";
import {
  makeStyles,
  tokens,
  shorthands,
  Tree,
  TreeItem,
  TreeItemLayout,
  CounterBadge,
  Button,
  Menu,
  MenuTrigger,
  MenuPopover,
  MenuList,
  MenuItem,
  Text,
  Caption1Strong,
} from "@fluentui/react-components";
import {
  Folder20Regular,
  Folder20Filled,
  Mail20Regular,
  Mail20Filled,
  Send20Regular,
  Send20Filled,
  Drafts20Regular,
  Drafts20Filled,
  Delete20Regular,
  Delete20Filled,
  Warning20Regular,
  Warning20Filled,
  Star20Regular,
  Star20Filled,
  Flag20Regular,
  Flag20Filled,
  Archive20Regular,
  Archive20Filled,
  MoreHorizontal20Regular,
  Add20Regular,
  Edit20Regular,
  bundleIcon,
} from "@fluentui/react-icons";
import { useEmailStore } from "../stores/useEmailStore";
import { FolderInfo } from "../types/email";

const FolderIcon = bundleIcon(Folder20Filled, Folder20Regular);
const MailIcon = bundleIcon(Mail20Filled, Mail20Regular);
const SendIcon = bundleIcon(Send20Filled, Send20Regular);
const DraftsIcon = bundleIcon(Drafts20Filled, Drafts20Regular);
const DeleteIcon = bundleIcon(Delete20Filled, Delete20Regular);
const WarningIcon = bundleIcon(Warning20Filled, Warning20Regular);
const StarIcon = bundleIcon(Star20Filled, Star20Regular);
const FlagIcon = bundleIcon(Flag20Filled, Flag20Regular);
const ArchiveIcon = bundleIcon(Archive20Filled, Archive20Regular);

const useStyles = makeStyles({
  root: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
    backgroundColor: tokens.colorNeutralBackground2,
    overflow: "hidden",
  },
  header: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    ...shorthands.padding(tokens.spacingVerticalM, tokens.spacingHorizontalM),
    borderBottom: `1px solid ${tokens.colorNeutralStroke1}`,
  },
  accountSection: {
    ...shorthands.padding(tokens.spacingVerticalS, tokens.spacingHorizontalM),
    borderBottom: `1px solid ${tokens.colorNeutralStroke1}`,
  },
  accountList: {
    display: "flex",
    flexDirection: "column",
    ...shorthands.gap(tokens.spacingVerticalXS),
    ...shorthands.margin(tokens.spacingVerticalS, "0"),
  },
  accountItem: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    ...shorthands.padding(tokens.spacingVerticalXS, tokens.spacingHorizontalS),
    ...shorthands.borderRadius(tokens.borderRadiusMedium),
    cursor: "pointer",
    "&:hover": {
      backgroundColor: tokens.colorNeutralBackground1Hover,
    },
  },
  accountItemActive: {
    backgroundColor: tokens.colorBrandBackground2,
    "&:hover": {
      backgroundColor: tokens.colorBrandBackground2Hover,
    },
  },
  accountInfo: {
    display: "flex",
    alignItems: "center",
    ...shorthands.gap(tokens.spacingHorizontalXS),
    flex: 1,
    minWidth: 0,
  },
  accountIndicator: {
    width: "8px",
    height: "8px",
    ...shorthands.borderRadius("50%"),
    flexShrink: 0,
  },
  accountName: {
    ...shorthands.overflow("hidden"),
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  folderTree: {
    flex: 1,
    overflowY: "auto",
    overflowX: "hidden",
    ...shorthands.padding(tokens.spacingVerticalS, "0"),
  },
  treeItem: {
    ...shorthands.padding("0", tokens.spacingHorizontalS),
  },
  treeItemContent: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    width: "100%",
    ...shorthands.gap(tokens.spacingHorizontalXS),
  },
  treeItemLabel: {
    display: "flex",
    alignItems: "center",
    ...shorthands.gap(tokens.spacingHorizontalS),
    flex: 1,
    minWidth: 0,
  },
  folderName: {
    flex: 1,
    ...shorthands.overflow("hidden"),
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  badge: {
    marginLeft: "auto",
  },
  labelSection: {
    ...shorthands.padding(tokens.spacingVerticalM, tokens.spacingHorizontalM),
    borderTop: `1px solid ${tokens.colorNeutralStroke1}`,
  },
  labelList: {
    display: "flex",
    flexDirection: "column",
    ...shorthands.gap(tokens.spacingVerticalXS),
    ...shorthands.margin(tokens.spacingVerticalS, "0"),
  },
  labelItem: {
    display: "flex",
    alignItems: "center",
    ...shorthands.gap(tokens.spacingHorizontalS),
    ...shorthands.padding(tokens.spacingVerticalXS, tokens.spacingHorizontalS),
    ...shorthands.borderRadius(tokens.borderRadiusMedium),
    cursor: "pointer",
    "&:hover": {
      backgroundColor: tokens.colorNeutralBackground1Hover,
    },
  },
  labelDot: {
    width: "12px",
    height: "12px",
    ...shorthands.borderRadius("50%"),
  },
});

const folderIcons: Record<string, React.ReactElement> = {
  inbox: <MailIcon />,
  sent: <SendIcon />,
  drafts: <DraftsIcon />,
  trash: <DeleteIcon />,
  spam: <WarningIcon />,
  starred: <StarIcon />,
  important: <FlagIcon />,
  archive: <ArchiveIcon />,
  all: <FolderIcon />,
};

export const FolderPane: React.FC = () => {
  const styles = useStyles();
  const { t } = useTranslation();
  const {
    accounts,
    activeAccountIds,
    currentAccountId,
    folders,
    currentFolderId,
    selectFolder,
    toggleAccountActive,
    setCurrentAccount,
    labels,
  } = useEmailStore();

  // Helper function to get localized folder names
  const getLocalizedFolderName = (
    folderName: string,
    folderType?: string,
  ): string => {
    const folderNameLower = folderName.toLowerCase();
    const folderTypeKey = folderType?.toLowerCase() || folderNameLower;

    // Map common folder names to translation keys
    const folderTranslationMap: { [key: string]: string } = {
      inbox: t("sidebar.inbox"),
      sent: t("sidebar.sent"),
      "sent items": t("sidebar.sent"),
      drafts: t("sidebar.drafts"),
      spam: t("sidebar.spam"),
      junk: t("sidebar.spam"),
      trash: t("sidebar.trash"),
      deleted: t("sidebar.trash"),
      "deleted items": t("sidebar.trash"),
      starred: t("sidebar.starred"),
      important: t("sidebar.important"),
      "all mail": t("sidebar.allMail"),
      archive: t("sidebar.archive"),
    };

    return (
      folderTranslationMap[folderTypeKey] ||
      folderTranslationMap[folderNameLower] ||
      folderName
    );
  };

  const handleAccountClick = (accountId: string) => {
    setCurrentAccount(accountId);
  };

  const handleAccountToggle = (accountId: string) => {
    toggleAccountActive(accountId);
  };

  const handleFolderClick = (folderId: string) => {
    selectFolder(folderId);
  };

  const renderFolderTree = (accountId: string, folderList: FolderInfo[]) => {
    const renderFolder = (
      folder: FolderInfo,
      level: number = 0,
    ): React.ReactElement => {
      const isSelected = folder.id === currentFolderId;
      const icon = folderIcons[folder.type] || <FolderIcon />;

      return (
        <TreeItem
          key={folder.id}
          itemType="leaf"
          value={folder.id}
          className={styles.treeItem}
        >
          <TreeItemLayout
            iconBefore={icon}
            aside={
              folder.unreadCount > 0 ? (
                <CounterBadge
                  count={folder.unreadCount}
                  color="important"
                  size="small"
                  appearance="filled"
                />
              ) : undefined
            }
            onClick={() => handleFolderClick(folder.id)}
          >
            <div className={styles.treeItemContent}>
              <Text
                className={styles.folderName}
                weight={isSelected ? "semibold" : "regular"}
              >
                {getLocalizedFolderName(folder.name, folder.type)}
              </Text>
            </div>
          </TreeItemLayout>
          {folder.children && folder.children.length > 0 && (
            <Tree>
              {folder.children.map((child) => renderFolder(child, level + 1))}
            </Tree>
          )}
        </TreeItem>
      );
    };

    return (
      <Tree aria-label={`Folders for ${accountId}`}>
        {folderList.map((folder) => renderFolder(folder))}
      </Tree>
    );
  };

  accounts.filter((account) => activeAccountIds.has(account.id));
  const currentAccountFolders = currentAccountId
    ? folders.get(currentAccountId) || []
    : [];
  const currentAccountLabels = currentAccountId
    ? labels.get(currentAccountId) || []
    : [];

  return (
    <div className={styles.root}>
      <div className={styles.header}>
        <Text weight="semibold">Folders</Text>
        <Button
          appearance="subtle"
          icon={<Add20Regular />}
          size="small"
          title="Add folder"
        />
      </div>

      {/* Multiple Account Section */}
      {accounts.length > 1 && (
        <div className={styles.accountSection}>
          <Caption1Strong>{t("sidebar.folders").toUpperCase()}</Caption1Strong>
          <div className={styles.accountList}>
            {accounts.map((account) => {
              const isActive = activeAccountIds.has(account.id);
              const isCurrent = account.id === currentAccountId;
              return (
                <div
                  key={account.id}
                  className={`${styles.accountItem} ${isCurrent ? styles.accountItemActive : ""}`}
                  onClick={() => handleAccountClick(account.id)}
                >
                  <div className={styles.accountInfo}>
                    <div
                      className={styles.accountIndicator}
                      style={{
                        backgroundColor: isActive
                          ? account.color || tokens.colorBrandBackground
                          : tokens.colorNeutralBackground6,
                      }}
                    />
                    <Text
                      className={styles.accountName}
                      size={200}
                      weight={isCurrent ? "semibold" : "regular"}
                    >
                      {account.name}
                    </Text>
                  </div>
                  {account.unreadCount ? (
                    <CounterBadge
                      count={account.unreadCount}
                      size="small"
                      color="important"
                    />
                  ) : null}
                  <Menu>
                    <MenuTrigger disableButtonEnhancement>
                      <Button
                        appearance="subtle"
                        icon={<MoreHorizontal20Regular />}
                        size="small"
                        onClick={(e) => e.stopPropagation()}
                      />
                    </MenuTrigger>
                    <MenuPopover>
                      <MenuList>
                        <MenuItem
                          onClick={() => handleAccountToggle(account.id)}
                        >
                          {isActive ? "Hide" : "Show"}
                        </MenuItem>
                        <MenuItem icon={<Edit20Regular />}>
                          Edit Account
                        </MenuItem>
                        <MenuItem icon={<Delete20Regular />}>
                          Remove Account
                        </MenuItem>
                      </MenuList>
                    </MenuPopover>
                  </Menu>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Folder Tree */}
      <div className={styles.folderTree}>
        {currentAccountFolders.length > 0 &&
          currentAccountId &&
          renderFolderTree(currentAccountId, currentAccountFolders)}
      </div>

      {/* Labels Section */}
      {currentAccountLabels.length > 0 && (
        <div className={styles.labelSection}>
          <Caption1Strong>{t("sidebar.labels").toUpperCase()}</Caption1Strong>
          <div className={styles.labelList}>
            {currentAccountLabels.map((label) => (
              <div key={label.id} className={styles.labelItem}>
                <div
                  className={styles.labelDot}
                  style={{ backgroundColor: label.color }}
                />
                <Text size={200}>{label.name}</Text>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
};
