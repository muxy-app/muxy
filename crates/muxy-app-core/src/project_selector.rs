//! Finds the one project a selector typed by the user refers to.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mismatch {
    NotFound,
    Ambiguous,
}

/// The one candidate `exact` accepts or, when it accepts none, the one `loose`
/// accepts.
pub fn pick<T>(
    candidates: &[T],
    exact: impl Fn(&T) -> bool,
    loose: impl Fn(&T) -> bool,
) -> Result<&T, Mismatch> {
    match only(candidates, exact) {
        Err(Mismatch::NotFound) => only(candidates, loose),
        found => found,
    }
}

fn only<T>(candidates: &[T], accepts: impl Fn(&T) -> bool) -> Result<&T, Mismatch> {
    let mut matches = candidates.iter().filter(|candidate| accepts(candidate));
    let first = matches.next().ok_or(Mismatch::NotFound)?;
    if matches.next().is_some() {
        return Err(Mismatch::Ambiguous);
    }
    Ok(first)
}

pub fn same_name(name: &str, selector: &str) -> bool {
    name.to_lowercase() == selector.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_names_win_over_ones_that_differ_in_case() {
        let cases: &[(&[&str], &str, Result<&str, Mismatch>)] = &[
            (&["App"], "app", Ok("App")),
            (&["ÉTÉ"], "été", Ok("ÉTÉ")),
            (&["App", "app"], "app", Ok("app")),
            (&["App", "app"], "App", Ok("App")),
            (&["App", "app"], "APP", Err(Mismatch::Ambiguous)),
            (&["app", "app"], "app", Err(Mismatch::Ambiguous)),
            (&["app", "app", "App"], "app", Err(Mismatch::Ambiguous)),
            (&["web"], "app", Err(Mismatch::NotFound)),
            (&[], "app", Err(Mismatch::NotFound)),
        ];
        for (names, selector, expected) in cases {
            let found = pick(
                names,
                |name| name == selector,
                |name| same_name(name, selector),
            );
            assert_eq!(found.copied(), *expected, "{selector} in {names:?}");
        }
    }
}
