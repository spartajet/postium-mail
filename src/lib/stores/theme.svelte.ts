import { getContext, setContext } from 'svelte';

type Theme = 'light' | 'dark' | 'system';

class ThemeState {
  theme = $state<Theme>('system');
  resolved = $state<'light' | 'dark'>('light');

  constructor() {
    const saved = localStorage.getItem('postium-theme') as Theme | null;
    if (saved) {
      this.theme = saved;
    }

    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    this.resolved = this.getResolved();

    mediaQuery.addEventListener('change', () => {
      if (this.theme === 'system') {
        this.resolved = this.getResolved();
        this.applyClass();
      }
    });

    this.applyClass();
  }

  private getResolved(): 'light' | 'dark' {
    if (this.theme === 'system') {
      return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
    }
    return this.theme;
  }

  private applyClass() {
    document.documentElement.classList.toggle('dark', this.resolved === 'dark');
  }

  setTheme(theme: Theme) {
    this.theme = theme;
    localStorage.setItem('postium-theme', theme);
    this.resolved = this.getResolved();
    this.applyClass();
  }
}

const THEME_KEY = Symbol('theme');

export function createThemeState() {
  const state = new ThemeState();
  setContext(THEME_KEY, state);
  return state;
}

export function getThemeState() {
  return getContext<ThemeState>(THEME_KEY);
}
