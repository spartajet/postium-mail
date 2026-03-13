<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { useCalendarStore } from "@/stores/calendar";
import { useUIStore } from "@/stores/ui";
import { ChevronLeftOutlined, ChevronRightOutlined, AddOutlined } from "@vicons/material";
import CalendarGrid from "./CalendarGrid.vue";
import EventModal from "./EventModal.vue";

// Stores
const calendarStore = useCalendarStore();
const uiStore = useUIStore();

// 当前月份标题
const monthTitle = computed(() => {
    const year = calendarStore.currentDate.getFullYear();
    const month = calendarStore.currentDate.getMonth() + 1;
    return `${year}年${month}月`;
});

// 星期标题
const weekDays = ["日", "一", "二", "三", "四", "五", "六"];

// 上一个月
function handlePrevMonth() {
    calendarStore.previousMonth();
}

// 下一个月
function handleNextMonth() {
    calendarStore.nextMonth();
}

// 回到今天
function handleToday() {
    calendarStore.goToToday();
}

// 打开新建事件模态框
function handleNewEvent() {
    calendarStore.startCreateEvent();
    showEventModal.value = true;
}

// 选择日期
function handleSelectDate(date: Date) {
    calendarStore.selectDate(date);
    calendarStore.startCreateEvent(date);
    showEventModal.value = true;
}

// 编辑事件
function handleEditEvent(eventId: string) {
    calendarStore.startEditEvent(eventId);
    showEventModal.value = true;
}

// 关闭事件模态框
function handleCloseModal() {
    showEventModal.value = false;
    calendarStore.cancelEdit();
}

// 保存事件
async function handleSaveEvent(eventData: any) {
    await calendarStore.saveEvent(eventData);
    showEventModal.value = false;
    uiStore.showSuccess("事件已保存");
}

// 删除事件
async function handleDeleteEvent(eventId: string) {
    await calendarStore.deleteEvent(eventId);
    showEventModal.value = false;
    uiStore.showSuccess("事件已删除");
}

// 事件模态框显示状态
const showEventModal = ref(false);

// 初始化
onMounted(async () => {
    await calendarStore.fetchEvents();
});
</script>

<template>
    <div class="calendar-full-view">
        <div class="calendar-view">
            <!-- 日历头部 -->
            <div class="calendar-header">
                <div class="calendar-title">
                    <h2>{{ monthTitle }}</h2>
                    <div class="calendar-nav">
                        <button class="icon-btn" @click="handlePrevMonth" title="上个月">
                            <ChevronLeftOutlined :size="20" />
                        </button>
                        <button class="icon-btn" @click="handleNextMonth" title="下个月">
                            <ChevronRightOutlined :size="20" />
                        </button>
                        <button class="btn btn-ghost" @click="handleToday">
                            今天
                        </button>
                    </div>
                </div>
                <button class="btn btn-primary" @click="handleNewEvent">
                    <AddOutlined :size="16" />
                    <span>新建事务</span>
                </button>
            </div>

            <!-- 星期标题 -->
            <div class="calendar-grid-header">
                <div
                    v-for="day in weekDays"
                    :key="day"
                    class="calendar-day-name"
                >
                    {{ day }}
                </div>
            </div>

            <!-- 日历网格 -->
            <CalendarGrid
                :days="calendarStore.calendarGrid"
                @select-date="handleSelectDate"
                @edit-event="handleEditEvent"
            />
        </div>

        <!-- 事件模态框 -->
        <EventModal
            v-if="showEventModal"
            :event="calendarStore.editingEvent"
            :selected-date="calendarStore.selectedDate"
            @close="handleCloseModal"
            @save="handleSaveEvent"
            @delete="handleDeleteEvent"
        />
    </div>
</template>

<style scoped>
.calendar-full-view {
    flex: 1;
    display: flex;
    flex-direction: column;
    background: var(--bg-base);
    overflow: hidden;
}

.calendar-view {
    flex: 1;
    display: flex;
    flex-direction: column;
    padding: 24px;
    overflow: hidden;
}

.calendar-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 24px;
}

.calendar-title {
    display: flex;
    align-items: center;
    gap: 16px;
}

.calendar-title h2 {
    font-size: 20px;
    font-weight: 600;
    color: var(--text-primary);
    margin: 0;
}

.calendar-nav {
    display: flex;
    align-items: center;
    gap: 8px;
}

.calendar-grid-header {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 1px;
    background: var(--border-subtle);
    border: 1px solid var(--border-subtle);
    border-radius: 8px 8px 0 0;
    overflow: hidden;
}

.calendar-day-name {
    padding: 8px;
    text-align: center;
    font-size: 14px;
    font-weight: 500;
    color: var(--text-muted);
    background: var(--bg-elevated);
}

.btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 8px 16px;
    border-radius: 8px;
    font-size: 14px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
    border: none;
}

.btn-primary {
    background: linear-gradient(135deg, var(--primary) 0%, var(--secondary) 100%);
    color: white;
    box-shadow: 0 4px 16px rgba(124, 58, 237, 0.3);
}

.btn-primary:hover {
    transform: translateY(-1px);
    box-shadow: 0 6px 20px rgba(124, 58, 237, 0.4);
}

.btn-ghost {
    background: transparent;
    color: var(--text-secondary);
}

.btn-ghost:hover {
    background: var(--bg-glass-hover);
    color: var(--text-primary);
}

.icon-btn {
    width: 36px;
    height: 36px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 8px;
    color: var(--text-secondary);
    cursor: pointer;
    transition: all 0.15s ease;
    border: none;
    background: transparent;
}

.icon-btn:hover {
    background: var(--bg-glass-hover);
    color: var(--text-primary);
}

@media (max-width: 768px) {
    .calendar-view {
        padding: 16px;
    }

    .calendar-header {
        flex-direction: column;
        gap: 16px;
        align-items: flex-start;
    }

    .calendar-title {
        width: 100%;
        justify-content: space-between;
    }
}
</style>
