//! Length-type-value structures (Codec Specific Capabilities /
//! Configuration, Metadata, advertising data): the length octet counts the
//! type and the value (v1 ltv_find / ltv_put in le_audio_util.cpp).

/// The value of the first LTV of type `ty` in `data`; None if there is none
/// or the structure is malformed before it (zero length octets are skipped,
/// as v1 did)
pub fn find(data: &[u8], ty: u8) -> Option<&[u8]> {
    iter(data).find(|&(t, _)| t == ty).map(|(_, v)| v)
}

/// Appends one LTV (`value` at most 254 octets; a longer one is cut there)
pub fn put(out: &mut Vec<u8>, ty: u8, value: &[u8]) {
    let value = &value[..value.len().min(254)];
    out.push(value.len() as u8 + 1);
    out.push(ty);
    out.extend_from_slice(value);
}

/// The (type, value) pairs of `data` in order; zero length octets are
/// skipped (as v1 ltv_find did), a malformed entry (longer than what is
/// left) ends it
pub fn iter(data: &[u8]) -> Ltvs<'_> {
    Ltvs { rest: data }
}

/// Iterator of [`iter`]
#[derive(Debug, Clone)]
pub struct Ltvs<'a> {
    rest: &'a [u8],
}

impl<'a> Iterator for Ltvs<'a> {
    type Item = (u8, &'a [u8]);

    fn next(&mut self) -> Option<(u8, &'a [u8])> {
        loop {
            let (&len, tail) = self.rest.split_first()?;
            let len = len as usize;
            if len == 0 {
                self.rest = tail;
                continue;
            }
            if len > tail.len() {
                self.rest = &[];
                return None;
            }
            let (entry, rest) = tail.split_at(len);
            self.rest = rest;
            return Some((entry[0], &entry[1..]));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_and_put() {
        let mut v = Vec::new();
        put(&mut v, 0x01, &[0x08]);
        put(&mut v, 0x03, &[0x03, 0, 0, 0]);
        put(&mut v, 0x07, &[]);
        assert_eq!(v, [0x02, 0x01, 0x08, 0x05, 0x03, 0x03, 0, 0, 0, 0x01, 0x07]);
        assert_eq!(find(&v, 0x01), Some(&[0x08][..]));
        assert_eq!(find(&v, 0x03), Some(&[0x03, 0, 0, 0][..]));
        assert_eq!(find(&v, 0x07), Some(&[][..]));
        assert_eq!(find(&v, 0x02), None);
        let all: Vec<_> = iter(&v).map(|(t, val)| (t, val.len())).collect();
        assert_eq!(all, [(0x01, 1), (0x03, 4), (0x07, 0)]);
    }

    #[test]
    fn zero_length_and_malformed() {
        // A zero length octet is skipped (v1 ltv_find)
        let v = [0x00, 0x02, 0x05, 0x01, 0x00];
        assert_eq!(find(&v, 0x05), Some(&[0x01][..]));
        // An entry longer than the data ends the search
        let v = [0x02, 0x01, 0x08, 0x09, 0x02, 0x01];
        assert_eq!(find(&v, 0x01), Some(&[0x08][..]));
        assert_eq!(find(&v, 0x02), None);
        assert_eq!(iter(&v).count(), 1);
        assert_eq!(find(&[], 0x01), None);
    }
}
