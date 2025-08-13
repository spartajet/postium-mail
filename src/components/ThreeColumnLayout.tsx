import React, { useState, useCallback, useEffect } from "react";
import { useTranslation } from "react-i18next";
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
  LocalLanguage20Regular,
  LocalLanguage20Filled,
  bundleIcon,
} from "@fluentui/react-icons";
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels";
import { FolderPane } from "./FolderPane";
import { EmailListPane } from "./EmailListPane";
import { EmailDetailPane } from "./EmailDetailPane";
import { ComposeDialog } from "./ComposeDialog";
import { useEmailStore } from "../stores/useEmailStore";
import { uiLogger } from "../utils/logger";
import { changeLanguage, supportedLanguages } from "../i18n";
import { Checkmark20Regular } from "@fluentui/react-icons";

const MailIcon = bundleIcon(Mail20Filled, Mail20Regular);
const ComposeIcon = bundleIcon(Compose20Filled, Compose20Regular);
const SettingsIcon = bundleIcon(Settings20Filled, Settings20Regular);
const PersonIcon = bundleIcon(Person20Filled, Person20Regular);
const ArrowSyncIcon = bundleIcon(ArrowSync20Filled, ArrowSync20Regular);
const FilterIcon = bundleIcon(Filter20Filled, Filter20Regular);
const NavigationIcon = bundleIcon(Navigation20Filled, Navigation20Regular);
const LocalLanguageIcon = bundleIcon(
  LocalLanguage20Filled,
  LocalLanguage20Regular,
);

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
    width: "2px",
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
      left: "-3px",
      right: "-3px",
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

// Panel sizes stored in localStorage
const PANEL_SIZES_KEY = "postium-mail-panel-sizes";

const getStoredPanelSizes = () => {
  try {
    const stored = localStorage.getItem(PANEL_SIZES_KEY);
    if (stored) {
      return JSON.parse(stored);
    }
  } catch (error) {
    console.error("Failed to load panel sizes", error);
  }
  return null;
};

const savePanelSizes = (sizes: number[]) => {
  try {
    localStorage.setItem(PANEL_SIZES_KEY, JSON.stringify(sizes));
  } catch (error) {
    console.error("Failed to save panel sizes", error);
  }
};

export const ThreeColumnLayout: React.FC = () => {
  const styles = useStyles();
  const { t, i18n } = useTranslation();
  const [isDarkTheme, setIsDarkTheme] = useState(false);
  const [isComposeOpen, setIsComposeOpen] = useState(false);
  const [showFolderPane, setShowFolderPane] = useState(true);

  const {
    accounts,
    currentAccountId,
    currentEmail,
    selectEmail,
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

  // Set first email as current when emails are loaded
  useEffect(() => {
    if (!currentEmail && currentAccountEmails.length > 0) {
      // Select the first email by default
      selectEmail(currentAccountEmails[0]);
      uiLogger.info("Auto-selected first email");
    }
  }, [currentAccountEmails, currentEmail, selectEmail]);

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

  const handleLanguageChange = async (langCode: string) => {
    await changeLanguage(langCode);
    await uiLogger.info(`Language changed to ${langCode}`);
  };

  const handlePanelResize = (sizes: number[]) => {
    savePanelSizes(sizes);
  };

  const theme = isDarkTheme ? webDarkTheme : webLightTheme;

  // Get stored panel sizes or use defaults
  const storedSizes = getStoredPanelSizes();
  const defaultSizes = showFolderPane
    ? [20, 35, 45] // folder, list, detail
    : [50, 50]; // list, detail when folder is hidden

  return (
    <FluentProvider theme={theme}>
      <div className={styles.root}>
        {/* Header */}
        <div className={styles.header}>
          <div className={styles.headerLeft}>
            <Tooltip
              content={t("header.toggleNavigation")}
              relationship="label"
            >
              <ToolbarButton
                icon={<NavigationIcon />}
                onClick={() => setShowFolderPane(!showFolderPane)}
                appearance="subtle"
              />
            </Tooltip>
            <div className={styles.logo}>
              <MailIcon />
              <Text className={styles.logoText}>{t("app.name")}</Text>
            </div>
            <Button
              appearance="primary"
              icon={<ComposeIcon />}
              onClick={handleCompose}
            >
              {t("header.compose")}
            </Button>
          </div>

          <div className={styles.headerCenter}>
            <SearchBox
              className={styles.searchBox}
              placeholder={t("header.searchPlaceholder")}
              value={searchTerm}
              onChange={(_, data) => handleSearch(data.value)}
              appearance="filled-lighter"
            />
          </div>

          <div className={styles.headerRight}>
            <Toolbar size="small">
              <Tooltip
                content={
                  isSyncing ? t("header.syncing") : t("header.syncAllAccounts")
                }
                relationship="label"
              >
                <ToolbarButton
                  icon={isSyncing ? <Spinner size="tiny" /> : <ArrowSyncIcon />}
                  onClick={handleSync}
                  disabled={isSyncing}
                />
              </Tooltip>
              <ToolbarButton icon={<FilterIcon />} />

              {/* Language Selector */}
              <Menu>
                <MenuTrigger disableButtonEnhancement>
                  <ToolbarButton icon={<LocalLanguageIcon />} />
                </MenuTrigger>
                <MenuPopover>
                  <MenuList>
                    {supportedLanguages.map((lang) => (
                      <MenuItem
                        key={lang.code}
                        onClick={() => handleLanguageChange(lang.code)}
                        icon={
                          i18n.language === lang.code ? (
                            <Checkmark20Regular />
                          ) : undefined
                        }
                      >
                        {lang.nativeName}
                      </MenuItem>
                    ))}
                  </MenuList>
                </MenuPopover>
              </Menu>

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
                      <MenuItem icon={<PersonIcon />}>
                        {t("account.addAccount")}
                      </MenuItem>
                      <MenuItem icon={<SettingsIcon />}>
                        {t("account.manageAccounts")}
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
            onLayout={handlePanelResize}
          >
            {/* Folder Pane */}
            {showFolderPane && (
              <>
                <Panel
                  defaultSize={storedSizes ? storedSizes[0] : defaultSizes[0]}
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
              defaultSize={
                storedSizes
                  ? showFolderPane
                    ? storedSizes[1]
                    : storedSizes[0]
                  : showFolderPane
                    ? defaultSizes[1]
                    : defaultSizes[0]
              }
              minSize={25}
              maxSize={60}
              className={styles.panel}
            >
              <EmailListPane />
            </Panel>

            {/* Email Detail Pane - Always shown, displays message when no email selected */}
            <PanelResizeHandle className={styles.resizeHandle} />
            <Panel
              defaultSize={
                storedSizes
                  ? showFolderPane
                    ? storedSizes[2]
                    : storedSizes[1]
                  : showFolderPane
                    ? defaultSizes[2]
                    : defaultSizes[1]
              }
              minSize={30}
              className={styles.panel}
            >
              <EmailDetailPane />
            </Panel>
          </PanelGroup>
        </div>

        {/* Status Bar */}
        <div className={styles.statusBar}>
          <div>
            {selectedEmailIds.size > 0
              ? t("emailList.selected", { count: selectedEmailIds.size })
              : t("emailList.totalEmails", {
                  count: currentAccountEmails.length,
                })}
          </div>
          <div className={styles.syncIndicator}>
            {isSyncing && (
              <>
                <Spinner size="extra-tiny" />
                <Caption1>{t("statusBar.syncing")}</Caption1>
              </>
            )}
            {!isSyncing && accounts.length > 0 && (
              <Caption1>
                {t("account.activeAccounts", {
                  active: accounts.filter((a) => a.isActive).length,
                  total: accounts.length,
                })}
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
