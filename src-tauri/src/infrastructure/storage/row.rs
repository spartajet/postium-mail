pub fn int_to_bool(value: i64) -> bool {
    value != 0
}

pub fn opt_int_to_bool(value: Option<i64>) -> Option<bool> {
    value.map(int_to_bool)
}

pub fn bool_to_int(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

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
