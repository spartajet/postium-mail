<script lang="ts">
  import { getContext } from 'svelte';
  import { getI18nState } from '$lib/stores/i18n.svelte';
  import { getEmailState } from '$lib/stores/email.svelte';
  import type ComposeModal from './ComposeModal.svelte';
  import DOMPurify from 'dompurify';
  import { ChevronUp, ChevronDown, Layers, Paperclip, Reply, Forward, Star, Archive, Trash2, Mail } from 'lucide-svelte';

  const COMPOSE_MODAL_KEY = Symbol.for('compose-modal');
  const getComposeModal = getContext<() => ComposeModal | undefined>(COMPOSE_MODAL_KEY);

  const i18n = getI18nState();
  const t = $derived(i18n.t);
  const emailState = getEmailState();

  let aiSummary = $state('');
  let isLoadingSummary = $state(false);

  function getInitials(name: string): string {
    return name.charAt(0).toUpperCase();
  }

  function formatFullDate(timestamp: number): string {
    return new Date(timestamp * 1000).toLocaleString(undefined, {
      year: 'numeric', month: '2-digit', day: '2-digit',
      hour: '2-digit', minute: '2-digit',
    });
  }

  async function loadAISummary() {
    if (!emailState.selectedEmail) return;
    isLoadingSummary = true;
    try {
      await new Promise(resolve => setTimeout(resolve, 800));
      aiSummary = '• 邮件主要内容摘要\n• 需要关注的关键点\n• 建议的后续行动';
    } finally {
      isLoadingSummary = false;
    }
  }

  function handleToggleStar() {
    if (emailState.selectedEmail) {
      emailState.toggleStar(emailState.selectedEmail.id);
    }
  }

  function handleDelete() {
    if (emailState.selectedEmail) {
      emailState.deleteEmails([emailState.selectedEmail.id]);
    }
  }

  // H-03: 使用 stale flag 防止 race condition
  let summaryRequestId = 0;
  $effect(() => {
    const email = emailState.selectedEmail;
    if (email) {
      aiSummary = '';
      const currentId = ++summaryRequestId;
      isLoadingSummary = true;
      new Promise(resolve => setTimeout(resolve, 800)).then(() => {
        if (currentId === summaryRequestId) {
          aiSummary = '• 邮件主要内容摘要\n• 需要关注的关键点\n• 建议的后续行动';
          isLoadingSummary = false;
        }
      });
    }
  });

  const aiActions = $derived([
    { id: 'reply', label: t.ai.smartReply },
    { id: 'summary', label: t.ai.summary },
    { id: 'translate', label: t.ai.translate },
    { id: 'tasks', label: t.ai.tasks },
  ]);

  function getFileExtensionColor(filename: string): string {
    const ext = filename.split('.').pop()?.toLowerCase() || '';
    const colors: Record<string, string> = {
      pdf: '#F40F02', jpg: '#9C27B0', jpeg: '#9C27B0', png: '#9C27B0',
      gif: '#9C27B0', svg: '#9C27B0', webp: '#9C27B0',
      mp4: '#FF9800', avi: '#FF9800', mov: '#FF9800', mkv: '#FF9800',
      mp3: '#2196F3', wav: '#2196F3', flac: '#2196F3', aac: '#2196F3',
      doc: '#2B579A', docx: '#2B579A', xls: '#217346', xlsx: '#217346',
      ppt: '#D24726', pptx: '#D24726',
      txt: '#757575', zip: '#4CAF50', rar: '#4CAF50', '7z': '#4CAF50',
    };
    return colors[ext] || '#757575';
  }

  function getFileExtensionLabel(filename: string): string {
    return filename.split('.').pop()?.toUpperCase() || 'FILE';
  }
</script>

<div class="detail-panel flex h-full flex-1 flex-col bg-background">
  {#if emailState.selectedEmail}
    <!-- Top toolbar: nav arrows -->
    <div class="flex items-center justify-between border-b border-border px-5 py-2">
      <div class="flex items-center gap-1">
        <button class="icon-btn-sm flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground" title={t.email.from}>
          <ChevronUp size={20} />
        </button>
        <button class="icon-btn-sm flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground" title={t.email.to}>
          <ChevronDown size={20} />
        </button>
      </div>
    </div>

    <!-- Subject -->
    <div class="border-b border-border px-5 py-4">
      <h2 class="text-xl font-semibold text-foreground">
        {emailState.selectedEmail.subject || '(No Subject)'}
      </h2>
    </div>

    <!-- Sender meta -->
    <div class="flex items-center gap-3 border-b border-border px-5 py-3">
      <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-primary/15 text-sm font-semibold text-primary">
        {getInitials(emailState.selectedEmail.sender_name || emailState.selectedEmail.sender_email)}
      </div>
      <div class="min-w-0 flex-1">
        <div class="flex items-center gap-2">
          <span class="text-sm font-medium text-foreground">
            {emailState.selectedEmail.sender_name || emailState.selectedEmail.sender_email}
          </span>
          <span class="truncate text-xs text-muted-foreground">
            &lt;{emailState.selectedEmail.sender_email}&gt;
          </span>
        </div>
        <div class="mt-0.5 text-xs text-muted-foreground">
          {t.email.to}: {emailState.selectedEmail.recipient_emails}
          {#if emailState.selectedEmail.cc_emails}
            &nbsp;|&nbsp; {t.email.cc}: {emailState.selectedEmail.cc_emails}
          {/if}
        </div>
      </div>
      <span class="shrink-0 text-xs text-muted-foreground">
        {formatFullDate(emailState.selectedEmail.sent_at)}
      </span>
    </div>

    <!-- Scrollable content area -->
    <div class="flex-1 overflow-y-auto">
      <!-- AI Summary card -->
      {#if isLoadingSummary || aiSummary}
        <div class="ai-card mx-5 mt-4 rounded-lg border border-primary/20 bg-primary/5">
          <div class="flex items-center gap-2 px-4 py-2.5">
            <Layers size={16} class="text-primary" />
            <span class="text-xs font-semibold text-primary">{t.ai.summary}</span>
          </div>
          <div class="border-t border-primary/10 px-4 py-3">
            {#if isLoadingSummary}
              <div class="space-y-2">
                <div class="skeleton skeleton-text" style="width: 90%"></div>
                <div class="skeleton skeleton-text" style="width: 75%"></div>
                <div class="skeleton skeleton-text" style="width: 60%"></div>
              </div>
            {:else}
              <ul class="space-y-1">
                {#each aiSummary.split('\n') as line}
                  <li class="text-sm text-foreground/80">{line.replace('• ', '')}</li>
                {/each}
              </ul>
            {/if}
          </div>
        </div>
      {/if}

      <!-- Email body -->
      <div class="px-5 py-4">
        {#if emailState.selectedEmail.body_html}
          {@html DOMPurify.sanitize(emailState.selectedEmail.body_html)}
        {:else}
          <pre class="whitespace-pre-wrap text-sm leading-relaxed text-foreground/90">{emailState.selectedEmail.body_text || ''}</pre>
        {/if}
      </div>

      <!-- Attachments -->
      {#if emailState.selectedEmail.has_attachments}
        <div class="border-t border-border px-5 py-4">
          <div class="mb-3 flex items-center gap-2 text-sm font-medium text-muted-foreground">
            <Paperclip size={18} />
            <span>{t.email.attachments}</span>
          </div>
          <div class="flex flex-wrap gap-3">
            <!-- Placeholder attachment items -->
            <div class="attachment-item flex items-center gap-2 rounded-lg border border-border bg-glass px-3 py-2 transition-colors hover:border-primary hover:bg-glass-hover">
              <div class="flex h-8 w-8 items-center justify-center rounded bg-blue-500/10 text-xs font-bold text-blue-500">PDF</div>
              <div class="min-w-0">
                <div class="truncate text-sm font-medium text-foreground">document.pdf</div>
                <div class="text-xs text-muted-foreground">2.4 MB</div>
              </div>
            </div>
          </div>
        </div>
      {/if}
    </div>

    <!-- Action buttons -->
    <div class="flex items-center gap-1 border-t border-border px-5 py-2">
      <button class="icon-btn-sm flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground" title={t.email.reply} onclick={() => {
        const modal = getComposeModal?.();
        if (emailState.selectedEmail && modal) {
          modal.showReply(
            emailState.selectedEmail.sender_email,
            emailState.selectedEmail.subject || '',
            emailState.selectedEmail.body_text || ''
          );
        }
      }}>
        <Reply size={18} />
      </button>
      <button class="icon-btn-sm flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground" title={t.email.forward} onclick={() => {
        const modal = getComposeModal?.();
        if (emailState.selectedEmail && modal) {
          modal.showForward(
            emailState.selectedEmail.subject || '',
            emailState.selectedEmail.body_text || ''
          );
        }
      }}>
        <Forward size={18} />
      </button>
      <button
        class="icon-btn-sm flex h-8 w-8 items-center justify-center rounded-md transition-colors hover:bg-glass-hover {emailState.selectedEmail.is_starred ? 'text-yellow-400' : 'text-muted-foreground hover:text-foreground'}"
        title={t.email.star}
        onclick={handleToggleStar}
      >
        <Star size={18} class={emailState.selectedEmail.is_starred ? 'fill-current' : ''} />
      </button>
      <button class="icon-btn-sm flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground" title={t.email.archive}>
        <Archive size={18} />
      </button>
      <button
        class="icon-btn-sm flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-destructive"
        title={t.common.delete}
        onclick={handleDelete}
      >
        <Trash2 size={18} />
      </button>
    </div>

    <!-- AI Actions bar -->
    <div class="flex items-center gap-3 border-t border-border px-5 py-2">
      <div class="flex items-center gap-1.5 text-xs text-muted-foreground">
        <Layers size={14} class="text-primary" />
        <span>AI {t.ai.provider}</span>
      </div>
      <div class="flex items-center gap-1">
        {#each aiActions as action}
          <button class="ai-action-btn flex items-center gap-1 rounded-md px-2.5 py-1 text-xs text-muted-foreground transition-colors hover:bg-primary/10 hover:text-primary">
            <span>{action.label}</span>
          </button>
        {/each}
      </div>
    </div>
  {:else}
    <!-- Empty state -->
    <div class="flex h-full flex-col items-center justify-center py-12">
      <Mail size={48} class="mb-4 opacity-30" strokeWidth={1} />
      <h3 class="text-lg text-foreground/70">{t.email.noEmailSelected}</h3>
    </div>
  {/if}
</div>
