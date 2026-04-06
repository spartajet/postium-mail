// Tauri doesn't have a Node.js server to do proper SSR
// so we use adapter-static with a fallback to index.html to put the site in SPA mode
// See: https://svelte.dev/docs/kit/single-page-apps
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  compilerOptions: {
    warningFilter: (warning) => {
      // 忽略 a11y label 关联警告
      if (warning.code === "a11y_label_has_associated_control") return false;
      if (warning.code === "a11y_click_events_have_key_events") return false;
      if (warning.code === "a11y_no_static_element_interactions") return false;
      if (warning.code === "a11y_click_events_have_key_events") return false;
      if (warning.code === "a11y_no_static_element_interactions") return false;
      if (warning.code === "a11y_no_static_element_interactions") return false;
      if (warning.code === "a11y_no_static_element_interactions") return false;
    },
  },
  kit: {
    adapter: adapter({
      fallback: "index.html",
    }),
  },
};

export default config;
