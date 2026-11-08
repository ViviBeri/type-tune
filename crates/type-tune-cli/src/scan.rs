//! Scan-specific candidate filtering helpers.

use std::collections::HashSet;

use type_tune_core::model::{Candidate, ScanDiagnostic};

/// Removes candidates that cannot be rewritten and keeps their visible diagnostics.
pub(crate) fn remove_unsupported_candidates(
    mut candidates: Vec<Candidate>,
    unsupported_diagnostics: Vec<ScanDiagnostic>,
) -> (Vec<Candidate>, Vec<ScanDiagnostic>) {
    let unsupported_ids = unsupported_diagnostics
        .iter()
        .map(|diagnostic| diagnostic.candidate_id.clone())
        .collect::<HashSet<_>>();
    let diagnostics = unsupported_diagnostics
        .into_iter()
        .filter(|diagnostic| {
            candidates
                .iter()
                .any(|candidate| candidate.id == diagnostic.candidate_id)
        })
        .collect();
    candidates.retain(|candidate| !unsupported_ids.contains(&candidate.id));
    (candidates, diagnostics)
}

/// Orders explain diagnostics by their user-visible identity.
pub(crate) fn sort_diagnostics(diagnostics: &mut [ScanDiagnostic]) {
    diagnostics.sort_by(|left, right| {
        [
            left.source_path.as_str(),
            left.symbol_name.as_str(),
            left.reason.as_str(),
        ]
        .cmp(&[
            right.source_path.as_str(),
            right.symbol_name.as_str(),
            right.reason.as_str(),
        ])
    });
}

#[cfg(test)]
mod tests {
    use super::sort_diagnostics;
    use type_tune_core::model::ScanDiagnostic;

    fn diagnostic(source_path: &str, symbol_name: &str, reason: &str) -> ScanDiagnostic {
        ScanDiagnostic {
            severity: "info".into(),
            reason: reason.into(),
            message: "omitted".into(),
            candidate_id: format!("{source_path}-{symbol_name}"),
            symbol_name: symbol_name.into(),
            kind: "typeAlias".into(),
            source_path: source_path.into(),
            referrer_count: None,
            min_referrers: None,
        }
    }

    #[test]
    fn sorts_diagnostics_by_visible_identity() {
        let mut diagnostics = vec![
            diagnostic("src/b.ts", "Value", "ignored"),
            diagnostic("src/a.ts", "Zoo", "ignored"),
            diagnostic("src/a.ts", "Alpha", "unsupported"),
        ];

        sort_diagnostics(&mut diagnostics);

        assert_eq!(
            diagnostics
                .iter()
                .map(|diagnostic| (
                    diagnostic.source_path.as_str(),
                    diagnostic.symbol_name.as_str()
                ))
                .collect::<Vec<_>>(),
            vec![
                ("src/a.ts", "Alpha"),
                ("src/a.ts", "Zoo"),
                ("src/b.ts", "Value"),
            ]
        );
    }
}
