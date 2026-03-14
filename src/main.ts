import { createApp } from "vue";
import { createPinia } from "pinia";
import { createI18n } from "vue-i18n";
import App from "./App.vue";

// 导入全局样式
import "@/assets/styles/global.scss";

// 导入语言包
import { messages, defaultLocale, getSavedLocale } from "@/locales";

// 从 localStorage 读取保存的语言，如果没有则使用默认语言
const initialLocale = getSavedLocale();

// 创建 i18n 实例
const i18n = createI18n({
    legacy: false, // 使用 Composition API 模式
    locale: initialLocale, // 使用保存的语言或默认语言
    fallbackLocale: defaultLocale, // 回退语言
    messages,
});

// 创建应用实例
const app = createApp(App);

// 使用 Pinia 状态管理
const pinia = createPinia();
app.use(pinia);

// 使用 i18n 插件
app.use(i18n);

// 挂载应用
app.mount("#app");

// 导出 i18n 实例供其他模块使用
export { i18n };
