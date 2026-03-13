<script setup lang="ts">
import { ref, computed, watch } from "vue";
import type { CalendarEvent } from "@/types";
import { format } from "date-fns";

// Props
interface Props {
    event?: CalendarEvent | null;
    selectedDate?: Date | null;
}

const props = defineProps<Props>();

// Emits
const emit = defineEmits<{
    close: [];
    save: [eventData: Partial<CalendarEvent>];
    delete: [eventId: string];
}>();

// 表单数据
const form = ref({
    title: "",
    date: format(new Date(), "yyyy-MM-dd"),
    startTime: "09:00",
    endTime: "10:00",
    repeat: "none",
    repeatEnd: "",
    color: "#7C3AED",
    notes: "",
});

// 颜色选项
const colorOptions = [
    { value: "#7C3AED", label: "紫色" },
    { value: "#3B82F6", label: "蓝色" },
    { value: "#10B981", label: "绿色" },
    { value: "#F59E0B", label: "橙色" },
    { value: "#EF4444", label: "红色" },
    { value: "#EC4899", label: "粉色" },
];

// 重复选项
const repeatOptions = [
    { value: "none", label: "不重复" },
    { value: "daily", label: "每天" },
    { value: "weekly", label: "每周" },
    { value: "monthly", label: "每月" },
    { value: "yearly", label: "每年" },
];

// 是否是编辑模式
const isEditMode = computed(() => !!props.event);

// 监听 event 变化，填充表单
watch(
    () => props.event,
    (newEvent) => {
        if (newEvent) {
            form.value = {
                title: newEvent.title,
                date: format(new Date(newEvent.date), "yyyy-MM-dd"),
                startTime: newEvent.startTime,
                endTime: newEvent.endTime,
                repeat: newEvent.repeat,
                repeatEnd: newEvent.repeatEnd || "",
                color: newEvent.color,
                notes: newEvent.notes,
            };
        }
    },
    { immediate: true }
);

// 监听 selectedDate 变化
watch(
    () => props.selectedDate,
    (newDate) => {
        if (newDate && !props.event) {
            form.value.date = format(new Date(newDate), "yyyy-MM-dd");
        }
    },
    { immediate: true }
);

// 关闭模态框
function handleClose() {
    emit("close");
}

// 保存事件
function handleSave() {
    if (!form.value.title.trim()) {
        return;
    }

    const eventData: Partial<CalendarEvent> = {
        title: form.value.title,
        date: new Date(form.value.date),
        startTime: form.value.startTime,
        endTime: form.value.endTime,
        repeat: form.value.repeat as any,
        repeatEnd: form.value.repeatEnd || undefined,
        color: form.value.color,
        notes: form.value.notes,
    };

    emit("save", eventData);
}

// 删除事件
function handleDelete() {
    if (props.event?.id) {
        emit("delete", props.event.id);
    }
}

// 点击遮罩关闭
function handleOverlayClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
        handleClose();
    }
}
</script>

<template>
    <Teleport to="body">
        <Transition name="modal">
            <div class="modal-overlay show" @click="handleOverlayClick">
                <div class="modal">
                    <!-- 头部 -->
                    <div class="modal-header">
                        <h3>
                            <svg
                                class="modal-icon"
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                            >
                                <path
                                    d="M19 4h-1V2h-2v2H8V2H6v2H5c-1.11 0-1.99.9-1.99 2L3 20c0 1.1.89 2 2 2h14c1.1 0 2-.9 2-2V6c0-1.1-.9-2-2-2zm0 16H5V9h14v11zM9 11H7v2h2v-2zm4 0h-2v2h2v-2zm4 0h-2v2h2v-2zm-8 4H7v2h2v-2zm4 0h-2v2h2v-2zm4 0h-2v2h2v-2z"
                                />
                            </svg>
                            {{ isEditMode ? "编辑事务" : "新建事务" }}
                        </h3>
                        <button class="icon-btn" @click="handleClose">
                            <svg
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                            >
                                <path
                                    d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"
                                />
                            </svg>
                        </button>
                    </div>

                    <!-- 表单内容 -->
                    <div class="modal-body">
                        <!-- 标题 -->
                        <div class="form-group">
                            <label>标题</label>
                            <input
                                v-model="form.title"
                                type="text"
                                placeholder="输入事务标题"
                            />
                        </div>

                        <!-- 日期和时间 -->
                        <div class="form-row">
                            <div class="form-group">
                                <label>日期</label>
                                <input v-model="form.date" type="date" />
                            </div>
                            <div class="form-group">
                                <label>开始时间</label>
                                <input v-model="form.startTime" type="time" />
                            </div>
                            <div class="form-group">
                                <label>结束时间</label>
                                <input v-model="form.endTime" type="time" />
                            </div>
                        </div>

                        <!-- 重复 -->
                        <div class="form-row">
                            <div class="form-group">
                                <label>重复</label>
                                <select v-model="form.repeat">
                                    <option
                                        v-for="option in repeatOptions"
                                        :key="option.value"
                                        :value="option.value"
                                    >
                                        {{ option.label }}
                                    </option>
                                </select>
                            </div>
                            <div
                                v-if="form.repeat !== 'none'"
                                class="form-group"
                            >
                                <label>结束日期</label>
                                <input v-model="form.repeatEnd" type="date" />
                            </div>
                        </div>

                        <!-- 颜色 -->
                        <div class="form-group">
                            <label>颜色</label>
                            <div class="color-picker">
                                <label
                                    v-for="color in colorOptions"
                                    :key="color.value"
                                    class="color-option"
                                >
                                    <input
                                        v-model="form.color"
                                        type="radio"
                                        :value="color.value"
                                    />
                                    <span
                                        class="color-dot"
                                        :style="{ backgroundColor: color.value }"
                                    ></span>
                                </label>
                            </div>
                        </div>

                        <!-- 备注 -->
                        <div class="form-group">
                            <label>备注</label>
                            <textarea
                                v-model="form.notes"
                                placeholder="添加备注..."
                                rows="3"
                            ></textarea>
                        </div>
                    </div>

                    <!-- 底部按钮 -->
                    <div class="modal-footer">
                        <button
                            v-if="isEditMode"
                            class="btn btn-danger"
                            @click="handleDelete"
                        >
                            删除
                        </button>
                        <div class="footer-right">
                            <button class="btn btn-ghost" @click="handleClose">
                                取消
                            </button>
                            <button
                                class="btn btn-primary"
                                :disabled="!form.title.trim()"
                                @click="handleSave"
                            >
                                保存
                            </button>
                        </div>
                    </div>
                </div>
            </div>
        </Transition>
    </Teleport>
</template>

<style scoped>
.modal-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.6);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 400;
    opacity: 0;
    visibility: hidden;
    transition: all 0.2s ease;
}

.modal-overlay.show {
    opacity: 1;
    visibility: visible;
}

.modal {
    width: 90%;
    max-width: 500px;
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: 12px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
    display: flex;
    flex-direction: column;
    max-height: 90vh;
    transform: scale(0.95);
    transition: transform 0.2s ease;
}

.modal-overlay.show .modal {
    transform: scale(1);
}

.modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 20px 24px;
    border-bottom: 1px solid var(--border-subtle);
}

.modal-header h3 {
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 18px;
    font-weight: 600;
    color: var(--text-primary);
    margin: 0;
}

.modal-icon {
    width: 24px;
    height: 24px;
    color: var(--primary);
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

.icon-btn svg {
    width: 20px;
    height: 20px;
}

.modal-body {
    padding: 24px;
    overflow-y: auto;
    flex: 1;
}

.form-group {
    margin-bottom: 20px;
}

.form-group label {
    display: block;
    font-size: 14px;
    font-weight: 500;
    color: var(--text-secondary);
    margin-bottom: 8px;
}

.form-row {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
    gap: 16px;
}

.form-group input,
.form-group select,
.form-group textarea {
    width: 100%;
    padding: 10px 14px;
    background: var(--bg-glass);
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
    font-size: 14px;
    color: var(--text-primary);
    transition: all 0.15s ease;
}

.form-group input:focus,
.form-group select:focus,
.form-group textarea:focus {
    outline: none;
    border-color: var(--primary);
    box-shadow: 0 0 0 3px var(--primary-light);
}

.form-group textarea {
    resize: vertical;
    min-height: 80px;
}

.color-picker {
    display: flex;
    gap: 12px;
    flex-wrap: wrap;
}

.color-option {
    position: relative;
    cursor: pointer;
}

.color-option input[type="radio"] {
    position: absolute;
    opacity: 0;
    width: 0;
    height: 0;
}

.color-dot {
    display: block;
    width: 32px;
    height: 32px;
    border-radius: 50%;
    border: 2px solid transparent;
    transition: all 0.15s ease;
}

.color-option input[type="radio"]:checked + .color-dot {
    border-color: white;
    box-shadow: 0 0 0 2px var(--primary);
}

.color-option:hover .color-dot {
    transform: scale(1.1);
}

.modal-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 24px;
    border-top: 1px solid var(--border-subtle);
    gap: 12px;
}

.footer-right {
    display: flex;
    gap: 12px;
    margin-left: auto;
}

.btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 10px 20px;
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
}

.btn-primary:hover:not(:disabled) {
    transform: translateY(-1px);
}

.btn-primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
}

.btn-ghost {
    background: transparent;
    color: var(--text-secondary);
}

.btn-ghost:hover {
    background: var(--bg-glass-hover);
    color: var(--text-primary);
}

.btn-danger {
    background: rgba(239, 68, 68, 0.15);
    color: #ef4444;
}

.btn-danger:hover {
    background: rgba(239, 68, 68, 0.25);
}

/* Transition */
.modal-enter-active,
.modal-leave-active {
    transition: opacity 0.2s ease;
}

.modal-enter-from,
.modal-leave-to {
    opacity: 0;
}

.modal-enter-active .modal,
.modal-leave-active .modal {
    transition: transform 0.2s ease;
}

.modal-enter-from .modal,
.modal-leave-to .modal {
    transform: scale(0.95);
}

@media (max-width: 640px) {
    .modal {
        width: 95%;
        max-height: 95vh;
    }

    .modal-body {
        padding: 16px;
    }

    .form-row {
        grid-template-columns: 1fr;
    }
}
</style>
