import { defineStore } from "pinia";
import { ref, computed } from "vue";
import type { CalendarEvent } from "@/types";
import { generateCalendarEvents, delay } from "@/mocks";

export const useCalendarStore = defineStore("calendar", () => {
  // ========================================
  // State
  // ========================================

  // 日历事件列表
  const events = ref<CalendarEvent[]>([]);

  // 当前显示的月份
  const currentDate = ref(new Date());

  // 选中的日期
  const selectedDate = ref<Date | null>(null);

  // 正在编辑的事件 ID
  const editingEventId = ref<string | null>(null);

  // 加载状态
  const isLoading = ref(false);

  // ========================================
  // Getters
  // ========================================

  // 当前年月
  const currentYearMonth = computed(() => {
    const year = currentDate.value.getFullYear();
    const month = currentDate.value.getMonth();
    return { year, month };
  });

  // 当前月份的标题
  const monthTitle = computed(() => {
    const year = currentDate.value.getFullYear();
    const month = currentDate.value.getMonth() + 1;
    return `${year}年${month}月`;
  });

  // 当前月份的事件
  const currentMonthEvents = computed(() => {
    const { year, month } = currentYearMonth.value;
    const startOfMonth = new Date(year, month, 1);
    const endOfMonth = new Date(year, month + 1, 0);

    return events.value.filter((event) => {
      const eventDate = new Date(event.date);
      return eventDate >= startOfMonth && eventDate <= endOfMonth;
    });
  });

  // 获取选中日期的事件
  const selectedDateEvents = computed(() => {
    if (!selectedDate.value) return [];
    return getEventsForDate(selectedDate.value);
  });

  // 正在编辑的事件
  const editingEvent = computed(() => {
    if (!editingEventId.value) return null;
    return events.value.find((e) => e.id === editingEventId.value) || null;
  });

  // 日历网格数据
  const calendarGrid = computed(() => {
    const { year, month } = currentYearMonth.value;
    const firstDay = new Date(year, month, 1);
    const lastDay = new Date(year, month + 1, 0);
    const startDayOfWeek = firstDay.getDay();
    const totalDays = lastDay.getDate();
    const prevMonthLastDay = new Date(year, month, 0).getDate();

    const days: Array<{
      date: Date;
      isCurrentMonth: boolean;
      isToday: boolean;
      events: CalendarEvent[];
    }> = [];

    // 上个月的天数
    for (let i = startDayOfWeek - 1; i >= 0; i--) {
      const date = new Date(year, month - 1, prevMonthLastDay - i);
      days.push({
        date,
        isCurrentMonth: false,
        isToday: isSameDay(date, new Date()),
        events: getEventsForDate(date),
      });
    }

    // 当前月份的天数
    for (let i = 1; i <= totalDays; i++) {
      const date = new Date(year, month, i);
      days.push({
        date,
        isCurrentMonth: true,
        isToday: isSameDay(date, new Date()),
        events: getEventsForDate(date),
      });
    }

    // 下个月的天数（填充到 6 行）
    const remainingCells = 42 - days.length;
    for (let i = 1; i <= remainingCells; i++) {
      const date = new Date(year, month + 1, i);
      days.push({
        date,
        isCurrentMonth: false,
        isToday: isSameDay(date, new Date()),
        events: getEventsForDate(date),
      });
    }

    return days;
  });

  // ========================================
  // Actions
  // ========================================

  // 获取事件列表
  async function fetchEvents() {
    isLoading.value = true;
    try {
      await delay(200);
      events.value = generateCalendarEvents(15);
    } catch (error) {
      console.error("Failed to fetch events:", error);
    } finally {
      isLoading.value = false;
    }
  }

  // 获取指定日期的事件
  function getEventsForDate(date: Date): CalendarEvent[] {
    const dateNormalized = new Date(
      date.getFullYear(),
      date.getMonth(),
      date.getDate(),
    );

    return events.value.filter((event) => {
      const eventDate = new Date(event.date);
      const eventDateNormalized = new Date(
        eventDate.getFullYear(),
        eventDate.getMonth(),
        eventDate.getDate(),
      );

      // 非重复事件
      if (event.repeat === "none") {
        return eventDateNormalized.getTime() === dateNormalized.getTime();
      }

      // 重复事件
      return isRecurringEventOnDate(event, dateNormalized);
    });
  }

  // 检查重复事件是否在指定日期
  function isRecurringEventOnDate(event: CalendarEvent, date: Date): boolean {
    const eventDate = new Date(event.date);
    const eventDateNormalized = new Date(
      eventDate.getFullYear(),
      eventDate.getMonth(),
      eventDate.getDate(),
    );

    // 检查是否在重复结束日期之后
    if (event.repeatEnd) {
      const repeatEndDate = new Date(event.repeatEnd);
      if (date > repeatEndDate) return false;
    }

    // 检查是否在事件开始日期之前
    if (date < eventDateNormalized) return false;

    const diffTime = date.getTime() - eventDateNormalized.getTime();
    const diffDays = Math.floor(diffTime / (1000 * 60 * 60 * 24));

    switch (event.repeat) {
      case "daily":
        return true;
      case "weekly":
        return diffDays % 7 === 0;
      case "monthly":
        return date.getDate() === eventDate.getDate();
      case "yearly":
        return (
          date.getDate() === eventDate.getDate() &&
          date.getMonth() === eventDate.getMonth()
        );
      default:
        return false;
    }
  }

  // 选择日期
  function selectDate(date: Date) {
    selectedDate.value = date;
  }

  // 清除选中的日期
  function clearSelectedDate() {
    selectedDate.value = null;
  }

  // 导航月份
  function navigateMonth(delta: number) {
    const newDate = new Date(currentDate.value);
    newDate.setMonth(newDate.getMonth() + delta);
    currentDate.value = newDate;
  }

  // 上一个月
  function previousMonth() {
    navigateMonth(-1);
  }

  // 下一个月
  function nextMonth() {
    navigateMonth(1);
  }

  // 回到今天
  function goToToday() {
    currentDate.value = new Date();
    selectedDate.value = new Date();
  }

  // 开始创建事件
  function startCreateEvent(date?: Date) {
    editingEventId.value = null;
    selectedDate.value = date || new Date();
  }

  // 开始编辑事件
  function startEditEvent(eventId: string) {
    editingEventId.value = eventId;
    const event = events.value.find((e) => e.id === eventId);
    if (event) {
      selectedDate.value = new Date(event.date);
    }
  }

  // 取消编辑
  function cancelEdit() {
    editingEventId.value = null;
  }

  // 保存事件
  async function saveEvent(
    eventData: Partial<CalendarEvent>,
  ): Promise<CalendarEvent> {
    isLoading.value = true;
    try {
      await delay(300);

      if (editingEventId.value) {
        // 更新现有事件
        const index = events.value.findIndex(
          (e) => e.id === editingEventId.value,
        );
        if (index !== -1) {
          events.value[index] = {
            ...events.value[index],
            ...eventData,
          } as CalendarEvent;
          return events.value[index];
        }
      }

      // 创建新事件
      const newEvent: CalendarEvent = {
        id: `event-${Date.now()}`,
        title: eventData.title || "新事件",
        date: eventData.date || new Date(),
        startTime: eventData.startTime || "09:00",
        endTime: eventData.endTime || "10:00",
        repeat: eventData.repeat || "none",
        repeatEnd: eventData.repeatEnd,
        color: eventData.color || "#7C3AED",
        notes: eventData.notes || "",
      };

      events.value.push(newEvent);
      return newEvent;
    } finally {
      isLoading.value = false;
      editingEventId.value = null;
    }
  }

  // 删除事件
  async function deleteEvent(eventId: string) {
    isLoading.value = true;
    try {
      await delay(200);
      const index = events.value.findIndex((e) => e.id === eventId);
      if (index !== -1) {
        events.value.splice(index, 1);
      }
    } finally {
      isLoading.value = false;
      editingEventId.value = null;
    }
  }

  // 更新事件
  function updateEvent(eventId: string, updates: Partial<CalendarEvent>) {
    const event = events.value.find((e) => e.id === eventId);
    if (event) {
      Object.assign(event, updates);
    }
  }

  // 清空所有事件
  function clearEvents() {
    events.value = [];
    selectedDate.value = null;
    editingEventId.value = null;
  }

  // ========================================
  // Helper Functions
  // ========================================

  // 检查是否是同一天
  function isSameDay(date1: Date, date2: Date): boolean {
    return (
      date1.getFullYear() === date2.getFullYear() &&
      date1.getMonth() === date2.getMonth() &&
      date1.getDate() === date2.getDate()
    );
  }

  return {
    // State
    events,
    currentDate,
    selectedDate,
    editingEventId,
    isLoading,

    // Getters
    currentYearMonth,
    monthTitle,
    currentMonthEvents,
    selectedDateEvents,
    editingEvent,
    calendarGrid,

    // Actions
    fetchEvents,
    getEventsForDate,
    selectDate,
    clearSelectedDate,
    navigateMonth,
    previousMonth,
    nextMonth,
    goToToday,
    startCreateEvent,
    startEditEvent,
    cancelEdit,
    saveEvent,
    deleteEvent,
    updateEvent,
    clearEvents,
  };
});
