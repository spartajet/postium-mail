<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { CalendarEvent } from "@/types";

// i18n
const { t } = useI18n();

// Props
interface Props {
    days: Array<{
        date: Date;
        isCurrentMonth: boolean;
        isToday: boolean;
        events: CalendarEvent[];
    }>;
}

const props = defineProps<Props>();

// Emits
const emit = defineEmits<{
    "select-date": [date: Date];
    "edit-event": [eventId: string];
}>();

// 选择日期
function handleDateClick(date: Date) {
    emit("select-date", date);
}

// 编辑事件
function handleEventClick(event: CalendarEvent, e: Event) {
    e.stopPropagation();
    emit("edit-event", event.id);
}

// 格式化日期
function formatDay(date: Date): number {
    return date.getDate();
}

// 检查是否是今天
function isToday(date: Date): boolean {
    const today = new Date();
    return (
        date.getFullYear() === today.getFullYear() &&
        date.getMonth() === today.getMonth() &&
        date.getDate() === today.getDate()
    );
}
</script>

<template>
    <div class="calendar-grid">
        <div
            v-for="(day, index) in days"
            :key="index"
            :class="[
                'calendar-day',
                {
                    'other-month': !day.isCurrentMonth,
                    today: day.isToday,
                },
            ]"
            @click="handleDateClick(day.date)"
        >
            <div class="calendar-day-number">
                {{ formatDay(day.date) }}
            </div>
            <div v-if="day.events.length > 0" class="calendar-events">
                <div
                    v-for="event in day.events.slice(0, 3)"
                    :key="event.id"
                    class="calendar-event"
                    :style="{ backgroundColor: event.color }"
                    @click="handleEventClick(event, $event)"
                >
                    <span class="event-time">{{ event.startTime }}</span>
                    <span class="event-title">{{ event.title }}</span>
                </div>
                <div
                    v-if="day.events.length > 3"
                    class="more-events"
                >
                    +{{ day.events.length - 3 }} {{ t('calendar.moreEvents') }}
                </div>
            </div>
        </div>
    </div>
</template>

<style scoped>
.calendar-grid {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 1px;
    background: var(--border-subtle);
    border: 1px solid var(--border-subtle);
    border-top: none;
    border-radius: 0 0 8px 8px;
    overflow: hidden;
    flex: 1;
    min-height: 400px;
}

.calendar-day {
    min-height: 100px;
    padding: 8px;
    background: var(--bg-elevated);
    cursor: pointer;
    transition: background 0.15s ease;
    display: flex;
    flex-direction: column;
}

.calendar-day:hover {
    background: var(--bg-glass-hover);
}

.calendar-day.other-month {
    background: var(--bg-glass);
}

.calendar-day.other-month .calendar-day-number {
    color: var(--text-muted);
}

.calendar-day.today .calendar-day-number {
    background: var(--primary);
    color: white;
    border-radius: 50%;
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-weight: 600;
}

.calendar-day-number {
    font-size: 14px;
    font-weight: 500;
    color: var(--text-primary);
    margin-bottom: 4px;
}

.calendar-events {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    overflow: hidden;
}

.calendar-event {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 2px 6px;
    border-radius: 4px;
    font-size: 11px;
    color: white;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: pointer;
    transition: opacity 0.15s ease;
}

.calendar-event:hover {
    opacity: 0.9;
}

.event-time {
    font-weight: 500;
    opacity: 0.9;
}

.event-title {
    overflow: hidden;
    text-overflow: ellipsis;
}

.more-events {
    font-size: 11px;
    color: var(--text-muted);
    padding: 2px 6px;
}

@media (max-width: 768px) {
    .calendar-day {
        min-height: 80px;
        padding: 4px;
    }

    .calendar-event {
        font-size: 10px;
        padding: 1px 4px;
    }

    .event-time {
        display: none;
    }
}
</style>
