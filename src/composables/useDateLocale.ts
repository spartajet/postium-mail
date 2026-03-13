import { computed } from "vue";
import { useI18n } from "vue-i18n";
import {
    zhCN,
    zhTW,
    enUS,
    fr,
    de,
    ja,
} from "date-fns/locale";

// 日期语言映射
const dateLocales = {
    'zh-CN': zhCN,
    'zh-TW': zhTW,
    'en-US': enUS,
    'fr-FR': fr,
    'de-DE': de,
    'ja-JP': ja,
} as const;

/**
 * 日期格式化语言切换 Composable
 * 用于根据当前语言自动切换 date-fns 的语言包
 */
export function useDateLocale() {
    const { locale } = useI18n();

    // 当前日期语言包
    const currentDateLocale = computed(() => {
        const currentLocale = locale.value;
        return dateLocales[currentLocale as keyof typeof dateLocales] || enUS;
    });

    return {
        currentDateLocale,
    };
}
