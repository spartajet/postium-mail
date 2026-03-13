import {
    zhCN,
    enUS,
    frFR,
    deDE,
    jaJP,
    dateZhCN,
    dateEnUS,
    dateFrFR,
    dateDeDE,
    dateJaJP
} from "naive-ui";
import type { NLocale, NDateLocale } from "naive-ui";

// Naive UI 语言包映射
// 注意：Naive UI 没有繁体中文语言包，zh-TW 使用 zh-CN
export const naiveUILocales: Record<string, NLocale> = {
    "zh-CN": zhCN,
    "zh-TW": zhCN, // 使用简体中文作为繁体的替代
    "en-US": enUS,
    "fr-FR": frFR,
    "de-DE": deDE,
    "ja-JP": jaJP,
};

// Naive UI 日期语言包映射
export const naiveUIDateLocales: Record<string, NDateLocale> = {
    "zh-CN": dateZhCN,
    "zh-TW": dateZhCN, // 使用简体中文作为繁体的替代
    "en-US": dateEnUS,
    "fr-FR": dateFrFR,
    "de-DE": dateDeDE,
    "ja-JP": dateJaJP,
};

// 获取 Naive UI 语言包
export function getNaiveUILocale(code: string): NLocale {
    return naiveUILocales[code] || enUS;
}

// 获取 Naive UI 日期语言包
export function getNaiveUIDateLocale(code: string): NDateLocale {
    return naiveUIDateLocales[code] || dateEnUS;
}
