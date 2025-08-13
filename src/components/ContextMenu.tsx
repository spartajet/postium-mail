import React, { useEffect, useRef } from "react";
import {
  makeStyles,
  tokens,
  shorthands,
  MenuList,
  MenuItem,
  MenuDivider,
  MenuPopover,
} from "@fluentui/react-components";
import {
  Mail20Regular,
  MailRead20Regular,
  Star20Regular,
  Flag20Regular,
  Delete20Regular,
  Archive20Regular,
  Copy20Regular,
  ArrowReply20Regular,
  ArrowForward20Regular,
  Folder20Regular,
  Warning20Regular,
} from "@fluentui/react-icons";
import { useTranslation } from "react-i18next";
import { Email } from "../types/email";

const useStyles = makeStyles({
  menuPopover: {
    minWidth: "200px",
    boxShadow: tokens.shadow16,
    ...shorthands.borderRadius(tokens.borderRadiusMedium),
  },
});

interface ContextMenuProps {
  isOpen: boolean;
  position: { x: number; y: number };
  onClose: () => void;
  email?: Email | null;
  selectedEmails?: Email[];
  onAction?: (action: string, emails?: Email[]) => void;
}

export const ContextMenu: React.FC<ContextMenuProps> = ({
  isOpen,
  position,
  onClose,
  email,
  selectedEmails = [],
  onAction,
}) => {
  const styles = useStyles();
  const { t } = useTranslation();
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (isOpen && menuRef.current) {
      // Adjust position if menu would go off-screen
      const menuRect = menuRef.current.getBoundingClientRect();
      const adjustedX = Math.min(
        position.x,
        window.innerWidth - menuRect.width - 10,
      );
      const adjustedY = Math.min(
        position.y,
        window.innerHeight - menuRect.height - 10,
      );

      if (menuRef.current) {
        menuRef.current.style.left = `${adjustedX}px`;
        menuRef.current.style.top = `${adjustedY}px`;
      }
    }
  }, [isOpen, position]);

  const handleAction = (action: string) => {
    if (onAction) {
      const targetEmails =
        selectedEmails.length > 0 ? selectedEmails : email ? [email] : [];
      onAction(action, targetEmails);
    }
    onClose();
  };

  if (!isOpen) return null;

  const hasMultipleSelection = selectedEmails.length > 1;
  const targetEmail = email || selectedEmails[0];

  return (
    <div
      ref={menuRef}
      style={{
        position: "fixed",
        left: position.x,
        top: position.y,
        zIndex: 10000,
      }}
      onContextMenu={(e) => e.preventDefault()}
    >
      <MenuPopover className={styles.menuPopover}>
        <MenuList>
          {targetEmail && (
            <>
              <MenuItem
                icon={<ArrowReply20Regular />}
                onClick={() => handleAction("reply")}
                disabled={hasMultipleSelection}
              >
                {t("emailDetail.reply")}
              </MenuItem>
              <MenuItem
                icon={<ArrowForward20Regular />}
                onClick={() => handleAction("forward")}
                disabled={hasMultipleSelection}
              >
                {t("emailDetail.forward")}
              </MenuItem>
              <MenuDivider />
            </>
          )}

          <MenuItem
            icon={
              targetEmail?.isRead ? <Mail20Regular /> : <MailRead20Regular />
            }
            onClick={() =>
              handleAction(targetEmail?.isRead ? "markUnread" : "markRead")
            }
          >
            {targetEmail?.isRead
              ? t("emailList.markAsUnread")
              : t("emailList.markAsRead")}
          </MenuItem>

          <MenuItem
            icon={<Star20Regular />}
            onClick={() => handleAction("toggleStar")}
          >
            {targetEmail?.isStarred
              ? t("emailList.unstar")
              : t("emailList.star")}
          </MenuItem>

          <MenuItem
            icon={<Flag20Regular />}
            onClick={() => handleAction("toggleFlag")}
          >
            {targetEmail?.isFlagged
              ? t("emailList.unflag")
              : t("emailList.flag")}
          </MenuItem>

          <MenuDivider />

          <MenuItem
            icon={<Folder20Regular />}
            onClick={() => handleAction("moveToFolder")}
          >
            {t("emailDetail.moveToFolder")}
          </MenuItem>

          <MenuItem
            icon={<Archive20Regular />}
            onClick={() => handleAction("archive")}
          >
            {t("emailList.archive")}
          </MenuItem>

          <MenuItem
            icon={<Warning20Regular />}
            onClick={() => handleAction("spam")}
          >
            {t("emailList.spam")}
          </MenuItem>

          <MenuItem
            icon={<Delete20Regular />}
            onClick={() => handleAction("delete")}
          >
            {t("emailList.moveToTrash")}
          </MenuItem>

          {hasMultipleSelection && (
            <>
              <MenuDivider />
              <MenuItem disabled>
                {t("emailList.selected", { count: selectedEmails.length })}
              </MenuItem>
            </>
          )}

          <MenuDivider />

          <MenuItem
            icon={<Copy20Regular />}
            onClick={() => handleAction("copy")}
            disabled={hasMultipleSelection}
          >
            {t("app.copy", "Copy")}
          </MenuItem>
        </MenuList>
      </MenuPopover>
    </div>
  );
};

interface useContextMenuReturn {
  contextMenu: {
    isOpen: boolean;
    position: { x: number; y: number };
    email: Email | null;
  };
  openContextMenu: (e: React.MouseEvent, email?: Email) => void;
  closeContextMenu: () => void;
}

export const useContextMenu = (): useContextMenuReturn => {
  const [contextMenu, setContextMenu] = React.useState<{
    isOpen: boolean;
    position: { x: number; y: number };
    email: Email | null;
  }>({
    isOpen: false,
    position: { x: 0, y: 0 },
    email: null,
  });

  const openContextMenu = (e: React.MouseEvent, email?: Email) => {
    e.preventDefault();
    e.stopPropagation();

    setContextMenu({
      isOpen: true,
      position: { x: e.clientX, y: e.clientY },
      email: email || null,
    });
  };

  const closeContextMenu = () => {
    setContextMenu((prev) => ({ ...prev, isOpen: false }));
  };

  useEffect(() => {
    const handleClick = () => closeContextMenu();
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === "Escape") closeContextMenu();
    };

    if (contextMenu.isOpen) {
      document.addEventListener("click", handleClick);
      document.addEventListener("keydown", handleEscape);
    }

    return () => {
      document.removeEventListener("click", handleClick);
      document.removeEventListener("keydown", handleEscape);
    };
  }, [contextMenu.isOpen]);

  return {
    contextMenu,
    openContextMenu,
    closeContextMenu,
  };
};
