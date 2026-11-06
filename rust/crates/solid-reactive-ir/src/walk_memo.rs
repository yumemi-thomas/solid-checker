//! A search over callers whose answer is a pure function of where it stands
//! and how much depth it has left. Without this, a function many sites reach
//! is asked once per path, `sites^depth` in all. Keyed on the remaining depth
//! as well, so a memo never answers for a deeper or a shallower ask: the
//! answers are exactly the unmemoized ones, and only the repeats go.

use std::hash::Hash;

use rustc_hash::FxHashMap as HashMap;

/// The answer at `key`, computed once. `compute` gets the memo back, for the
/// search's own recursion.
pub(crate) fn memoized<K: Clone + Eq + Hash, V: Clone>(
    memo: &mut HashMap<K, V>,
    key: K,
    compute: impl FnOnce(&mut HashMap<K, V>) -> V,
) -> V {
    if let Some(answer) = memo.get(&key) {
        return answer.clone();
    }
    let answer = compute(memo);
    memo.insert(key, answer.clone());
    answer
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ladder of diamonds: every rung reaches the next one by two sites.
    /// `open` is the rung that answers true; the depth running out answers
    /// false, as the callers' bounds do. Asked with no memo.
    fn plain(rung: u32, depth: u32, open: u32, runs: &mut u32) -> bool {
        *runs += 1;
        if rung == open {
            return true;
        }
        let Some(depth) = depth.checked_sub(1) else {
            return false;
        };
        plain(rung + 1, depth, open, runs) && plain(rung + 1, depth, open, runs)
    }

    /// The same search through [`memoized`], keyed as the callers key theirs.
    fn kept(
        rung: u32,
        depth: u32,
        open: u32,
        runs: &mut u32,
        memo: &mut HashMap<(u32, u32), bool>,
    ) -> bool {
        memoized(memo, (rung, depth), |memo| {
            *runs += 1;
            if rung == open {
                return true;
            }
            let Some(depth) = depth.checked_sub(1) else {
                return false;
            };
            kept(rung + 1, depth, open, runs, memo) && kept(rung + 1, depth, open, runs, memo)
        })
    }

    #[test]
    fn a_memo_answers_as_the_plain_search_does_at_every_depth() {
        for depth in 0..12 {
            for open in 0..14 {
                let (mut plain_runs, mut kept_runs) = (0, 0);
                let answer = plain(0, depth, open, &mut plain_runs);
                let memo = &mut HashMap::default();
                assert_eq!(
                    answer,
                    kept(0, depth, open, &mut kept_runs, memo),
                    "depth {depth}, open {open}"
                );
                assert!(kept_runs <= plain_runs);
            }
        }
    }

    #[test]
    fn a_repeated_ask_is_not_run_again() {
        let mut memo = HashMap::default();
        let mut runs = 0;
        for _ in 0..3 {
            let answer = memoized(&mut memo, ("a.tsx", 4), |_| {
                runs += 1;
                true
            });
            assert!(answer);
        }
        assert_eq!(runs, 1);
    }

    #[test]
    fn another_depth_is_another_ask() {
        // An ask with less depth left may answer otherwise, so a memo keyed
        // without it would change the answer.
        let mut memo = HashMap::default();
        assert!(memoized(&mut memo, ("a.tsx", 2), |_| true));
        assert!(!memoized(&mut memo, ("a.tsx", 1), |_| false));
        assert!(memoized(&mut memo, ("a.tsx", 2), |_| false));
    }

    #[test]
    fn a_ladder_of_diamonds_costs_its_rungs_not_its_paths() {
        let (mut runs, memo) = (0, &mut HashMap::default());
        // 2^40 paths, one run per rung.
        assert!(kept(0, 40, 40, &mut runs, memo));
        assert_eq!(runs, 41);
    }
}
