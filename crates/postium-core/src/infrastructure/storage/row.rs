//! SQLite 布尔值转换工具模块
//!
//! SQLite 原生不支持布尔类型，布尔值以整数（0/1）存储。
//! 本模块提供整数与布尔值之间的双向转换函数，
//! 用于在数据库读写时统一处理布尔字段。

/// 将整数转换为布尔值（0 为 false，非 0 为 true）。
pub fn int_to_bool(value: i64) -> bool {
    value != 0
}

/// 将 `Option<i64>` 转换为 `Option<bool>`，`None` 保持不变。
pub fn opt_int_to_bool(value: Option<i64>) -> Option<bool> {
    value.map(int_to_bool)
}

/// 将布尔值转换为整数（true 为 1，false 为 0）。
pub fn bool_to_int(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

/// 将 `Option<bool>` 转换为 `Option<i64>`，`None` 保持不变。
pub fn opt_bool_to_int(value: Option<bool>) -> Option<i64> {
    value.map(bool_to_int)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int_to_bool_treats_zero_as_false_and_nonzero_as_true() {
        assert!(!int_to_bool(0));
        assert!(int_to_bool(1));
        assert!(int_to_bool(-1));
    }

    #[test]
    fn optional_conversions_preserve_none_and_convert_some_values() {
        assert_eq!(opt_int_to_bool(None), None);
        assert_eq!(opt_int_to_bool(Some(0)), Some(false));
        assert_eq!(opt_bool_to_int(None), None);
        assert_eq!(opt_bool_to_int(Some(true)), Some(1));
    }
}
