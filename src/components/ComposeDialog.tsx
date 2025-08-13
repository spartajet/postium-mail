import React, { useState } from "react";
import {
  Dialog,
  DialogSurface,
  DialogContent,
  Button,
  Input,
  Textarea,
  makeStyles,
  tokens,
  shorthands,
  Text,
  Toolbar,
  ToolbarButton,
  ToolbarDivider,
  Label,
} from "@fluentui/react-components";
import {
  Dismiss24Regular,
  Send20Regular,
  Attach20Regular,
  TextBold20Regular,
  TextItalic20Regular,
  TextUnderline20Regular,
  TextBulletList20Regular,
  TextNumberListLtr20Regular,
  Link20Regular,
  Image20Regular,
  Save20Regular,
} from "@fluentui/react-icons";
import { useEmailStore } from "../stores/useEmailStore";

const useStyles = makeStyles({
  dialog: {
    maxWidth: "800px",
    width: "90vw",
    height: "80vh",
    maxHeight: "800px",
  },
  dialogContent: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
    ...shorthands.padding(0),
  },
  header: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    ...shorthands.padding(tokens.spacingVerticalM, tokens.spacingHorizontalL),
    borderBottom: `1px solid ${tokens.colorNeutralStroke1}`,
  },
  form: {
    display: "flex",
    flexDirection: "column",
    flex: 1,
    overflow: "hidden",
  },
  field: {
    display: "flex",
    alignItems: "center",
    ...shorthands.padding(tokens.spacingVerticalS, tokens.spacingHorizontalL),
    borderBottom: `1px solid ${tokens.colorNeutralStroke1}`,
  },
  fieldLabel: {
    minWidth: "60px",
    marginRight: tokens.spacingHorizontalM,
  },
  fieldInput: {
    flex: 1,
  },
  editorContainer: {
    flex: 1,
    display: "flex",
    flexDirection: "column",
    overflow: "hidden",
  },
  editorToolbar: {
    ...shorthands.padding(tokens.spacingVerticalXS, tokens.spacingHorizontalL),
    borderBottom: `1px solid ${tokens.colorNeutralStroke1}`,
  },
  editor: {
    flex: 1,
    ...shorthands.padding(tokens.spacingVerticalM, tokens.spacingHorizontalL),
    overflow: "auto",
  },
  footer: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    ...shorthands.padding(tokens.spacingVerticalM, tokens.spacingHorizontalL),
    borderTop: `1px solid ${tokens.colorNeutralStroke1}`,
  },
});

interface ComposeDialogProps {
  isOpen: boolean;
  onClose: () => void;
}

export const ComposeDialog: React.FC<ComposeDialogProps> = ({
  isOpen,
  onClose,
}) => {
  const styles = useStyles();
  const {
    composeDrafts,
    activeComposeDraftId,
    updateDraft,
    sendEmail,
    saveDraft,
  } = useEmailStore();

  const activeDraft = composeDrafts.find((d) => d.id === activeComposeDraftId);

  const [to, setTo] = useState(activeDraft?.to.join(", ") || "");
  const [cc, setCc] = useState(activeDraft?.cc?.join(", ") || "");
  const [bcc, setBcc] = useState(activeDraft?.bcc?.join(", ") || "");
  const [subject, setSubject] = useState(activeDraft?.subject || "");
  const [body, setBody] = useState(activeDraft?.body || "");
  const [showCc, setShowCc] = useState(!!activeDraft?.cc?.length);
  const [showBcc, setShowBcc] = useState(!!activeDraft?.bcc?.length);

  const handleSend = async () => {
    if (!activeComposeDraftId) return;

    updateDraft(activeComposeDraftId, {
      to: to
        .split(",")
        .map((e) => e.trim())
        .filter(Boolean),
      cc: cc
        ? cc
            .split(",")
            .map((e) => e.trim())
            .filter(Boolean)
        : undefined,
      bcc: bcc
        ? bcc
            .split(",")
            .map((e) => e.trim())
            .filter(Boolean)
        : undefined,
      subject,
      body,
    });

    await sendEmail(activeComposeDraftId);
    onClose();
  };

  const handleSaveDraft = async () => {
    if (!activeComposeDraftId) return;

    updateDraft(activeComposeDraftId, {
      to: to
        .split(",")
        .map((e) => e.trim())
        .filter(Boolean),
      cc: cc
        ? cc
            .split(",")
            .map((e) => e.trim())
            .filter(Boolean)
        : undefined,
      bcc: bcc
        ? bcc
            .split(",")
            .map((e) => e.trim())
            .filter(Boolean)
        : undefined,
      subject,
      body,
    });

    await saveDraft(activeComposeDraftId);
  };

  return (
    <Dialog open={isOpen} onOpenChange={(_, data) => !data.open && onClose()}>
      <DialogSurface className={styles.dialog}>
        <DialogContent className={styles.dialogContent}>
          <div className={styles.header}>
            <Text size={400} weight="semibold">
              New Message
            </Text>
            <Button
              appearance="subtle"
              icon={<Dismiss24Regular />}
              onClick={onClose}
            />
          </div>

          <div className={styles.form}>
            <div className={styles.field}>
              <Label className={styles.fieldLabel}>To:</Label>
              <Input
                className={styles.fieldInput}
                value={to}
                onChange={(_, data) => setTo(data.value)}
                placeholder="Recipients"
                appearance="underline"
              />
              <Button
                appearance="subtle"
                size="small"
                onClick={() => setShowCc(!showCc)}
              >
                Cc
              </Button>
              <Button
                appearance="subtle"
                size="small"
                onClick={() => setShowBcc(!showBcc)}
              >
                Bcc
              </Button>
            </div>

            {showCc && (
              <div className={styles.field}>
                <Label className={styles.fieldLabel}>Cc:</Label>
                <Input
                  className={styles.fieldInput}
                  value={cc}
                  onChange={(_, data) => setCc(data.value)}
                  placeholder="Cc recipients"
                  appearance="underline"
                />
              </div>
            )}

            {showBcc && (
              <div className={styles.field}>
                <Label className={styles.fieldLabel}>Bcc:</Label>
                <Input
                  className={styles.fieldInput}
                  value={bcc}
                  onChange={(_, data) => setBcc(data.value)}
                  placeholder="Bcc recipients"
                  appearance="underline"
                />
              </div>
            )}

            <div className={styles.field}>
              <Label className={styles.fieldLabel}>Subject:</Label>
              <Input
                className={styles.fieldInput}
                value={subject}
                onChange={(_, data) => setSubject(data.value)}
                placeholder="Subject"
                appearance="underline"
              />
            </div>

            <div className={styles.editorContainer}>
              <Toolbar className={styles.editorToolbar} size="small">
                <ToolbarButton icon={<TextBold20Regular />} />
                <ToolbarButton icon={<TextItalic20Regular />} />
                <ToolbarButton icon={<TextUnderline20Regular />} />
                <ToolbarDivider />
                <ToolbarButton icon={<TextBulletList20Regular />} />
                <ToolbarButton icon={<TextNumberListLtr20Regular />} />
                <ToolbarDivider />
                <ToolbarButton icon={<Link20Regular />} />
                <ToolbarButton icon={<Image20Regular />} />
                <ToolbarButton icon={<Attach20Regular />} />
              </Toolbar>

              <Textarea
                className={styles.editor}
                value={body}
                onChange={(_, data) => setBody(data.value)}
                placeholder="Write your message..."
                resize="none"
                appearance="outline"
              />
            </div>
          </div>

          <div className={styles.footer}>
            <div>
              <Button
                appearance="primary"
                icon={<Send20Regular />}
                onClick={handleSend}
                disabled={!to || !subject}
              >
                Send
              </Button>
              <Button
                appearance="subtle"
                icon={<Save20Regular />}
                onClick={handleSaveDraft}
                style={{ marginLeft: tokens.spacingHorizontalS }}
              >
                Save Draft
              </Button>
            </div>
            <div>
              <Button appearance="subtle" onClick={onClose}>
                Cancel
              </Button>
            </div>
          </div>
        </DialogContent>
      </DialogSurface>
    </Dialog>
  );
};
