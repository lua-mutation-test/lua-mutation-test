//! Mutation score calculation and breakdowns.

use crate::result::MutantResult;
use std::collections::HashMap;

/// Category for a single mutant result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    Killed,
    Survived,
    TimedOut,
    Error,
    Skipped,
    Equivalent,
}

/// Counts and percentage for a set of mutant results.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MutationScore {
    pub killed: usize,
    pub survived: usize,
    pub timed_out: usize,
    pub error: usize,
    pub skipped: usize,
    pub equivalent: usize,
}

impl MutationScore {
    /// Total number of mutants represented.
    pub fn total(&self) -> usize {
        self.killed + self.survived + self.timed_out + self.error + self.skipped + self.equivalent
    }

    /// Number of mutants counted in the score denominator.
    ///
    /// By default, errors, timeouts, and likely-equivalent mutants are excluded.
    pub fn denominator(&self) -> usize {
        self.killed + self.survived + self.skipped
    }

    /// Overall mutation score as a fraction between 0.0 and 1.0.
    ///
    /// Returns `None` when there are no countable mutants.
    pub fn score(&self) -> Option<f64> {
        let denom = self.denominator();
        if denom == 0 {
            None
        } else {
            Some(self.killed as f64 / denom as f64)
        }
    }

    /// Overall mutation score as a percentage.
    pub fn percentage(&self) -> Option<f64> {
        self.score().map(|s| s * 100.0)
    }

    /// Adds a single categorized result.
    pub fn add(&mut self, category: Category) {
        match category {
            Category::Killed => self.killed += 1,
            Category::Survived => self.survived += 1,
            Category::TimedOut => self.timed_out += 1,
            Category::Error => self.error += 1,
            Category::Skipped => self.skipped += 1,
            Category::Equivalent => self.equivalent += 1,
        }
    }
}

/// Categorizes a `MutantResult`.
pub fn categorize(result: &MutantResult) -> Category {
    match result {
        MutantResult::Killed { .. } => Category::Killed,
        MutantResult::Survived { .. } => Category::Survived,
        MutantResult::TimedOut { .. } => Category::TimedOut,
        MutantResult::Error { .. } => Category::Error,
        MutantResult::Equivalent { .. } => Category::Equivalent,
    }
}

/// Score breakdowns grouped by file, operator, and function.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ScoreBreakdown {
    pub overall: MutationScore,
    pub by_file: HashMap<std::path::PathBuf, MutationScore>,
    pub by_operator: HashMap<String, MutationScore>,
    pub by_function: HashMap<String, MutationScore>,
}

/// Aggregates a list of mutant results into overall and breakdown scores.
pub fn score_results(results: &[MutantResult]) -> ScoreBreakdown {
    let mut breakdown = ScoreBreakdown::default();

    for result in results {
        let category = categorize(result);
        let mutant = result.mutant();
        breakdown.overall.add(category);
        breakdown
            .by_file
            .entry(mutant.file.clone())
            .or_default()
            .add(category);
        breakdown
            .by_operator
            .entry(mutant.operator.clone())
            .or_default()
            .add(category);
        // Function metadata is not yet available; group all under a fallback key.
        breakdown
            .by_function
            .entry("<global>".to_string())
            .or_default()
            .add(category);
    }

    breakdown
}

/// Computes a score for a single mutant result list without breakdowns.
pub fn score(results: &[MutantResult]) -> MutationScore {
    let mut score = MutationScore::default();
    for result in results {
        score.add(categorize(result));
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mutant::{CandidateMutant, Mutant};
    use std::path::PathBuf;

    fn dummy_result(category: Category) -> MutantResult {
        let mutant = Mutant::from_candidate(
            CandidateMutant {
                start_byte: 0,
                end_byte: 0,
                replacement: String::new(),
            },
            "dummy",
            PathBuf::from("src/foo.lua"),
            "local x = 1",
        );
        match category {
            Category::Killed => MutantResult::Killed {
                mutant,
                duration_ms: 0,
                stdout_snippet: String::new(),
                stderr_snippet: String::new(),
            },
            Category::Survived => MutantResult::Survived {
                mutant,
                duration_ms: 0,
                stdout_snippet: String::new(),
                stderr_snippet: String::new(),
            },
            Category::TimedOut => MutantResult::TimedOut {
                mutant,
                duration_ms: 0,
                stdout_snippet: String::new(),
                stderr_snippet: String::new(),
            },
            Category::Error => MutantResult::Error {
                mutant,
                duration_ms: 0,
                reason: String::new(),
                stdout_snippet: String::new(),
                stderr_snippet: String::new(),
            },
            Category::Skipped => MutantResult::Survived {
                mutant,
                duration_ms: 0,
                stdout_snippet: String::new(),
                stderr_snippet: String::new(),
            },
            Category::Equivalent => MutantResult::Equivalent {
                mutant,
                reason: String::new(),
            },
        }
    }

    #[test]
    fn calculates_overall_percentage() {
        let results = vec![
            dummy_result(Category::Killed),
            dummy_result(Category::Killed),
            dummy_result(Category::Survived),
        ];
        let score = score(&results);
        assert_eq!(score.percentage(), Some(2.0 / 3.0 * 100.0));
    }

    #[test]
    fn excludes_errors_and_timeouts_from_denominator() {
        let mut results = vec![
            dummy_result(Category::Killed),
            dummy_result(Category::Survived),
            dummy_result(Category::TimedOut),
            dummy_result(Category::Error),
        ];
        results.push(dummy_result(Category::Killed));
        let score = score(&results);
        assert_eq!(score.killed, 2);
        assert_eq!(score.denominator(), 3);
        assert_eq!(score.percentage(), Some(2.0 / 3.0 * 100.0));
    }

    #[test]
    fn aggregates_by_file_and_operator() {
        let results = vec![
            dummy_result(Category::Killed),
            dummy_result(Category::Survived),
        ];
        let breakdown = score_results(&results);
        assert_eq!(breakdown.overall.total(), 2);
        assert!(breakdown
            .by_file
            .contains_key(&PathBuf::from("src/foo.lua")));
        assert!(breakdown.by_operator.contains_key("dummy"));
    }

    #[test]
    fn empty_results_have_no_score() {
        let score = MutationScore::default();
        assert_eq!(score.score(), None);
        assert_eq!(score.percentage(), None);
    }

    #[test]
    fn total_counts_every_bucket() {
        let mut score = MutationScore::default();
        for _ in 0..1 {
            score.add(Category::Killed);
        }
        for _ in 0..2 {
            score.add(Category::Survived);
        }
        for _ in 0..3 {
            score.add(Category::TimedOut);
        }
        for _ in 0..4 {
            score.add(Category::Error);
        }
        for _ in 0..5 {
            score.add(Category::Skipped);
        }
        for _ in 0..6 {
            score.add(Category::Equivalent);
        }
        assert_eq!(score.total(), 21);
    }

    #[test]
    fn denominator_excludes_timeouts_errors_equivalent() {
        let mut score = MutationScore::default();
        for _ in 0..2 {
            score.add(Category::Killed);
        }
        score.add(Category::Survived);
        score.add(Category::Skipped);
        score.add(Category::TimedOut);
        score.add(Category::Error);
        score.add(Category::Equivalent);
        assert_eq!(score.denominator(), 4);
        assert_eq!(score.percentage(), Some(50.0));
    }

    #[test]
    fn add_routes_each_category() {
        let mut score = MutationScore::default();
        score.add(Category::Killed);
        score.add(Category::Survived);
        score.add(Category::TimedOut);
        score.add(Category::Error);
        score.add(Category::Skipped);
        score.add(Category::Equivalent);
        assert_eq!(score.killed, 1);
        assert_eq!(score.survived, 1);
        assert_eq!(score.timed_out, 1);
        assert_eq!(score.error, 1);
        assert_eq!(score.skipped, 1);
        assert_eq!(score.equivalent, 1);
    }
}
