<script setup lang="ts">
import { useEditor, EditorContent } from '@tiptap/vue-3'
import StarterKit from '@tiptap/starter-kit'
import Underline from '@tiptap/extension-underline'
import TextAlign from '@tiptap/extension-text-align'
import Link from '@tiptap/extension-link'
import Image from '@tiptap/extension-image'
import Text from '@tiptap/extension-text'
import { watch } from 'vue'

const props = defineProps<{
  modelValue: string
  placeholder?: string
  editable?: boolean
  minHeight?: string
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
}>()

const editor = useEditor({
  content: props.modelValue,
  extensions: [
    StarterKit,
    Underline,
    TextAlign.configure({
      types: ['left', 'center', 'right', 'justify'],
    }),
    Link.configure({
      openOnClick: false,
    }),
    Image.configure({
        inline: true,
        allowBase64: true,
    }),
    Text,
  ],
  editorProps: {
    attributes: {
      class: 'prose prose-sm sm:prose lg:prose-lg mx-auto focus:outline-none',
    },
  },
  onUpdate: () => {
    if (editor.value) {
      emit('update:modelValue', editor.value.getHTML())
    }
  },
})

// 监听外部值变化
watch(() => props.modelValue, (newValue) => {
  if (editor.value && editor.value.getHTML() !== newValue) {
    editor.value.commands.setContent(newValue)
  }
})

// 工具栏操作
const setLink = () => {
  const url = window.prompt('输入链接地址:')
  if (url) {
    editor.value?.chain().focus().setLink({ href: url }).run()
  }
}

const addImage = () => {
  const url = window.prompt('输入图片地址:')
  if (url) {
    editor.value?.chain().focus().setImage({ src: url }).run()
  }
}

// 暴露方法给父组件
defineExpose({
  insertText: (text: string) => {
    editor.value?.chain().focus().insertContent(text).run()
  },
  getHTML: () => {
    return editor.value?.getHTML() || ''
  },
  getText: () => {
    return editor.value?.getText() || ''
  },
  clear: () => {
    editor.value?.commands.clearContent(true)
    emit('update:modelValue', '')
  }
})
</script>

<template>
  <div class="rich-text-editor" :class="{ 'is-disabled': !editable }">
    <!-- 工具栏 -->
    <div v-if="editor" class="editor-toolbar">
      <button
        @click="editor.chain().focus().toggleBold().run()"
        :class="{ 'is-active': editor.isActive('bold') }"
        title="粗体"
        :disabled="!editable"
      >
        <span class="icon">B</span>
      </button>

      <button
        @click="editor.chain().focus().toggleItalic().run()"
        :class="{ 'is-active': editor.isActive('italic') }"
        title="斜体"
        :disabled="!editable"
      >
        <span class="icon">I</span>
      </button>

      <button
        @click="editor.chain().focus().toggleUnderline().run()"
        :class="{ 'is-active': editor.isActive('underline') }"
        title="下划线"
        :disabled="!editable"
      >
        <span class="icon">U</span>
      </button>

      <div class="toolbar-divider"></div>

      <button
        @click="editor.chain().focus().toggleStrike().run()"
        :class="{ 'is-active': editor.isActive('strike') }"
        title="删除线"
        :disabled="!editable"
      >
        <span class="icon">S</span>
      </button>

      <div class="toolbar-divider"></div>

      <button
        @click="editor.chain().focus().toggleHeading({ level: 1 }).run()"
        :class="{ 'is-active': editor.isActive('heading', { level: 1 }) }"
        title="一级标题"
        :disabled="!editable"
      >
        <span class="icon">H1</span>
      </button>

      <button
        @click="editor.chain().focus().toggleHeading({ level: 2 }).run()"
        :class="{ 'is-active': editor.isActive('heading', { level: 2 }) }"
        title="二级标题"
        :disabled="!editable"
      >
        <span class="icon">H2</span>
      </button>

      <button
        @click="editor.chain().focus().toggleHeading({ level: 3 }).run()"
        :class="{ 'is-active': editor.isActive('heading', { level: 3 }) }"
        title="三级标题"
        :disabled="!editable"
      >
        <span class="icon">H3</span>
      </button>

      <div class="toolbar-divider"></div>

      <button
        @click="editor.chain().focus().toggleBulletList().run()"
        :class="{ 'is-active': editor.isActive('bulletList') }"
        title="无序列表"
        :disabled="!editable"
      >
        <span class="icon">•</span>
      </button>

      <button
        @click="editor.chain().focus().toggleOrderedList().run()"
        :class="{ 'is-active': editor.isActive('orderedList') }"
        title="有序列表"
        :disabled="!editable"
      >
        <span class="icon">1.</span>
      </button>

      <div class="toolbar-divider"></div>

      <button
        @click="editor.chain().focus().setTextAlign('left').run()"
        :class="{ 'is-active': editor.isActive({ textAlign: 'left' }) }"
        title="左对齐"
        :disabled="!editable"
      >
        <span class="icon">⇤</span>
      </button>

      <button
        @click="editor.chain().focus().setTextAlign('center').run()"
        :class="{ 'is-active': editor.isActive({ textAlign: 'center' }) }"
        title="居中"
        :disabled="!editable"
      >
        <span class="icon">⇔⇥</span>
      </button>

      <button
        @click="editor.chain().focus().setTextAlign('right').run()"
        :class="{ 'is-active': editor.isActive({ textAlign: 'right' }) }"
        title="右对齐"
        :disabled="!editable"
      >
        <span class="icon">⥤</span>
      </button>

      <div class="toolbar-divider"></div>

      <button @click="setLink" title="插入链接" :disabled="!editable">
        <span class="icon">🔗</span>
      </button>

      <button @click="addImage" title="插入图片" :disabled="!editable">
        <span class="icon">🖼️</span>
      </button>

      <div class="toolbar-divider"></div>

      <button
        @click="editor.chain().focus().undo().run()"
        title="撤销"
        :disabled="!editable || !editor.can().undo()"
      >
        <span class="icon">↶</span>
      </button>

      <button
        @click="editor.chain().focus().redo().run()"
        title="重做"
        :disabled="!editable || !editor.can().redo()"
      >
        <span class="icon">↷</span>
      </button>
    </div>

    <!-- 编辑器内容 -->
    <EditorContent
      :editor="editor"
      class="editor-content"
      :placeholder="placeholder"
      :editable="editable"
    />
  </div>
</template>

<style scoped>
.rich-text-editor {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  overflow: hidden;
  background: var(--bg-glass);
}

.rich-text-editor.is-disabled {
  opacity: 0.6;
  pointer-events: none;
}

.editor-toolbar {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  padding: 8px;
  background: var(--bg-glass-hover);
  border-bottom: 1px solid var(--border-subtle);
}

.editor-toolbar button {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border: none;
  background: transparent;
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  cursor: pointer;
  transition: all 0.15s ease;
}

.editor-toolbar button:hover:not(:disabled) {
  background: var(--bg-glass-active);
  color: var(--text-primary);
}

.editor-toolbar button:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.editor-toolbar button.is-active {
  background: var(--primary-light);
  color: var(--primary);
}

.editor-toolbar .icon {
  font-size: 14px;
  font-weight: 600;
}

.toolbar-divider {
  width: 1px;
  height: 24px;
  background: var(--border-subtle);
  margin: 0 4px;
}

.editor-content {
  min-height: v-bind('minHeight');
  max-height: 500px;
  overflow-y: auto;
  padding: 12px;
}

.editor-content :deep(.ProseMirror) {
  outline: none;
}

.editor-content :deep(.ProseMirror p.is-editor-empty:first-child::before) {
  content: attr(placeholder);
  float: left;
  color: var(--text-muted);
  pointer-events: none;
  height: 0;
}

.editor-content :deep(.ProseMirror) {
  color: var(--text-primary);
}

.editor-content :deep(.ProseMirror p) {
  margin: 0.5em 0;
}

.editor-content :deep(.ProseMirror h1),
.editor-content :deep(.ProseMirror h2),
.editor-content :deep(.ProseMirror h3) {
  margin: 0.75em 0 0.25em;
  font-weight: 600;
}

.editor-content :deep(.ProseMirror ul),
.editor-content :deep(.ProseMirror ol) {
  padding-left: 1.5em;
  margin: 0.5em 0;
}

.editor-content :deep(.ProseMirror a) {
  color: var(--primary);
  text-decoration: underline;
}

.editor-content :deep(.ProseMirror img) {
  max-width: 100%;
  height: auto;
  border-radius: var(--radius-md);
}

/* Tiptap ProseMirror 样式 */
:deep(.ProseMirror) {
  outline: none;
}

:deep(.ProseMirror p.is-editor-empty:first-child::before) {
  color: var(--text-muted);
  content: attr(data-placeholder);
  float: left;
  height: 0;
  pointer-events: none;
}
</style>
