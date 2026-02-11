#![allow(dead_code)]

/// Find first zero bit in `[start_bit, max_bit)`.
///
/// Uses byte-level skipping for full bytes (`0xFF`) and then bit probing.
pub fn find_first_zero(bitmap: &[u8], start_bit: usize, max_bit: usize) -> Option<usize> {
    if start_bit >= max_bit {
        return None;
    }

    let mut bit = start_bit;
    while bit < max_bit {
        let byte_idx = bit / 8;
        let bit_in_byte = bit % 8;

        if byte_idx >= bitmap.len() {
            return None;
        }

        let byte = bitmap[byte_idx];
        if bit_in_byte == 0 && byte == 0xFF {
            bit += 8;
            continue;
        }

        for local_bit in bit_in_byte..8 {
            let candidate = byte_idx * 8 + local_bit;
            if candidate >= max_bit {
                return None;
            }
            if (byte & (1 << local_bit)) == 0 {
                return Some(candidate);
            }
        }

        bit = (byte_idx + 1) * 8;
    }

    None
}

/// Find a run of `count` continuous zero bits in `[start_bit, max_bit)`.
pub fn find_zero_run(
    bitmap: &[u8],
    start_bit: usize,
    max_bit: usize,
    count: usize,
) -> Option<usize> {
    if count == 0 {
        return Some(start_bit.min(max_bit));
    }
    if start_bit >= max_bit {
        return None;
    }

    let mut run_start = find_first_zero(bitmap, start_bit, max_bit)?;
    let mut run_len = 0usize;
    let mut bit = run_start;

    while bit < max_bit {
        if !test_bit(bitmap, bit) {
            if run_len == 0 {
                run_start = bit;
            }
            run_len += 1;
            if run_len >= count {
                return Some(run_start);
            }
        } else {
            run_len = 0;
        }
        bit += 1;
    }

    None
}

/// Set one bit to 1.
pub fn set_bit(bitmap: &mut [u8], bit: usize) {
    let byte_idx = bit / 8;
    let bit_in_byte = bit % 8;
    bitmap[byte_idx] |= 1 << bit_in_byte;
}

/// Clear one bit to 0.
pub fn clear_bit(bitmap: &mut [u8], bit: usize) {
    let byte_idx = bit / 8;
    let bit_in_byte = bit % 8;
    bitmap[byte_idx] &= !(1 << bit_in_byte);
}

/// Test whether the bit is 1.
pub fn test_bit(bitmap: &[u8], bit: usize) -> bool {
    let byte_idx = bit / 8;
    let bit_in_byte = bit % 8;
    (bitmap[byte_idx] & (1 << bit_in_byte)) != 0
}

/// Count zero bits in `[0, max_bit)`.
pub fn count_zeros(bitmap: &[u8], max_bit: usize) -> usize {
    let mut zeros = 0usize;
    let mut bit = 0usize;
    while bit < max_bit {
        if !test_bit(bitmap, bit) {
            zeros += 1;
        }
        bit += 1;
    }
    zeros
}

#[cfg(test)]
mod tests {
    use super::{clear_bit, count_zeros, find_first_zero, find_zero_run, set_bit, test_bit};

    #[test]
    fn test_find_first_zero_with_byte_skip() {
        let bitmap = [0xFFu8, 0b1110_1111];
        assert_eq!(find_first_zero(&bitmap, 0, 16), Some(12));
    }

    #[test]
    fn test_find_zero_run() {
        let bitmap = [0b0000_0001u8, 0b1111_1111];
        assert_eq!(find_zero_run(&bitmap, 0, 16, 3), Some(1));
        assert_eq!(find_zero_run(&bitmap, 2, 16, 4), Some(2));
        assert_eq!(find_zero_run(&bitmap, 8, 16, 1), None);
    }

    #[test]
    fn test_set_clear_test_and_count_zeros() {
        let mut bitmap = [0u8; 2];
        assert_eq!(count_zeros(&bitmap, 16), 16);

        set_bit(&mut bitmap, 0);
        set_bit(&mut bitmap, 9);
        assert!(test_bit(&bitmap, 0));
        assert!(test_bit(&bitmap, 9));
        assert_eq!(count_zeros(&bitmap, 16), 14);

        clear_bit(&mut bitmap, 0);
        assert!(!test_bit(&bitmap, 0));
        assert_eq!(count_zeros(&bitmap, 16), 15);
    }
}
