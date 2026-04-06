<script lang="ts">
  import { onMount } from 'svelte';
  import { Editor } from '@tiptap/core';
  import StarterKit from '@tiptap/starter-kit';
  import Placeholder from '@tiptap/extension-placeholder';
  import { Bold, Italic, Strikethrough, List, ListOrdered, Quote } from 'lucide-svelte';

  let editorEl: HTMLElement;
  let editor: Editor;

  let htmlContent = $state('');

  onMount(() => {
    editor = new Editor({
      element: editorEl,
      extensions: [
        StarterKit,
        Placeholder.configure({
          placeholder: 'Write your message...',
        }),
      ],
      content: '',
      onUpdate: ({ editor: e }) => {
        htmlContent = e.getHTML();
      },
    });

    return () => {
      editor.destroy();
    };
  });

  export function getHtml(): string {
    return htmlContent;
  }

  export function getText(): string {
    return editor?.getText() || '';
  }

  export function setContent(html: string) {
    editor?.commands.setContent(html);
  }

  export function clear() {
    editor?.commands.clearContent();
  }
</script>

<div class="flex h-full flex-col">
  <!-- Toolbar -->
  <div class="flex items-center gap-0.5 border-b border-border px-2 py-1">
    <button
      class="flex h-7 w-7 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-glass-hover {editor?.isActive('bold') ? 'bg-glass-hover text-foreground' : ''}"
      onclick={() => editor?.chain().focus().toggleBold().run()}
      title="Bold"
    >
      <Bold size={14} />
    </button>
    <button
      class="flex h-7 w-7 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-glass-hover {editor?.isActive('italic') ? 'bg-glass-hover text-foreground' : ''}"
      onclick={() => editor?.chain().focus().toggleItalic().run()}
      title="Italic"
    >
      <Italic size={14} />
    </button>
    <button
      class="flex h-7 w-7 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-glass-hover {editor?.isActive('strike') ? 'bg-glass-hover text-foreground' : ''}"
      onclick={() => editor?.chain().focus().toggleStrike().run()}
      title="Strikethrough"
    >
      <Strikethrough size={14} />
    </button>
    <div class="mx-1 h-4 w-px bg-border"></div>
    <button
      class="flex h-7 w-7 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-glass-hover {editor?.isActive('bulletList') ? 'bg-glass-hover text-foreground' : ''}"
      onclick={() => editor?.chain().focus().toggleBulletList().run()}
      title="Bullet List"
    >
      <List size={14} />
    </button>
    <button
      class="flex h-7 w-7 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-glass-hover {editor?.isActive('orderedList') ? 'bg-glass-hover text-foreground' : ''}"
      onclick={() => editor?.chain().focus().toggleOrderedList().run()}
      title="Ordered List"
    >
      <ListOrdered size={14} />
    </button>
    <div class="mx-1 h-4 w-px bg-border"></div>
    <button
      class="flex h-7 w-7 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-glass-hover {editor?.isActive('blockquote') ? 'bg-glass-hover text-foreground' : ''}"
      onclick={() => editor?.chain().focus().toggleBlockquote().run()}
      title="Quote"
    >
      <Quote size={14} />
    </button>
  </div>

  <!-- Editor area -->
  <div class="flex-1 overflow-y-auto">
    <div bind:this={editorEl} class="prose prose-sm max-w-none p-4 dark:prose-invert"></div>
  </div>
</div>

<style>
  :global(.tiptap) {
    outline: none;
    min-height: 100%;
  }

  :global(.tiptap p.is-editor-empty:first-child::before) {
    content: attr(data-placeholder);
    float: left;
    color: var(--color-muted-foreground);
    pointer-events: none;
    height: 0;
  }
</style>
