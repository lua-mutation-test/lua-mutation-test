//! Shard-aware partitioning for CI matrix fan-out.
//!
//! `lmut run --shard k/n` partitions the generated mutant inventory
//! deterministically by mutant count — not by file — so shards stay balanced
//! regardless of file layout. Partitioning sorts by a stable mutant identity
//! (file + location + operator) and then strides, so retries and the
//! incremental cache stay coherent across runs and machines.

use crate::mutant::Mutant;
use crate::result::MutantResult;
use std::cmp::Ordering;

/// A single shard selection `k/n` (1-based index, total count).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shard {
    /// 1-based shard index.
    pub index: usize,
    /// Total number of shards.
    pub total: usize,
}

impl Shard {
    /// Parses a `--shard` value of the form `<k>/<n>` (e.g. `2/5`).
    pub fn parse(s: &str) -> Result<Self, String> {
        let trimmed = s.trim();
        let (k_str, n_str) = trimmed
            .split_once('/')
            .ok_or_else(|| format!("invalid --shard '{s}': expected format <k>/<n>, e.g. --shard 2/5"))?;
        let k_str = k_str.trim();
        let n_str = n_str.trim();
        let k: usize = k_str.parse().map_err(|_| {
            format!("invalid --shard '{s}': index k ('{k_str}') is not a positive integer")
        })?;
        let n: usize = n_str.parse().map_err(|_| {
            format!("invalid --shard '{s}': total n ('{n_str}') is not a positive integer")
        })?;
        if n == 0 {
            return Err(format!("invalid --shard '{s}': total n must be >= 1"));
        }
        if k == 0 || k > n {
            return Err(format!(
                "invalid --shard '{s}': index k must be between 1 and n ({n})"
            ));
        }
        Ok(Self { index: k, total: n })
    }

    /// Returns true when the 0-based position in the stably-sorted inventory
    /// belongs to this shard (stride selection).
    pub fn contains(&self, position: usize) -> bool {
        position % self.total == self.index - 1
    }
}

/// Stable ordering for mutants: file + byte location + operator (+ replacement
/// and id as final tiebreakers so the order is total).
///
/// All mutants in the same group share file/operator/location in the
/// difficulty-cap path, where replacement is the only discriminator — hence it
/// must participate in the key. The `id` tiebreaker guards against pathological
/// duplicates surviving dedup across different files with identical bytes.
pub fn stable_cmp(a: &Mutant, b: &Mutant) -> Ordering {
    a.file
        .cmp(&b.file)
        .then_with(|| a.start_byte.cmp(&b.start_byte))
        .then_with(|| a.end_byte.cmp(&b.end_byte))
        .then_with(|| a.line.cmp(&b.line))
        .then_with(|| a.column.cmp(&b.column))
        .then_with(|| a.operator.cmp(&b.operator))
        .then_with(|| a.replacement.cmp(&b.replacement))
        .then_with(|| a.id.cmp(&b.id))
}

/// Sorts executable mutant jobs into the stable shard order (in place).
pub fn sort_jobs(jobs: &mut [(Mutant, String)]) {
    jobs.sort_by(|a, b| stable_cmp(&a.0, &b.0));
}

/// Sorts mutants into the stable shard order (in place).
pub fn sort_mutants(mutants: &mut [Mutant]) {
    mutants.sort_by(stable_cmp);
}

fn mutant_of_result(result: &MutantResult) -> &Mutant {
    result.mutant()
}

/// Sorts `MutantResult`s (e.g. equivalents) into the stable shard order.
pub fn sort_results(results: &mut [MutantResult]) {
    results.sort_by(|a, b| stable_cmp(mutant_of_result(a), mutant_of_result(b)));
}

/// Selects the stride subset belonging to `shard` from a stably-sorted list.
///
/// Callers must sort first with [`sort_jobs`]/[`sort_mutants`]/[`sort_results`]
/// (or use the `select_*` helpers below which sort internally). Position `i`
/// belongs to shard `k/n` when `i % n == k - 1`, so the union of all shards
/// covers the inventory exactly once with sizes differing by at most one.
pub fn stride_select<T>(items: Vec<T>, shard: &Shard) -> Vec<T> {
    items
        .into_iter()
        .enumerate()
        .filter(|(i, _)| shard.contains(*i))
        .map(|(_, item)| item)
        .collect()
}

/// Sorts executable jobs stably and returns only this shard's subset.
pub fn select_shard_jobs(
    mut jobs: Vec<(Mutant, String)>,
    shard: &Shard,
) -> (Vec<(Mutant, String)>, usize) {
    sort_jobs(&mut jobs);
    let total = jobs.len();
    (stride_select(jobs, shard), total)
}

/// Sorts equivalent (or mixed) results stably and returns this shard's subset.
pub fn select_shard_results(
    mut results: Vec<MutantResult>,
    shard: &Shard,
) -> (Vec<MutantResult>, usize) {
    sort_results(&mut results);
    let total = results.len();
    (stride_select(results, shard), total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn mutant(file: &str, line: usize, operator: &str, replacement: &str) -> Mutant {
        Mutant {
            id: format!("{file}-{line}-{operator}-{replacement}"),
            operator: operator.to_string(),
            file: PathBuf::from(file),
            start_byte: line * 100,
            end_byte: line * 100 + 1,
            line,
            column: 1,
            original: "x".to_string(),
            replacement: replacement.to_string(),
            equivalent_reason: None,
        }
    }

    #[test]
    fn parses_valid_shard() {
        let shard = Shard::parse("2/5").unwrap();
        assert_eq!(shard, Shard { index: 2, total: 5 });
    }

    #[test]
    fn parses_shard_with_whitespace() {
        let shard = Shard::parse(" 1 / 3 ").unwrap();
        assert_eq!(shard, Shard { index: 1, total: 3 });
    }

    #[test]
    fn rejects_missing_slash() {
        assert!(Shard::parse("2").is_err());
    }

    #[test]
    fn rejects_zero_index() {
        assert!(Shard::parse("0/3").is_err());
    }

    #[test]
    fn rejects_zero_total() {
        assert!(Shard::parse("1/0").is_err());
    }

    #[test]
    fn rejects_index_above_total() {
        assert!(Shard::parse("4/3").is_err());
    }

    #[test]
    fn rejects_non_numeric() {
        assert!(Shard::parse("a/b").is_err());
        assert!(Shard::parse("1/x").is_err());
    }

    #[test]
    fn shards_cover_inventory_exactly_once_and_balanced() {
        let mutants: Vec<Mutant> = (0..10)
            .map(|i| mutant("b.lua", i, "arithmetic_operator", &format!("r{i}")))
            .collect();

        let mut seen = std::collections::HashSet::new();
        let mut sizes = Vec::new();
        for k in 1..=3 {
            let shard = Shard { index: k, total: 3 };
            let mut jobs: Vec<(Mutant, String)> =
                mutants.iter().cloned().map(|m| (m, String::new())).collect();
            // Reverse input to prove the selection sorts internally.
            jobs.reverse();
            let (selected, total) = select_shard_jobs(jobs, &shard);
            assert_eq!(total, 10);
            sizes.push(selected.len());
            for (m, _) in selected {
                assert!(seen.insert(m.id.clone()), "duplicate mutant {}", m.id);
            }
        }
        assert_eq!(seen.len(), 10);
        // 10 mutants over 3 shards -> sizes 4/3/3 in some order, max-min <= 1.
        sizes.sort_unstable();
        assert_eq!(sizes, vec![3, 3, 4]);
    }

    #[test]
    fn sharding_is_stable_regardless_of_input_order() {
        let mutants: Vec<Mutant> = (0..20)
            .map(|i| {
                mutant(
                    if i % 2 == 0 { "a.lua" } else { "b.lua" },
                    i,
                    "relational_operator",
                    &format!("r{i}"),
                )
            })
            .collect();
        let shard = Shard { index: 2, total: 5 };

        let forward: Vec<String> = {
            let jobs: Vec<(Mutant, String)> =
                mutants.iter().cloned().map(|m| (m, String::new())).collect();
            select_shard_jobs(jobs, &shard)
                .0
                .into_iter()
                .map(|(m, _)| m.id)
                .collect()
        };
        let mut reversed_jobs: Vec<(Mutant, String)> =
            mutants.iter().cloned().map(|m| (m, String::new())).collect();
        reversed_jobs.reverse();
        let backward: Vec<String> = select_shard_jobs(reversed_jobs, &shard)
            .0
            .into_iter()
            .map(|(m, _)| m.id)
            .collect();
        assert_eq!(forward, backward);
    }

    #[test]
    fn single_shard_selects_everything() {
        let jobs: Vec<(Mutant, String)> = (0..5)
            .map(|i| (mutant("a.lua", i, "op", "r"), String::new()))
            .collect();
        let shard = Shard { index: 1, total: 1 };
        let (selected, total) = select_shard_jobs(jobs, &shard);
        assert_eq!(total, 5);
        assert_eq!(selected.len(), 5);
    }

    #[test]
    fn stride_interleaves_rather_than_chunks() {
        // Sorted order is a.lua:0, a.lua:1, b.lua:0, b.lua:1 (file-major).
        // Stride 1/2 picks positions 0,2; contiguous chunking would pick 0,1.
        let jobs: Vec<(Mutant, String)> = vec![
            (mutant("b.lua", 1, "op", "r"), String::new()),
            (mutant("a.lua", 1, "op", "r"), String::new()),
            (mutant("b.lua", 0, "op", "r"), String::new()),
            (mutant("a.lua", 0, "op", "r"), String::new()),
        ];
        let shard = Shard { index: 1, total: 2 };
        let (selected, _) = select_shard_jobs(jobs, &shard);
        let files: Vec<String> = selected
            .iter()
            .map(|(m, _)| m.file.to_string_lossy().to_string())
            .collect();
        assert_eq!(files, vec!["a.lua".to_string(), "b.lua".to_string()]);
    }
}
