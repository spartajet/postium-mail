<script lang="ts">
  import { getI18nState } from '$lib/stores/i18n.svelte';
  import { Calendar as CalendarIcon, ChevronLeft, ChevronRight, Plus } from 'lucide-svelte';

  const i18n = getI18nState();
  const t = $derived(i18n.t);

  let currentDate = $state(new Date());

  function getDaysInMonth(year: number, month: number): number {
    return new Date(year, month + 1, 0).getDate();
  }

  function getFirstDayOfMonth(year: number, month: number): number {
    return new Date(year, month, 1).getDay();
  }

  const weekDays = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];

  const calendarDays = $derived.by(() => {
    const year = currentDate.getFullYear();
    const month = currentDate.getMonth();
    const daysInMonth = getDaysInMonth(year, month);
    const firstDay = getFirstDayOfMonth(year, month);
    const days: (number | null)[] = [];
    for (let i = 0; i < firstDay; i++) days.push(null);
    for (let i = 1; i <= daysInMonth; i++) days.push(i);
    return days;
  });

  const monthLabel = $derived(
    currentDate.toLocaleDateString(undefined, { year: 'numeric', month: 'long' })
  );

  function prevMonth() {
    currentDate = new Date(currentDate.getFullYear(), currentDate.getMonth() - 1, 1);
  }

  function nextMonth() {
    currentDate = new Date(currentDate.getFullYear(), currentDate.getMonth() + 1, 1);
  }

  function isToday(day: number | null): boolean {
    if (!day) return false;
    const today = new Date();
    return (
      day === today.getDate() &&
      currentDate.getMonth() === today.getMonth() &&
      currentDate.getFullYear() === today.getFullYear()
    );
  }
</script>

<div class="flex h-full flex-col overflow-hidden bg-background">
  <!-- Header -->
  <div class="flex items-center justify-between border-b border-border px-6 py-4">
    <div class="flex items-center gap-2">
      <CalendarIcon size={20} class="text-primary" />
      <h1 class="text-lg font-semibold text-foreground">{t.sidebar.calendar}</h1>
    </div>
    <div class="flex items-center gap-2">
      <button
        class="flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
        onclick={prevMonth}
      >
        <ChevronLeft size={18} />
      </button>
      <span class="min-w-[140px] text-center text-sm font-medium text-foreground">{monthLabel}</span>
      <button
        class="flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
        onclick={nextMonth}
      >
        <ChevronRight size={18} />
      </button>
    </div>
  </div>

  <!-- Calendar grid -->
  <div class="flex-1 p-6">
    <div class="grid grid-cols-7 gap-px rounded-lg border border-border bg-border overflow-hidden">
      {#each weekDays as day}
        <div class="bg-glass px-2 py-2 text-center text-xs font-medium text-muted-foreground">
          {day}
        </div>
      {/each}
      {#each calendarDays as day, i ('d' + i)}
        <div class="bg-background px-2 py-3 text-center text-sm transition-colors hover:bg-glass-hover {isToday(day) ? 'font-bold text-primary' : day ? 'text-foreground' : 'text-transparent'}">
          {day || ''}
        </div>
      {/each}
    </div>

    <!-- Events placeholder -->
    <div class="mt-6">
      <div class="flex items-center justify-between mb-3">
        <h3 class="text-sm font-semibold text-foreground">Events</h3>
        <button class="flex items-center gap-1 rounded-md px-2 py-1 text-xs text-primary transition-colors hover:bg-primary/10">
          <Plus size={14} />
          Add Event
        </button>
      </div>
      <div class="rounded-lg border border-dashed border-border px-4 py-8 text-center text-sm text-muted-foreground">
        No events for this month
      </div>
    </div>
  </div>
</div>
