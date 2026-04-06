import zhCN from '$lib/i18n/zh-CN';
import enUS from '$lib/i18n/en-US';
import { getContext, setContext } from 'svelte';

export type Locale = 'zh-CN' | 'en-US';

// 递归将 literal string 转为 string，保持结构
type LooseLiteral<T> = T extends string ? string : T extends readonly (infer U)[] ? LooseLiteral<U>[] : T extends object ? { [K in keyof T]: LooseLiteral<T[K]> } : T;
type Translation = LooseLiteral<typeof zhCN>;

const translations: Record<Locale, Translation> = {
  'zh-CN': zhCN,
  'en-US': enUS,
};

class I18nState {
  locale = $state<Locale>('zh-CN');
  t = $derived(translations[this.locale]);

  constructor() {
    const saved = localStorage.getItem('postium-locale') as Locale | null;
    if (saved && translations[saved]) {
      this.locale = saved;
    }
  }

  setLocale(locale: Locale) {
    this.locale = locale;
    localStorage.setItem('postium-locale', locale);
  }
}

const I18N_KEY = Symbol('i18n');

export function createI18nState() {
  const state = new I18nState();
  setContext(I18N_KEY, state);
  return state;
}

export function getI18nState() {
  return getContext<I18nState>(I18N_KEY);
}
