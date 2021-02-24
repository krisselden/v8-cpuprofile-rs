use serde::ser::{Serialize, Serializer};

/// Turns a `fn() -> Iterator` into an `IntoIterator`
#[derive(Debug)]
pub(crate) struct MakeIter<F>(F);

impl<I, F, T> From<F> for MakeIter<F>
where
    I: Iterator<Item = T>,
    F: Fn() -> I,
{
    fn from(make_iter: F) -> Self {
        Self(make_iter)
    }
}

// just a function pointer so we can copy
impl<F: Copy> Copy for MakeIter<F> {}
impl<F: Copy> Clone for MakeIter<F> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<I, F, T> IntoIterator for MakeIter<F>
where
    I: Iterator<Item = T>,
    F: Fn() -> I,
{
    type Item = I::Item;
    type IntoIter = I;

    fn into_iter(self) -> Self::IntoIter {
        (self.0)()
    }
}

impl<F> Serialize for MakeIter<F>
where
    Self: IntoIterator + Copy,
    <Self as IntoIterator>::Item: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_seq(*self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn make_iter() {
        let strings = &["one", "two", "three", "four"];
        let make_iter = MakeIter::from(|| strings.iter().filter(|s| s.starts_with('t')));
        let mut vec = Vec::new();
        for s in make_iter {
            vec.push(*s);
        }
        assert_eq!(vec, ["two", "three"]);

        assert_eq!(
            serde_json::to_string(&make_iter).unwrap(),
            r#"["two","three"]"#
        );

        let nums = [1, 1, 2, 3, 5, 8, 13];

        let option_nums = Some(&nums[..]);

        let make_iter: MakeIter<_> = (|| {
            let mut last = 0;
            let slice = option_nums.unwrap_or(&[]);
            slice.iter().map(move |v| {
                let delta = v - last;
                last = *v;
                delta
            })
        })
        .into();

        let mut deltas = Vec::with_capacity(nums.len());

        for delta in make_iter {
            deltas.push(delta);
        }

        assert_eq!(deltas, [1, 0, 1, 1, 2, 3, 5]);

        let vec: Vec<i32> = make_iter.into_iter().collect();

        assert_eq!(vec, [1, 0, 1, 1, 2, 3, 5])
    }
}
