import { defineConfig } from "vitest/config";
import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [tailwindcss(), sveltekit()],
  test: {
    environment: "jsdom",
    setupFiles: ["./vitest.setup.js"],
    include: ["src/lib/__tests__/**/*.test.ts"],
  },
});
