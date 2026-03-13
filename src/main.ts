import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";

// 导入全局样式
import "@/assets/styles/global.scss";

// 创建应用实例
const app = createApp(App);

// 使用 Pinia 状态管理
const pinia = createPinia();
app.use(pinia);

// 挂载应用
app.mount("#app");
