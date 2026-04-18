<!--
  Postium Mail - 日历页面组件
  +page.svelte

  本组件是应用的日历视图页面，提供日历查看和事件管理功能。

  ==================== 功能说明 ====================
  1. 显示当前月份的日历网格
  2. 支持前后月份切换导航
  3. 高亮显示今天的日期
  4. 提供添加新事件的入口（占位符）
  5. 显示当月的所有事件列表（占位符）

  ==================== 组件状态 ====================
  - currentDate: 当前查看的日期，默认为今天
  - weekDays: 星期显示数组（Sun, Mon, Tue, Wed, Thu, Fri, Sat）
  - calendarDays: 计算得出的日历天数数组（包含空白填充）
  - monthLabel: 当前月份的标签（如 "2024年1月"）

  ==================== 交互说明 ====================
  - 点击左箭头：切换到上个月
  - 点击右箭头：切换到下个月
  - 点击日期：可以添加选中状态（当前未实现）
  - 点击"Add Event"按钮：打开添加事件模态框（当前未实现）

  ==================== 技术实现 ====================
  使用 Svelte 5 的 Runes API：
  - $state: 定义响应式状态
  - $derived: 定义派生状态（自动计算）
  - $derived.by: 定义复杂的派生状态

  日历计算逻辑：
  - getDaysInMonth(): 获取指定月份的总天数
  - getFirstDayOfMonth(): 获取指定月份的第一天是星期几
  - isToday(): 判断某个日期是否是今天
-->
<script lang="ts">
    // 导入国际化状态管理
    import { getI18nState } from "$lib/stores/i18n.svelte";
    // 导入图标组件
    import {
        Calendar as CalendarIcon,
        ChevronLeft,
        ChevronRight,
        Plus,
    } from "lucide-svelte";

    // 获取国际化状态实例
    const i18n = getI18nState();
    // 派生翻译函数，自动响应语言变化
    const t = $derived(i18n.t);

    // 当前查看的日期，默认为今天
    // 使用 $state 使其成为响应式状态
    let currentDate = $state(new Date());

    /**
     * 获取指定月份的总天数
     *
     * 通过创建下个月的第0天来获取当前月的最后一天。
     * JavaScript 的 Date 月份索引从 0 开始（0=1月，11=12月）。
     *
     * @param year - 年份（如 2024）
     * @param month - 月份索引（0-11，0=1月）
     * @returns 该月的总天数（28-31）
     *
     * 示例：
     * getDaysInMonth(2024, 1) // 返回 29（2024年2月有29天）
     * getDaysInMonth(2024, 3) // 返回 30（4月有30天）
     */
    function getDaysInMonth(year: number, month: number): number {
        // 创建下个月的第0天，即当前月的最后一天
        return new Date(year, month + 1, 0).getDate();
    }

    /**
     * 获取指定月份的第一天是星期几
     *
     * @param year - 年份
     * @param month - 月份索引（0-11）
     * @returns 星期几（0=周日，1=周一，...，6=周六）
     *
     * 示例：
     * getFirstDayOfMonth(2024, 0) // 返回 1（2024年1月1日是周一）
     */
    function getFirstDayOfMonth(year: number, month: number): number {
        return new Date(year, month, 1).getDay();
    }

    // 星期显示标签
    const weekDays = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

    /**
     * 计算日历网格显示的天数数组
     *
     * 使用 $derived.by 创建复杂的派生状态，当 currentDate 变化时自动重新计算。
     *
     * 数组包含：
     * - 前置空白（null）：填充月初空白格子
     * - 实际天数（1-31）：该月的所有日期
     *
     * 例如 2024年1月的数组：
     * [null, null, 1, 2, 3, ..., 31] （1月1日是周一，前面有2个空白）
     */
    const calendarDays = $derived.by(() => {
        const year = currentDate.getFullYear();
        const month = currentDate.getMonth();
        const daysInMonth = getDaysInMonth(year, month);
        const firstDay = getFirstDayOfMonth(year, month);

        // 初始化天数数组
        const days: (number | null)[] = [];

        // 在数组开头填充空白，使第一天对齐到正确的星期
        // 例如：第一天是周二（day=2），前面填充2个null
        for (let i = 0; i < firstDay; i++) days.push(null);

        // 填充该月的所有天数（1到该月最后一天）
        // 修正：原来的代码有bug，应该是 push(i) 而不是 push(null)
        for (let i = 1; i <= daysInMonth; i++) days.push(i);

        return days;
    });

    /**
     * 当前月份的显示标签
     * 使用本地化日期格式，如 "January 2024" 或 "2024年1月"
     */
    const monthLabel = $derived(
        currentDate.toLocaleDateString(undefined, {
            year: "numeric",
            month: "long",
        }),
    );

    /**
     * 切换到上个月
     *
     * 通过设置月份为当前月份减1来实现。
     * JavaScript 的 Date 会自动处理跨年情况。
     */
    function prevMonth() {
        currentDate = new Date(
            currentDate.getFullYear(),
            currentDate.getMonth() - 1,
            1,
        );
    }

    /**
     * 切换到下个月
     *
     * 通过设置月份为当前月份加1来实现。
     * JavaScript 的 Date 会自动处理跨年情况。
     */
    function nextMonth() {
        currentDate = new Date(
            currentDate.getFullYear(),
            currentDate.getMonth() + 1,
            1,
        );
    }

    /**
     * 判断某个日期是否是今天
     *
     * @param day - 要判断的日期数字（1-31）或 null（空白格子）
     * @returns 如果是今天返回 true，否则返回 false
     *
     * 比较逻辑：
     * 1. 如果 day 为 null，直接返回 false（空白格子不可能是今天）
     * 2. 比较日（date）
     * 3. 比较月（month）
     * 4. 比较年（year）
     * 只有三个条件都满足时，才是今天
     */
    function isToday(day: number | null): boolean {
        // 空白格子不可能是今天
        if (!day) return false;

        const today = new Date();

        // 比较年、月、日是否完全匹配
        return (
            day === today.getDate() &&
            currentDate.getMonth() === today.getMonth() &&
            currentDate.getFullYear() === today.getFullYear()
        );
    }
</script>

<div class="flex h-full flex-col overflow-hidden bg-background">
    <!-- ==================== 页面头部区域 ==================== -->
    <!-- 包含日历图标、标题和月份切换导航 -->
    <div
        class="flex items-center justify-between border-b border-border px-6 py-4"
    >
        <!-- 左侧：日历图标 + 标题 -->
        <div class="flex items-center gap-2">
            <CalendarIcon size={20} class="text-primary" />
            <h1 class="text-lg font-semibold text-foreground">
                {t.sidebar.calendar}
            </h1>
        </div>
        <!-- 右侧：月份导航控制 -->
        <div class="flex items-center gap-2">
            <!-- 上个月按钮 -->
            <button
                class="flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
                onclick={prevMonth}
            >
                <ChevronLeft size={18} />
            </button>
            <!-- 当前月份标签（如 "2024年1月"） -->
            <span
                class="min-w-[140px] text-center text-sm font-medium text-foreground"
                >{monthLabel}</span
            >
            <!-- 下个月按钮 -->
            <button
                class="flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
                onclick={nextMonth}
            >
                <ChevronRight size={18} />
            </button>
        </div>
    </div>

    <!-- ==================== 日历网格区域 ==================== -->
    <!-- 使用 7 列网格布局，包含星期标题行和日期格子 -->
    <div class="flex-1 p-6">
        <div
            class="grid grid-cols-7 gap-px rounded-lg border border-border bg-border overflow-hidden"
        >
            <!-- 星期标题行：Sun, Mon, Tue, Wed, Thu, Fri, Sat -->
            {#each weekDays as day}
                <div
                    class="bg-glass px-2 py-2 text-center text-xs font-medium text-muted-foreground"
                >
                    {day}
                </div>
            {/each}
            <!-- 日期格子：包含前置空白和实际日期 -->
            <!-- 样式逻辑：
                 - 今天的日期：加粗 + 主色（font-bold text-primary）
                 - 有效日期：前景色（text-foreground）
                 - 空白格子：透明（text-transparent），占位用
            -->
            {#each calendarDays as day, i ("d" + i)}
                <div
                    class="bg-background px-2 py-3 text-center text-sm transition-colors hover:bg-glass-hover {isToday(
                        day,
                    )
                        ? 'font-bold text-primary'
                        : day
                          ? 'text-foreground'
                          : 'text-transparent'}"
                >
                    {day || ""}
                </div>
            {/each}
        </div>

        <!-- ==================== 事件列表区域（占位符） ==================== -->
        <!-- 未来将显示当月的事件列表，当前为空状态提示 -->
        <div class="mt-6">
            <!-- 区域标题 + 添加事件按钮 -->
            <div class="flex items-center justify-between mb-3">
                <h3 class="text-sm font-semibold text-foreground">Events</h3>
                <!-- 添加新事件按钮（功能待实现） -->
                <button
                    class="flex items-center gap-1 rounded-md px-2 py-1 text-xs text-primary transition-colors hover:bg-primary/10"
                >
                    <Plus size={14} />
                    Add Event
                </button>
            </div>
            <!-- 空状态提示：当月无事件 -->
            <div
                class="rounded-lg border border-dashed border-border px-4 py-8 text-center text-sm text-muted-foreground"
            >
                No events for this month
            </div>
        </div>
    </div>
</div>
