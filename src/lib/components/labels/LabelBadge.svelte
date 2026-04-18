<!--
  Postium Mail - 标签徽章组件
  LabelBadge.svelte

  本组件用于在邮件列表、侧边栏等位置显示彩色标签徽章。

  ==================== 功能说明 ====================
  1. 根据标签颜色渲染带圆点的彩色徽章
  2. 支持可选的删除按钮（用于标签管理场景）
  3. 颜色通过半透明背景和实色文字实现视觉一致性

  ==================== 组件属性 ====================
  - label: 标签对象，包含 name（名称）和 color（颜色值，如 "#ff0000"）
  - removable: 是否显示删除按钮，默认 false
  - onremove: 删除按钮的回调函数（可选）

  ==================== 视觉设计 ====================
  - 背景色：标签颜色的 20% 透明度（{color}20）
  - 文字色：标签颜色的 100% 不透明
  - 左侧圆点：标签颜色的 100% 不透明
  - 删除按钮：hover 时降低透明度

  ==================== 使用场景 ====================
  - 邮件列表中显示邮件所属标签
  - 标签管理页面中显示可删除的标签
  - 撰写邮件时显示已选标签

  ==================== 技术实现 ====================
  使用 Svelte 5 的 Runes API：
  - $props(): 定义组件属性（替代传统的 export let）
-->
<script lang="ts">
    // 导入 X（关闭/删除）图标，用于可删除标签的移除按钮
    import { X } from "lucide-svelte";

    // 使用 Svelte 5 的 $props() 语法定义组件属性
    let {
        label, // 标签对象，包含 name（标签名称）和 color（标签颜色，十六进制格式）
        removable = false, // 是否显示删除按钮，默认不显示
        onremove, // 删除按钮点击时的回调函数（可选）
    }: {
        label: { name: string; color: string }; // 标签类型定义：名称 + 颜色
        removable?: boolean; // 是否可删除（可选）
        onremove?: () => void; // 删除回调（可选）
    } = $props();
</script>

<!-- 标签徽章容器：圆角胶囊形状，内联弹性布局 -->
<!-- 背景色使用标签颜色的 20% 透明度（{color}20），文字色使用标签颜色 -->
<span
    class="inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium"
    style="background-color: {label.color}20; color: {label.color}"
>
    <!-- 标签颜色指示圆点：使用标签颜色填充，视觉上标识标签颜色 -->
    <span
        class="h-1.5 w-1.5 rounded-full"
        style="background-color: {label.color}"
    ></span>

    <!-- 标签名称文本 -->
    {label.name}

    <!-- 可选的删除按钮：仅在 removable 为 true 时显示 -->
    {#if removable}
        <!-- 删除按钮：点击时触发 onremove 回调，hover 时降低透明度 -->
        <button class="ml-0.5 hover:opacity-70" onclick={onremove}>
            <X size={12} style="color: {label.color}" />
        </button>
    {/if}
</span>
