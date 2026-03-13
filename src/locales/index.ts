import zhCN from './zh-CN';
import zhTW from './zh-TW';
import enUS from './en-US';
import frFR from './fr-FR';
import deDE from './de-DE';
import jaJP from './ja-JP';

// 可用语言列表
export const availableLanguages = [
    { code: 'zh-CN', name: '简体中文', flag: '🇨🇳' },
    { code: 'zh-TW', name: '繁體中文', flag: '🇹🇼' },
    { code: 'en-US', name: 'English', flag: '🇺🇸' },
    { code: 'fr-FR', name: 'Français', flag: '🇫🇷' },
    { code: 'de-DE', name: 'Deutsch', flag: '🇩🇪' },
    { code: 'ja-JP', name: '日本語', flag: '🇯🇵' },
] as const;

// 语言代码类型
export type LanguageCode = typeof availableLanguages[number]['code'];

// 导出所有语言包
export const messages = {
    'zh-CN': zhCN,
    'zh-TW': zhTW,
    'en-US': enUS,
    'fr-FR': frFR,
    'de-DE': deDE,
    'ja-JP': jaJP,
};

// 默认语言
export const defaultLocale: LanguageCode = 'zh-CN';

// localStorage 键
const SETTINGS_KEY = 'postium-settings';

// 获取保存的语言设置
export function getSavedLocale(): LanguageCode {
    if (typeof window === 'undefined') {
        return defaultLocale;
    }

    try {
        const saved = localStorage.getItem(SETTINGS_KEY);
        if (saved) {
            const settings = JSON.parse(saved);
            if (settings.language && typeof settings.language === 'string') {
                // 验证保存的语言是否在支持列表中
                const isValid = availableLanguages.some(lang => lang.code === settings.language);
                if (isValid) {
                    return settings.language as LanguageCode;
                }
            }
        }
    } catch (e) {
        console.error('Failed to load saved language:', e);
    }

    return defaultLocale;
}

// 获取语言配置
export function getLanguageConfig(code: string) {
    return availableLanguages.find(lang => lang.code === code);
}

// 获取语言名称
export function getLanguageName(code: string): string {
    const config = getLanguageConfig(code);
    return config?.name || code;
}

// 获取语言国旗
export function getLanguageFlag(code: string): string {
    const config = getLanguageConfig(code);
    return config?.flag || '🌐';
}
