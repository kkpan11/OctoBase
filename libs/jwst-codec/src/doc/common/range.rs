use std::{mem, ops::Range};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum OrderRange {
    Range(Range<u64>),
    Fragment(Vec<Range<u64>>),
}

impl Default for OrderRange {
    fn default() -> Self {
        Self::Range(0..0)
    }
}

impl From<Range<u64>> for OrderRange {
    fn from(range: Range<u64>) -> Self {
        Self::Range(range)
    }
}

impl From<Vec<Range<u64>>> for OrderRange {
    fn from(value: Vec<Range<u64>>) -> Self {
        Self::Fragment(value)
    }
}

fn is_continuous_range(lhs: &Range<u64>, rhs: &Range<u64>) -> bool {
    lhs.end >= rhs.start && lhs.start <= rhs.end
}

impl OrderRange {
    pub fn is_empty(&self) -> bool {
        match self {
            OrderRange::Range(range) => range.is_empty(),
            OrderRange::Fragment(vec) => vec.is_empty(),
        }
    }

    pub fn contains(&self, clock: u64) -> bool {
        match self {
            OrderRange::Range(range) => range.contains(&clock),
            OrderRange::Fragment(ranges) => ranges.iter().any(|r| r.contains(&clock)),
        }
    }

    fn check_range_covered(old_vec: &[Range<u64>], new_vec: &[Range<u64>]) -> bool {
        let mut old_iter = old_vec.iter();
        let mut next_old = old_iter.next();
        let mut new_iter = new_vec.iter().peekable();
        let mut next_new = new_iter.next();
        'new_loop: while let Some(new_range) = next_new {
            while let Some(old_range) = next_old {
                if old_range.start < new_range.start || old_range.end > new_range.end {
                    if new_iter.peek().is_some() {
                        next_new = new_iter.next();
                        continue 'new_loop;
                    } else {
                        return false;
                    }
                }
                next_old = old_iter.next();
                if let Some(next_old) = &next_old {
                    if next_old.start > new_range.end {
                        continue;
                    }
                }
            }
            next_new = new_iter.next();
        }
        true
    }

    /// diff_range returns the difference between the old range and the new
    /// range. current range must be covered by the new range
    pub fn diff_range(&self, new_range: &OrderRange) -> Vec<Range<u64>> {
        let old_vec = self.clone().into_iter().collect::<Vec<_>>();
        let new_vec = new_range.clone().into_iter().collect::<Vec<_>>();

        if !Self::check_range_covered(&old_vec, &new_vec) {
            return Vec::new();
        }

        let mut diffs = Vec::new();
        let mut old_idx = 0;

        for new_range in &new_vec {
            let mut overlap_ranges = Vec::new();
            while old_idx < old_vec.len() && old_vec[old_idx].start <= new_range.end {
                overlap_ranges.push(old_vec[old_idx].clone());
                old_idx += 1;
            }

            if overlap_ranges.is_empty() {
                diffs.push(new_range.clone());
            } else {
                let mut last_end = overlap_ranges[0].start;
                if last_end > new_range.start {
                    diffs.push(new_range.start..last_end);
                }

                for overlap in &overlap_ranges {
                    if overlap.start > last_end {
                        diffs.push(last_end..overlap.start);
                    }
                    last_end = overlap.end;
                }

                if new_range.end > last_end {
                    diffs.push(last_end..new_range.end);
                }
            }
        }

        diffs
    }

    pub fn extends<T>(&mut self, list: T)
    where
        T: Into<Vec<Range<u64>>>,
    {
        let list: Vec<_> = list.into();
        if list.is_empty() {
            return;
        }

        if self.is_empty() {
            *self = OrderRange::Fragment(list);
        } else {
            match self {
                OrderRange::Range(old) => {
                    let old = old.clone();
                    // swap and push is faster then push one by one
                    *self = OrderRange::Fragment(list);
                    self.push(old);
                }
                OrderRange::Fragment(ranges) => {
                    ranges.extend(list);
                }
            }
        }

        self.sort();
        self.squash();
    }

    /// Push new range to current one.
    /// Range will be merged if overlap exists or turned into fragment if it's
    /// not continuous.
    pub fn push(&mut self, range: Range<u64>) {
        match self {
            OrderRange::Range(r) => {
                if r.start == r.end {
                    *self = range.into();
                } else if is_continuous_range(r, &range) {
                    r.end = r.end.max(range.end);
                    r.start = r.start.min(range.start);
                } else {
                    *self = OrderRange::Fragment(if r.start < range.start {
                        vec![r.clone(), range]
                    } else {
                        vec![range, r.clone()]
                    });
                }
            }
            OrderRange::Fragment(ranges) => {
                if ranges.is_empty() {
                    *self = OrderRange::Range(range);
                } else {
                    let search_result = ranges.binary_search_by(|r| {
                        if is_continuous_range(r, &range) {
                            std::cmp::Ordering::Equal
                        } else if r.end < range.start {
                            std::cmp::Ordering::Less
                        } else {
                            std::cmp::Ordering::Greater
                        }
                    });

                    match search_result {
                        Ok(idx) => {
                            let old = &mut ranges[idx];
                            ranges[idx] = old.start.min(range.start)..old.end.max(range.end);
                            self.squash();
                        }
                        Err(idx) => ranges.insert(idx, range),
                    }
                }
            }
        }
    }

    pub fn merge(&mut self, other: Self) {
        let raw = std::mem::take(self);
        if raw.is_empty() {
            *self = other;
            return;
        }
        *self = match (raw, other) {
            (OrderRange::Range(a), OrderRange::Range(b)) => {
                if is_continuous_range(&a, &b) {
                    // merge intersected range
                    OrderRange::Range(a.start.min(b.start)..a.end.max(b.end))
                } else {
                    OrderRange::Fragment(vec![a, b])
                }
            }
            (OrderRange::Fragment(mut a), OrderRange::Range(b)) => {
                a.push(b);
                OrderRange::Fragment(a)
            }
            (OrderRange::Range(a), OrderRange::Fragment(b)) => {
                let mut v = b;
                v.push(a);
                OrderRange::Fragment(v)
            }
            (OrderRange::Fragment(mut a), OrderRange::Fragment(mut b)) => {
                a.append(&mut b);
                OrderRange::Fragment(a)
            }
        };

        self.sort();
        self.squash()
    }

    /// Merge all available ranges list into one.
    fn squash(&mut self) {
        // merge all available ranges
        if let OrderRange::Fragment(ranges) = self {
            if ranges.is_empty() {
                *self = OrderRange::Range(0..0);
                return;
            }

            let mut cur_idx = 0;
            let mut next_idx = 1;
            while next_idx < ranges.len() {
                let cur = &ranges[cur_idx];
                let next = &ranges[next_idx];
                if is_continuous_range(cur, next) {
                    ranges[cur_idx] = cur.start.min(next.start)..cur.end.max(next.end);
                    ranges[next_idx] = 0..0;
                } else {
                    cur_idx = next_idx;
                }

                next_idx += 1;
            }

            ranges.retain(|r| !r.is_empty());
            if ranges.len() == 1 {
                *self = OrderRange::Range(ranges[0].clone());
            }
        }
    }

    fn sort(&mut self) {
        if let OrderRange::Fragment(ranges) = self {
            ranges.sort_by(|lhs, rhs| lhs.start.cmp(&rhs.start));
        }
    }

    pub fn pop(&mut self) -> Option<Range<u64>> {
        if self.is_empty() {
            None
        } else {
            match self {
                OrderRange::Range(range) => Some(mem::replace(range, 0..0)),
                OrderRange::Fragment(list) => Some(list.remove(0)),
            }
        }
    }
}

impl<'a> IntoIterator for &'a OrderRange {
    type Item = Range<u64>;
    type IntoIter = OrderRangeIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        OrderRangeIter { range: self, idx: 0 }
    }
}

pub struct OrderRangeIter<'a> {
    range: &'a OrderRange,
    idx: usize,
}

impl<'a> Iterator for OrderRangeIter<'a> {
    type Item = Range<u64>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.range {
            OrderRange::Range(range) => {
                if self.idx == 0 {
                    self.idx += 1;
                    Some(range.clone())
                } else {
                    None
                }
            }
            OrderRange::Fragment(ranges) => {
                if self.idx < ranges.len() {
                    let range = ranges[self.idx].clone();
                    self.idx += 1;
                    Some(range)
                } else {
                    None
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::OrderRange;
    #[test]
    fn test_range_push() {
        let mut range: OrderRange = (0..10).into();

        range.push(5..15);
        assert_eq!(range, OrderRange::Range(0..15));

        // turn to fragment
        range.push(20..30);
        assert_eq!(range, OrderRange::Fragment(vec![(0..15), (20..30)]));

        // auto merge
        range.push(15..16);
        assert_eq!(range, OrderRange::Fragment(vec![(0..16), (20..30)]));

        // squash
        range.push(16..20);
        assert_eq!(range, OrderRange::Range(0..30));
    }

    #[test]
    fn test_range_pop() {
        let mut range: OrderRange = vec![(0..10), (20..30)].into();
        assert_eq!(range.pop(), Some(0..10));

        let mut range: OrderRange = (0..10).into();
        assert_eq!(range.pop(), Some(0..10));
        assert!(range.is_empty());
        assert_eq!(range.pop(), None);
    }

    #[test]
    fn test_ranges_squash() {
        let mut range = OrderRange::Fragment(vec![(0..10), (20..30)]);

        // do nothing
        range.squash();
        assert_eq!(range, OrderRange::Fragment(vec![(0..10), (20..30)]));

        // merged into list
        range = OrderRange::Fragment(vec![(0..10), (10..20), (30..40)]);
        range.squash();
        assert_eq!(range, OrderRange::Fragment(vec![(0..20), (30..40)]));

        // turn to range
        range = OrderRange::Fragment(vec![(0..10), (10..20), (20..30)]);
        range.squash();
        assert_eq!(range, OrderRange::Range(0..30));
    }

    #[test]
    fn test_range_sort() {
        let mut range: OrderRange = vec![(20..30), (0..10), (10..50)].into();
        range.sort();
        assert_eq!(range, OrderRange::Fragment(vec![(0..10), (10..50), (20..30)]));
    }

    #[test]
    fn test_range_covered() {
        assert_eq!(OrderRange::check_range_covered(&[0..1], &[2..3]), false);
        assert_eq!(OrderRange::check_range_covered(&[0..1], &[0..3]), true);
        assert_eq!(OrderRange::check_range_covered(&[0..1], &[1..3]), false);
        assert_eq!(OrderRange::check_range_covered(&[0..1], &[0..3]), true);
        assert_eq!(OrderRange::check_range_covered(&[1..2], &[0..3]), true);
        assert_eq!(OrderRange::check_range_covered(&[1..2, 2..3], &[0..3]), true);
        assert_eq!(OrderRange::check_range_covered(&[1..2, 2..3, 3..4], &[0..3]), false);
        assert_eq!(OrderRange::check_range_covered(&[0..1, 2..3], &[0..2, 2..4]), true);
        assert_eq!(
            OrderRange::check_range_covered(&[0..1, 2..3, 3..4], &[0..2, 2..4]),
            true
        );
    }

    #[test]
    fn test_range_diff() {
        {
            let old = OrderRange::Range(0..1);
            let new = OrderRange::Range(2..3);
            let ranges = old.diff_range(&new);
            assert_eq!(ranges, vec![]);
        }

        {
            let old = OrderRange::Range(0..10);
            let new = OrderRange::Range(0..11);
            let ranges = old.diff_range(&new);
            assert_eq!(ranges, vec![(10..11)]);
        }

        {
            let old: OrderRange = vec![(0..10), (20..30)].into();
            let new: OrderRange = vec![(0..15), (20..30)].into();
            let ranges = old.diff_range(&new);
            assert_eq!(ranges, vec![(10..15)]);
        }

        {
            let old: OrderRange = vec![(0..3), (5..7), (8..10), (16..18), (21..23)].into();
            let new: OrderRange = vec![(0..12), (15..23)].into();
            let ranges = old.diff_range(&new);
            assert_eq!(ranges, vec![(3..5), (7..8), (10..12), (15..16), (18..21)]);
        }

        {
            let old: OrderRange = vec![(1..6), (8..12)].into();
            let new: OrderRange = vec![(0..12), (15..23), (24..28)].into();
            let ranges = old.diff_range(&new);
            assert_eq!(ranges, vec![(0..1), (6..8), (15..23), (24..28)]);
        }
    }

    #[test]
    fn test_range_merge() {
        let mut range: OrderRange = (0..10).into();
        range.merge((20..30).into());
        assert_eq!(range, OrderRange::Fragment(vec![(0..10), (20..30)]));

        let mut range: OrderRange = (0..10).into();
        range.merge(vec![(10..15), (20..30)].into());
        assert_eq!(range, OrderRange::Fragment(vec![(0..15), (20..30)]));

        let mut range: OrderRange = vec![(0..10), (20..30)].into();
        range.merge((10..20).into());
        assert_eq!(range, OrderRange::Range(0..30));

        let mut range: OrderRange = vec![(0..10), (20..30)].into();
        range.merge(vec![(10..20), (30..40)].into());
        assert_eq!(range, OrderRange::Range(0..40));
    }

    #[test]
    fn iter() {
        let range: OrderRange = vec![(0..10), (20..30)].into();

        assert_eq!(range.into_iter().collect::<Vec<_>>(), vec![(0..10), (20..30)]);

        let range: OrderRange = OrderRange::Range(0..10);

        assert_eq!(range.into_iter().collect::<Vec<_>>(), vec![(0..10)]);
    }
}
