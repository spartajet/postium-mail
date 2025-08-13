import React, { useState, useCallback } from "react";
import {
  FluentProvider,
  webLightTheme,
  webDarkTheme,
  makeStyles,
  tokens,
  shorthands,
  Button,
  Toolbar,
  ToolbarButton,
  ToolbarDivider,
  Menu,
  MenuTrigger,
  MenuPopover,
  MenuList,
  MenuItem,
  Tooltip,
  Badge,
  Avatar,
  Text,
  Caption1,
  SearchBox,
  Divider,
  Spinner,
} from "@fluentui/react-components";
import {
  Mail20Regular,
  Mail20Filled,
  Compose20Regular,
  Compose20Filled,
  Settings20Regular,
  Settings20Filled,
  Person20Regular,
  Person20Filled,
  ArrowSync20Regular,
  ArrowSync20Filled,
  Filter20Regular,
  Filter20Filled,
  Navigation20Regular,
  Navigation20Filled,
  bundleIcon,
} from "@fluentui/react-icons";
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels";
import { FolderPane } from "./FolderPane";
import { EmailListPane } from "./EmailListPane";
import { EmailDetailPane } from "./EmailDetailPane";
import { ComposeDialog } from "./ComposeDialog";
import { useEmailStore } from "../stores/useEmailStore";
import { uiLogger } from "../utils/logger";

const MailIcon = bundleIcon(Mail20Filled, Mail20Regular);
const ComposeIcon = bundleIcon(Compose20Filled, Compose20Regular);
const SettingsIcon = bundleIcon(Settings20Filled, Settings20Regular);
const PersonIcon = bundleIcon(Person20Filled, Person20Regular);
const ArrowSyncIcon = bundleIcon(ArrowSync20Filled, ArrowSync20Regular);
const FilterIcon = bundleIcon(Filter20Filled, Filter20Regular);
const NavigationIcon = bundleIcon(Navigation20Filled, Navigation20Regular);

const useStyles = makeStyles({
  root: {
    display: "flex",
    flexDirection: "column",
    height: "100vh",
    backgroundColor: tokens.colorNeutralBackground1,
    overflow: "hidden",
  },
  header: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    height: "48px",
    backgroundColor: tokens.colorNeutralBackground1,
    borderBottom: `1px solid ${tokens.colorNeutralStroke1}`,
    ...shorthands.padding("0", tokens.spacingHorizontalL),
  },
  headerLeft: {
    display: "flex",
    alignItems: "center",
    ...shorthands.gap(tokens.spacingHorizontalM),
  },
  headerCenter: {
    display: "flex",
    alignItems: "center",
    flex: 1,
    justifyContent: "center",
    ...shorthands.padding("0", tokens.spacingHorizontalXL),
  },
  headerRight: {
    display: "flex",
    alignItems: "center",
    ...shorthands.gap(tokens.spacingHorizontalS),
  },
  logo: {
    display: "flex",
    alignItems: "center",
    ...shorthands.gap(tokens.spacingHorizontalS),
  },
  logoText: {
    fontWeight: tokens.fontWeightSemibold,
    fontSize: tokens.fontSizeBase400,
  },
  content: {
    display: "flex",
    flex: 1,
    overflow: "hidden",
  },
  panelGroup: {
    width: "100%",
    height: "100%",
  },
  panel: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
    overflow: "hidden",
  },
  resizeHandle: {
    width: "1px",
    backgroundColor: tokens.colorNeutralStroke1,
    transition: "background-color 0.2s",
    cursor: "col-resize",
    position: "relative",
    "&:hover": {
      backgroundColor: tokens.colorBrandBackground,
    },
    "&::after": {
      content: '""',
      position: "absolute",
      top: 0,
      bottom: 0,
      left: "-2px",
      right: "-2px",
    },
  },
  accountBadge: {
    display: "flex",
    alignItems: "center",
    ...shorthands.gap(tokens.spacingHorizontalS),
    ...shorthands.padding(tokens.spacingVerticalS, tokens.spacingHorizontalM),
    ...shorthands.borderRadius(tokens.borderRadiusMedium),
    cursor: "pointer",
    "&:hover": {
      backgroundColor: tokens.colorNeutralBackground1Hover,
    },
  },
  syncIndicator: {
    display: "flex",
    alignItems: "center",
    ...shorthands.gap(tokens.spacingHorizontalXS),
  },
  searchBox: {
    minWidth: "300px",
    maxWidth: "500px",
  },
  statusBar: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    height: "24px",
    backgroundColor: tokens.colorNeutralBackground2,
    borderTop: `1px solid ${tokens.colorNeutralStroke1}`,
    ...shorthands.padding("0", tokens.spacingHorizontalM),
    fontSize: tokens.fontSizeBase200,
    color: tokens.colorNeutralForeground3,
  },
});

export const ThreeColumnLayout: React.FC = () => {
  const styles = useStyles();
  const [isDarkTheme, setIsDarkTheme] = useState(false);
  const [isComposeOpen, setIsComposeOpen] = useState(false);
  const [showFolderPane, setShowFolderPane] = useState(true);
  const [showDetailPane] = useState(true);

  const {
    accounts,
    currentAccountId,
    currentEmail,
    isSyncing,
    searchTerm,
    setSearchTerm,
    syncAllAccounts,
    createDraft,
    emails,
    selectedEmailIds,
  } = useEmailStore();

  const currentAccount = accounts.find((a) => a.id === currentAccountId);
  const currentAccountEmails = currentAccountId
    ? emails.get(currentAccountId) || []
    : [];

  const handleCompose = useCallback(async () => {
    await uiLogger.info("Opening compose dialog");
    createDraft();
    setIsComposeOpen(true);
  }, [createDraft]);

  const handleSync = useCallback(async () => {
    await uiLogger.info("Starting sync for all accounts");
    await syncAllAccounts();
  }, [syncAllAccounts]);

  const handleSearch = useCallback(
    (value: string) => {
      setSearchTerm(value);
    },
    [setSearchTerm],
  );

  const toggleTheme = () => {
    setIsDarkTheme(!isDarkTheme);
  };

  const theme = isDarkTheme ? webDarkTheme : webLightTheme;

  return (
    <FluentProvider theme={theme}>
      <div className={styles.root}>
        {/* Header */}
        <div className={styles.header}>
          <div className={styles.headerLeft}>
            <ToolbarButton
              icon={<NavigationIcon />}
              onClick={() => setShowFolderPane(!showFolderPane)}
              appearance="subtle"
            />
            <div className={styles.logo}>
              <MailIcon />
              <Text className={styles.logoText}>Postium Mail</Text>
            </div>
            <Button
              appearance="primary"
              icon={<ComposeIcon />}
              onClick={handleCompose}
            >
              Compose
            </Button>
          </div>

          <div className={styles.headerCenter}>
            <SearchBox
              className={styles.searchBox}
              placeholder="Search emails..."
              value={searchTerm}
              onChange={(_, data) => handleSearch(data.value)}
              appearance="filled-lighter"
            />
          </div>

          <div className={styles.headerRight}>
            <Toolbar size="small">
              <Tooltip
                content={isSyncing ? "Syncing..." : "Sync all accounts"}
                relationship="label"
              >
                <ToolbarButton
                  icon={isSyncing ? <Spinner size="tiny" /> : <ArrowSyncIcon />}
                  onClick={handleSync}
                  disabled={isSyncing}
                />
              </Tooltip>
              <ToolbarButton icon={<FilterIcon />} />
              <ToolbarDivider />

              {currentAccount && (
                <Menu>
                  <MenuTrigger disableButtonEnhancement>
                    <div className={styles.accountBadge}>
                      <Avatar
                        name={currentAccount.name}
                        size={24}
                        color="colorful"
                        badge={
                          currentAccount.unreadCount
                            ? { status: "available", outOfOffice: false }
                            : undefined
                        }
                      />
                      <div>
                        <Text size={200}>{currentAccount.name}</Text>
                        {currentAccount.unreadCount ? (
                          <Badge
                            appearance="filled"
                            color="important"
                            size="extra-small"
                          >
                            {currentAccount.unreadCount}
                          </Badge>
                        ) : null}
                      </div>
                    </div>
                  </MenuTrigger>
                  <MenuPopover>
                    <MenuList>
                      {accounts.map((account) => (
                        <MenuItem
                          key={account.id}
                          icon={
                            <Avatar
                              name={account.name}
                              size={20}
                              color="colorful"
                            />
                          }
                        >
                          <div>
                            <Text>{account.name}</Text>
                            <Caption1 block color="subtle">
                              {account.email}
                            </Caption1>
                          </div>
                        </MenuItem>
                      ))}
                      <Divider />
                      <MenuItem icon={<PersonIcon />}>Add Account</MenuItem>
                      <MenuItem icon={<SettingsIcon />}>
                        Manage Accounts
                      </MenuItem>
                    </MenuList>
                  </MenuPopover>
                </Menu>
              )}

              <ToolbarButton icon={<SettingsIcon />} />
              <ToolbarButton
                icon={isDarkTheme ? "🌙" : "☀️"}
                onClick={toggleTheme}
              />
            </Toolbar>
          </div>
        </div>

        {/* Main Content - Three Column Layout */}
        <div className={styles.content}>
          <PanelGroup
            direction="horizontal"
            className={styles.panelGroup}
            autoSaveId="email-layout"
          >
            {/* Folder Pane */}
            {showFolderPane && (
              <>
                <Panel
                  defaultSize={20}
                  minSize={15}
                  maxSize={30}
                  className={styles.panel}
                >
                  <FolderPane />
                </Panel>
                <PanelResizeHandle className={styles.resizeHandle} />
              </>
            )}

            {/* Email List Pane */}
            <Panel
              defaultSize={showDetailPane ? 35 : 60}
              minSize={25}
              maxSize={60}
              className={styles.panel}
            >
              <EmailListPane />
            </Panel>

            {/* Email Detail Pane */}
            {showDetailPane && currentEmail && (
              <>
                <PanelResizeHandle className={styles.resizeHandle} />
                <Panel defaultSize={45} minSize={30} className={styles.panel}>
                  <EmailDetailPane />
                </Panel>
              </>
            )}
          </PanelGroup>
        </div>

        {/* Status Bar */}
        <div className={styles.statusBar}>
          <div>
            {selectedEmailIds.size > 0
              ? `${selectedEmailIds.size} selected`
              : `${currentAccountEmails.length} emails`}
          </div>
          <div className={styles.syncIndicator}>
            {isSyncing && (
              <>
                <Spinner size="extra-tiny" />
                <Caption1>Syncing...</Caption1>
              </>
            )}
            {!isSyncing && accounts.length > 0 && (
              <Caption1>
                {accounts.filter((a) => a.isActive).length} of {accounts.length}{" "}
                accounts active
              </Caption1>
            )}
          </div>
        </div>

        {/* Compose Dialog */}
        {isComposeOpen && (
          <ComposeDialog
            isOpen={isComposeOpen}
            onClose={() => setIsComposeOpen(false)}
          />
        )}
      </div>
    </FluentProvider>
  );
};
