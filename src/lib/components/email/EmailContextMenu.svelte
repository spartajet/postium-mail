<script lang="ts">
    import { getI18nState } from "$lib/stores/i18n.svelte";
    import {
        Reply,
        ReplyAll,
        Forward,
        Flag,
        Mail,
        Trash2,
        MoreHorizontal,
    } from "lucide-svelte";

    const i18n = getI18nState();
    const t = $derived(i18n.t);

    let {
        x,
        y,
        emailId,
        isRead,
        isStarred,
        onClose,
    }: {
        x: number;
        y: number;
        emailId: number;
        isRead: boolean;
        isStarred: boolean;
        onClose: () => void;
    } = $props();

    let menuEl: HTMLDivElement | undefined = $state();

    // 调整菜单位置，防止超出视口
    let menuStyle = $derived.by(() => {
        const menuWidth = 200;
        const menuHeight = 340;
        const vw = window.innerWidth;
        const vh = window.innerHeight;
        const adjustedX = x + menuWidth > vw ? Math.max(0, x - menuWidth) : x;
        const adjustedY = y + menuHeight > vh ? Math.max(0, y - menuHeight) : y;
        return `left: ${adjustedX}px; top: ${adjustedY}px;`;
    });

    function handleAction(_action: string) {
        // TODO: 实现各操作功能
        onClose();
    }

    function handleClickOutside(e: MouseEvent) {
        if (menuEl && !menuEl.contains(e.target as Node)) {
            onClose();
        }
    }
</script>

<svelte:window onclick={handleClickOutside} />

<div
    bind:this={menuEl}
    class="fixed z-[100] min-w-[180px] rounded-lg border border-border bg-card py-1 shadow-lg"
    style={menuStyle}
>
    <!-- 回复 -->
    <button
        class="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-sm transition-colors hover:bg-glass-hover"
        onclick={() => handleAction("reply")}
    >
        <Reply size={15} class="text-muted-foreground" />
        <span>{t.email.reply}</span>
    </button>

    <!-- 全部回复 -->
    <button
        class="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-sm transition-colors hover:bg-glass-hover"
        onclick={() => handleAction("replyAll")}
    >
        <ReplyAll size={15} class="text-muted-foreground" />
        <span>{t.email.replyAll}</span>
    </button>

    <!-- 转发 -->
    <button
        class="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-sm transition-colors hover:bg-glass-hover"
        onclick={() => handleAction("forward")}
    >
        <Forward size={15} class="text-muted-foreground" />
        <span>{t.email.forward}</span>
    </button>

    <!-- 分割线 -->
    <div class="my-1 border-t border-border"></div>

    <!-- 标为 Flag / 取消 Flag -->
    <button
        class="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-sm transition-colors hover:bg-glass-hover"
        onclick={() => handleAction("star")}
    >
        <Flag size={15} class={isStarred ? "text-yellow-500" : "text-muted-foreground"} />
        <span>{isStarred ? t.email.unstar : t.email.star}</span>
    </button>

    <!-- 标为已读 / 未读 -->
    <button
        class="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-sm transition-colors hover:bg-glass-hover"
        onclick={() => handleAction("toggleRead")}
    >
        <Mail size={15} class="text-muted-foreground" />
        <span>{isRead ? t.email.markUnread : t.email.markRead}</span>
    </button>

    <!-- 分割线 -->
    <div class="my-1 border-t border-border"></div>

    <!-- 删除 -->
    <button
        class="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-sm text-destructive transition-colors hover:bg-destructive/10"
        onclick={() => handleAction("delete")}
    >
        <Trash2 size={15} />
        <span>{t.email.delete}</span>
    </button>

    <!-- 分割线 -->
    <div class="my-1 border-t border-border"></div>

    <!-- 更多操作 -->
    <button
        class="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-sm transition-colors hover:bg-glass-hover"
        onclick={() => handleAction("more")}
    >
        <MoreHorizontal size={15} class="text-muted-foreground" />
        <span>{t.common.operations}</span>
    </button>
</div>
