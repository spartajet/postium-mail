<script setup lang="ts">
import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { SearchOutlined, CloseOutlined } from "@vicons/material";

// Props
const props = defineProps<{
  accountId?: number;
  placeholder?: string;
}>();

// Emits
const emit = defineEmits<{
  search: [query: string, results: any[]];
  clear: [];
}>();

// State
const searchQuery = ref("");
const isSearching = ref(false);
const searchResults = ref<any[]>([]);
const showResults = ref(false);

// Computed
const hasQuery = computed(() => searchQuery.value.trim().length > 0);
const resultCount = computed(() => searchResults.value.length);

// Methods
async function handleSearch() {
  const query = searchQuery.value.trim();
  if (!query) return;

  isSearching.value = true;
  showResults.value = true;

  try {
    const results = await invoke<any[]>("search_emails_fts", {
      query,
      accountId: props.accountId || null,
      limit: 50,
    });

    searchResults.value = results;
    emit("search", query, results);
  } catch (error) {
    console.error("搜索失败:", error);
    searchResults.value = [];
  } finally {
    isSearching.value = false;
  }
}

function handleClear() {
  searchQuery.value = "";
  searchResults.value = [];
  showResults.value = false;
  emit("clear");
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === "Enter") {
    handleSearch();
  } else if (event.key === "Escape") {
    handleClear();
  }
}

function selectResult(result: any) {
  // 发出选择事件，让父组件处理邮件详情显示
  emit("search", searchQuery.value, [result]);
  showResults.value = false;
}

// Expose methods
defineExpose({
  focus: () => {
    const input = document.querySelector(".search-input") as HTMLInputElement;
    input?.focus();
  },
  clear: handleClear,
});
</script>

<template>
  <div class="search-bar">
    <div class="search-input-wrapper">
      <SearchOutlined class="search-icon" :size="20" />
      <input
        ref="searchInput"
        v-model="searchQuery"
        type="text"
        class="search-input"
        :placeholder="placeholder || '搜索邮件...'"
        @keydown="handleKeydown"
      />
      <CloseOutlined
        v-if="hasQuery"
        class="clear-icon"
        :size="18"
        @click="handleClear"
      />
    </div>

    <!-- 搜索结果下拉框 -->
    <Transition name="dropdown">
      <div v-if="showResults && (hasQuery || isSearching)" class="search-results">
        <!-- 加载状态 -->
        <div v-if="isSearching" class="search-loading">
          <div class="spinner"></div>
          <span>搜索中...</span>
        </div>

        <!-- 无结果 -->
        <div v-else-if="resultCount === 0 && hasQuery" class="search-no-results">
          <span>未找到相关邮件</span>
        </div>

        <!-- 结果列表 -->
        <div v-else-if="resultCount > 0" class="search-results-list">
          <div class="search-results-header">
            <span>找到 {{ resultCount }} 封邮件</span>
          </div>
          <div
            v-for="result in searchResults"
            :key="result.id"
            class="search-result-item"
            @click="selectResult(result)"
          >
            <div class="result-subject">{{ result.subject }}</div>
            <div class="result-meta">
              <span class="result-sender">{{ result.sender_email }}</span>
              <span class="result-folder">{{ result.folder }}</span>
            </div>
            <div v-if="result.body_text" class="result-preview">
              {{ result.body_text.slice(0, 100) }}...
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.search-bar {
  position: relative;
  width: 100%;
  max-width: 600px;
}

.search-input-wrapper {
  position: relative;
  display: flex;
  align-items: center;
  background: var(--bg-elevated);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 8px 12px;
  transition: all 0.2s ease;
}

.search-input-wrapper:focus-within {
  border-color: var(--primary-color);
  box-shadow: 0 0 0 3px rgba(var(--primary-rgb), 0.1);
}

.search-icon {
  color: var(--text-muted);
  margin-right: 8px;
}

.search-input {
  flex: 1;
  border: none;
  background: transparent;
  font-size: 14px;
  color: var(--text-primary);
  outline: none;
}

.search-input::placeholder {
  color: var(--text-muted);
}

.clear-icon {
  color: var(--text-muted);
  cursor: pointer;
  transition: color 0.2s ease;
}

.clear-icon:hover {
  color: var(--text-primary);
}

/* 搜索结果下拉框 */
.search-results {
  position: absolute;
  top: calc(100% + 8px);
  left: 0;
  right: 0;
  background: var(--bg-elevated);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.15);
  max-height: 400px;
  overflow-y: auto;
  z-index: 1000;
}

/* 加载状态 */
.search-loading {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
  gap: 12px;
  color: var(--text-secondary);
}

.spinner {
  width: 20px;
  height: 20px;
  border: 2px solid var(--border-color);
  border-top-color: var(--primary-color);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

/* 无结果 */
.search-no-results {
  padding: 24px;
  text-align: center;
  color: var(--text-muted);
}

/* 结果列表 */
.search-results-header {
  padding: 12px 16px;
  border-bottom: 1px solid var(--border-color);
  font-size: 12px;
  color: var(--text-muted);
  font-weight: 500;
}

.search-results-list {
  max-height: 350px;
  overflow-y: auto;
}

.search-result-item {
  padding: 12px 16px;
  border-bottom: 1px solid var(--border-color);
  cursor: pointer;
  transition: background 0.2s ease;
}

.search-result-item:hover {
  background: var(--bg-hover);
}

.search-result-item:last-child {
  border-bottom: none;
}

.result-subject {
  font-size: 14px;
  font-weight: 500;
  color: var(--text-primary);
  margin-bottom: 4px;
}

.result-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: var(--text-secondary);
  margin-bottom: 4px;
}

.result-sender {
  font-weight: 500;
}

.result-folder {
  padding: 2px 8px;
  background: var(--bg-subtle);
  border-radius: 4px;
  font-size: 11px;
}

.result-preview {
  font-size: 13px;
  color: var(--text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
}

/* 过渡动画 */
.dropdown-enter-active,
.dropdown-leave-active {
  transition: all 0.2s ease;
}

.dropdown-enter-from,
.dropdown-leave-to {
  opacity: 0;
  transform: translateY(-8px);
}

/* 滚动条样式 */
.search-results::-webkit-scrollbar,
.search-results-list::-webkit-scrollbar {
  width: 6px;
}

.search-results::-webkit-scrollbar-track,
.search-results-list::-webkit-scrollbar-track {
  background: transparent;
}

.search-results::-webkit-scrollbar-thumb,
.search-results-list::-webkit-scrollbar-thumb {
  background: var(--border-color);
  border-radius: 3px;
}

.search-results::-webkit-scrollbar-thumb:hover,
.search-results-list::-webkit-scrollbar-thumb:hover {
  background: var(--text-muted);
}
</style>
