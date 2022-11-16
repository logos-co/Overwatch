use crate::services::ServiceId;

pub const fn unique_ids(to_check: &[ServiceId]) -> bool {
    if to_check.is_empty() {
        return true;
    }
    let mut i: usize = 0;
    let mut j: usize = 1;
    while i < to_check.len() - 1 {
        if const_str::equal!(to_check[i], to_check[j]) {
            return false;
        }
        j += 1;
        if j >= to_check.len() {
            i += 1;
            j = i + 1;
        }
    }
    true
}

#[cfg(test)]
mod test {
    use crate::utils::const_checks::unique_ids;

    #[test]
    fn test_unique_ids() {
        // this shouldn't even compile if checks fails
        const _: () = assert!(unique_ids(&["A", "B"]));
        const _: () = assert!(!unique_ids(&["A", "A"]));

        const _: () = assert!(unique_ids(&[]));
        const _: () = assert!(unique_ids(&["A"]));
    }
}
