<!--
  Postium Mail - 富文本编辑器组件
  RichTextEditor.svelte

  本组件是基于 Tiptap 的富文本编辑器，用于邮件正文编辑。

  ==================== 功能说明 ====================
  1. 提供富文本编辑功能：加粗、斜体、删除线
  2. 提供列表功能：无序列表、有序列表
  3. 提供引用块功能
  4. 提供占位符提示文本
  5. 暴露公开方法供父组件调用：获取 HTML、获取纯文本、设置内容、清空内容

  ==================== 在架构中的位置 ====================
  位于 src/lib/components/email/ 目录下，是邮件功能模块的子组件。
  被引用于：
    - ComposeModal.svelte（写邮件模态框中的正文编辑区域）

  依赖的第三方库：
    - @tiptap/core：Tiptap 编辑器核心库（基于 ProseMirror）
    - @tiptap/starter-kit：Tiptap 入门套件（包含常用扩展：加粗、斜体、列表等）
    - @tiptap/extension-placeholder：占位符扩展（空内容时显示提示文字）
    - lucide-svelte：工具栏图标

  ==================== 技术说明 ====================
  Tiptap 是一个基于 ProseMirror 的富文本编辑器框架。
  - Editor 实例管理编辑器状态和内容
  - extensions 数组定义可用功能（加粗、斜体、列表等）
  - onUpdate 回调在内容变化时同步 HTML 到组件状态
  - 编辑器通过 bind:this 挂载到指定 DOM 元素

  ==================== 使用方式 ====================
  在父组件中通过 bind:this 获取编辑器实例引用：
  ```
  let richEditor = $state<RichTextEditor>();
  <RichTextEditor bind:this={richEditor} />

  // 获取内容
  richEditor?.getHtml()   // 获取 HTML 格式正文
  richEditor?.getText()   // 获取纯文本正文

  // 设置内容
  richEditor?.setContent('<p>Hello</p>')

  // 清空内容
  richEditor?.clear()
  ```
-->

<script lang="ts">
    // ==================== 导入依赖 ====================

    // 导入 Svelte 生命周期钩子，onMount 在组件挂载到 DOM 后执行
    import { onMount } from "svelte";
    // 导入 Tiptap 编辑器核心类
    // Editor 是 Tiptap 的主入口，负责管理编辑器实例、状态和扩展
    import { Editor } from "@tiptap/core";
    // 导入 Tiptap 入门套件
    // StarterKit 包含一组常用的编辑功能扩展：
    //   - Bold（加粗）、Italic（斜体）、Strike（删除线）
    //   - BulletList（无序列表）、OrderedList（有序列表）、ListItem（列表项）
    //   - Blockquote（引用块）、CodeBlock（代码块）
    //   - Heading（标题）、HorizontalRule（水平线）、Paragraph（段落）
    //   - Document（文档结构）、Text（文本节点）、Dropcursor（拖放光标）
    //   - Gapcursor（间隙光标）、History（撤销/重做）
    import StarterKit from "@tiptap/starter-kit";
    // 导入占位符扩展
    // 在编辑器内容为空时显示提示文字（如 "Write your message..."）
    import Placeholder from "@tiptap/extension-placeholder";
    // 导入工具栏图标组件（Lucide 图标库）
    import {
        Bold, // 加粗图标（B）
        Italic, // 斜体图标（I）
        Strikethrough, // 删除线图标（S 带横线）
        List, // 无序列表图标
        ListOrdered, // 有序列表图标
        Quote, // 引用块图标
    } from "lucide-svelte";

    // ==================== 编辑器状态 ====================

    // 编辑器挂载的 DOM 元素引用
    // 通过 bind:this 绑定到模板中的 <div> 元素
    // Tiptap 将在这个元素内创建编辑器实例
    let editorEl: HTMLElement;

    // Tiptap 编辑器实例
    // 在 onMount 中初始化，组件卸载时销毁
    let editor: Editor;

    // 编辑器的 HTML 内容（响应式状态）
    // 每次编辑器内容变化时自动更新
    // 供 getHtml() 方法返回给父组件
    let htmlContent = $state("");

    // ==================== 编辑器初始化 ====================

    /**
     * 组件挂载生命周期钩子
     *
     * 在组件挂载到 DOM 后初始化 Tiptap 编辑器实例。
     * 配置包括：
     * - element：挂载目标 DOM 元素
     * - extensions：启用的功能扩展列表
     * - content：初始内容（空字符串）
     * - onUpdate：内容变化时的回调函数
     *
     * 返回清理函数：组件卸载时销毁编辑器实例，防止内存泄漏
     */
    onMount(() => {
        // 创建 Tiptap 编辑器实例
        editor = new Editor({
            // 指定编辑器挂载的 DOM 元素
            // Tiptap 会在这个元素内部创建 ProseMirror 编辑器
            element: editorEl,

            // 启用的扩展列表
            extensions: [
                // 入门套件：包含加粗、斜体、列表、引用等常用功能
                StarterKit,
                // 占位符扩展：在编辑器为空时显示提示文字
                Placeholder.configure({
                    // 占位符文本（英文，因为 Tiptap 的占位符不支持直接国际化）
                    // TODO: 考虑通过 props 传入国际化的占位符文本
                    placeholder: "Write your message...",
                }),
            ],

            // 编辑器初始内容（空字符串）
            content: "",

            /**
             * 内容更新回调
             *
             * 每次编辑器内容发生变化时触发。
             * 将最新的 HTML 内容同步到 htmlContent 响应式状态。
             *
             * @param editor - 包含当前编辑器实例的对象
             */
            onUpdate: ({ editor: e }) => {
                // 获取编辑器的 HTML 内容并更新状态
                htmlContent = e.getHTML();
            },
        });

        // 返回清理函数：组件卸载时调用
        // 销毁编辑器实例，释放 ProseMirror 相关资源
        return () => {
            editor.destroy();
        };
    });

    // ==================== 公开方法（供父组件调用） ====================

    /**
     * 获取编辑器内容的 HTML 格式
     *
     * 返回当前编辑器内容的 HTML 字符串。
     * 用于发送邮件时获取富文本正文。
     *
     * @returns HTML 格式的编辑器内容
     */
    export function getHtml(): string {
        return htmlContent;
    }

    /**
     * 获取编辑器内容的纯文本格式
     *
     * 返回当前编辑器内容的纯文本字符串（去除所有 HTML 标签）。
     * 用于发送邮件时获取纯文本备用正文。
     *
     * @returns 纯文本格式的编辑器内容，编辑器未初始化时返回空字符串
     */
    export function getText(): string {
        return editor?.getText() || "";
    }

    /**
     * 设置编辑器的内容
     *
     * 将指定的 HTML 内容设置到编辑器中，替换当前内容。
     * 用于回复/转发邮件时预填充原始邮件内容。
     *
     * @param html - 要设置的 HTML 内容字符串
     */
    export function setContent(html: string) {
        editor?.commands.setContent(html);
    }

    /**
     * 清空编辑器内容
     *
     * 移除编辑器中的所有内容，恢复到空白状态。
     * 用于关闭模态框时重置编辑器。
     */
    export function clear() {
        editor?.commands.clearContent();
    }
</script>

<!-- ==================== 模板渲染 ==================== -->

<!--
  编辑器容器
  使用垂直弹性布局，工具栏在上，编辑区域在下
  - flex / flex-col：垂直弹性布局
  - h-full：占满父容器高度
-->
<div class="flex h-full flex-col">
    <!-- ==================== 工具栏 ==================== -->

    <!--
      富文本格式工具栏
      包含格式化按钮和分隔线
      - flex items-center gap-0.5：水平排列，居中对齐，紧凑间距
      - border-b border-border：底部边框分隔线
      - px-2 py-1：紧凑内边距
    -->
    <div class="flex items-center gap-0.5 border-b border-border px-2 py-1">
        <!--
          加粗按钮
          - editor?.isActive('bold') 检查当前光标位置是否在加粗文本中
          - 如果是，添加 bg-glass-hover text-foreground 高亮样式
          - editor?.chain().focus().toggleBold().run() 执行加粗切换操作
            - chain()：链式调用
            - focus()：保持编辑器聚焦
            - toggleBold()：切换加粗状态
            - run()：执行操作
        -->
        <button
            class="flex h-7 w-7 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-glass-hover {editor?.isActive(
                'bold',
            )
                ? 'bg-glass-hover text-foreground'
                : ''}"
            onclick={() => editor?.chain().focus().toggleBold().run()}
            title="Bold"
        >
            <Bold size={14} />
        </button>

        <!--
          斜体按钮
          - isActive('italic') 检查斜体状态
          - toggleItalic() 切换斜体
        -->
        <button
            class="flex h-7 w-7 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-glass-hover {editor?.isActive(
                'italic',
            )
                ? 'bg-glass-hover text-foreground'
                : ''}"
            onclick={() => editor?.chain().focus().toggleItalic().run()}
            title="Italic"
        >
            <Italic size={14} />
        </button>

        <!--
          删除线按钮
          - isActive('strike') 检查删除线状态
          - toggleStrike() 切换删除线
        -->
        <button
            class="flex h-7 w-7 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-glass-hover {editor?.isActive(
                'strike',
            )
                ? 'bg-glass-hover text-foreground'
                : ''}"
            onclick={() => editor?.chain().focus().toggleStrike().run()}
            title="Strikethrough"
        >
            <Strikethrough size={14} />
        </button>

        <!--
          分隔线
          将文本格式按钮与列表按钮视觉分组
          - mx-1：水平外边距
          - h-4 w-px：高度 1rem，宽度 1px
          - bg-border：边框颜色
        -->
        <div class="mx-1 h-4 w-px bg-border"></div>

        <!--
          无序列表按钮
          - isActive('bulletList') 检查是否在无序列表中
          - toggleBulletList() 切换无序列表
        -->
        <button
            class="flex h-7 w-7 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-glass-hover {editor?.isActive(
                'bulletList',
            )
                ? 'bg-glass-hover text-foreground'
                : ''}"
            onclick={() => editor?.chain().focus().toggleBulletList().run()}
            title="Bullet List"
        >
            <List size={14} />
        </button>

        <!--
          有序列表按钮
          - isActive('orderedList') 检查是否在有序列表中
          - toggleOrderedList() 切换有序列表
        -->
        <button
            class="flex h-7 w-7 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-glass-hover {editor?.isActive(
                'orderedList',
            )
                ? 'bg-glass-hover text-foreground'
                : ''}"
            onclick={() => editor?.chain().focus().toggleOrderedList().run()}
            title="Ordered List"
        >
            <ListOrdered size={14} />
        </button>

        <!-- 分隔线：将列表按钮与引用按钮视觉分组 -->
        <div class="mx-1 h-4 w-px bg-border"></div>

        <!--
          引用块按钮
          - isActive('blockquote') 检查是否在引用块中
          - toggleBlockquote() 切换引用块
        -->
        <button
            class="flex h-7 w-7 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-glass-hover {editor?.isActive(
                'blockquote',
            )
                ? 'bg-glass-hover text-foreground'
                : ''}"
            onclick={() => editor?.chain().focus().toggleBlockquote().run()}
            title="Quote"
        >
            <Quote size={14} />
        </button>
    </div>

    <!-- ==================== 编辑器区域 ==================== -->

    <!--
      编辑器可滚动区域
      - flex-1：占据剩余空间
      - overflow-y-auto：垂直方向超出时滚动
    -->
    <div class="flex-1 overflow-y-auto">
        <!--
          Tiptap 编辑器挂载点
          - bind:this={editorEl}：将 DOM 元素绑定到变量，供 Tiptap 挂载
          - prose prose-sm：Tailwind Typography 插件样式，美化排版
          - max-w-none：取消最大宽度限制
          - p-4：内边距
          - dark:prose-invert：暗色模式下反转文字颜色

          Tiptap 会在初始化时将这个 <div> 替换为 ProseMirror 编辑器 DOM 结构
        -->
        <div
            bind:this={editorEl}
            class="prose prose-sm max-w-none p-4 dark:prose-invert"
        ></div>
    </div>
</div>
