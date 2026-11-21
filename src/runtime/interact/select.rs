//! Reading results from the app and choosing which widget to act on.

use anyhow::{bail, Result};
use serde::Serialize;
use serde_json::Value;

use super::dart::{FIELD_SEP, RECORD_SEP};
use super::Finder;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Candidate {
    pub index: usize,
    #[serde(rename = "type")]
    pub type_name: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub rect: [f64; 4],
    /// `true` when a tap at its centre would reach it, `None` if unknown.
    pub hittable: Option<bool>,
}

/// Split a result string into records of fields, failing on an `ERR` record.
pub(super) fn records(raw: &str) -> Result<Vec<Vec<String>>> {
    let recs: Vec<Vec<String>> = raw
        .split(RECORD_SEP)
        .map(|r| r.split(FIELD_SEP).map(str::to_string).collect())
        .collect();
    match recs.first().and_then(|r| r.first()).map(String::as_str) {
        Some("ERR") => bail!("{}", recs[0].get(1).cloned().unwrap_or_default()),
        Some("OK") | Some("DONE") | Some("SCREEN") => Ok(recs),
        _ => bail!("Unexpected result from the app: {raw}"),
    }
}

pub(super) fn num(fields: &[String], i: usize) -> f64 {
    fields.get(i).and_then(|v| v.parse().ok()).unwrap_or(0.0)
}

pub(super) fn hittable(flag: Option<&String>) -> Option<bool> {
    match flag.map(String::as_str) {
        Some("1") => Some(true),
        Some("0") => Some(false),
        _ => None,
    }
}

pub(super) fn parse_candidates(recs: &[Vec<String>]) -> Vec<Candidate> {
    recs.iter()
        .skip(1)
        .enumerate()
        .map(|(index, f)| Candidate {
            index,
            type_name: f.first().cloned().unwrap_or_default(),
            label: f.get(1).cloned().unwrap_or_default(),
            key: None,
            rect: [num(f, 2), num(f, 3), num(f, 4), num(f, 5)],
            hittable: hittable(f.get(6)),
        })
        .collect()
}

/// Drop elements a tap could not reach (under another route or a modal
/// barrier) and number the rest. Returns the kept elements and how many
/// were dropped.
pub fn visible_only(all: Vec<Candidate>) -> (Vec<Candidate>, usize) {
    let total = all.len();
    let kept: Vec<Candidate> = all
        .into_iter()
        .filter(|c| c.hittable != Some(false))
        .enumerate()
        .map(|(index, c)| Candidate { index, ..c })
        .collect();
    let covered = total - kept.len();
    (kept, covered)
}

/// Pick the candidate to act on, refusing ambiguous or empty matches.
pub fn choose(cands: &[Candidate], index: Option<usize>, finder: &Finder) -> Result<usize> {
    if let Some(i) = index {
        if i < cands.len() {
            return Ok(i);
        }
        bail!(
            "index {i} is out of range: {} widget(s) match {}",
            cands.len(),
            describe_finder(finder)
        );
    }
    let hittable: Vec<&Candidate> = cands.iter().filter(|c| c.hittable != Some(false)).collect();
    match (cands.len(), hittable.len()) {
        (0, _) => bail!("No widget on screen matches {}", describe_finder(finder)),
        (_, 0) => bail!(
            "Every widget matching {} is covered by another screen or dialog; nothing was tapped",
            describe_finder(finder)
        ),
        (1, _) => Ok(0),
        (_, 1) => Ok(hittable[0].index),
        _ => bail!(
            "{} widgets match {}; nothing was tapped. Pass `index` to pick one:\n{}",
            cands.len(),
            describe_finder(finder),
            serde_json::to_string_pretty(cands).unwrap_or_default()
        ),
    }
}

pub(super) fn describe_finder(finder: &Finder) -> String {
    serde_json::to_string(finder).unwrap_or_default()
}

pub(super) fn labels(screen: &Value) -> Vec<String> {
    screen["elements"]
        .as_array()
        .map(|els| {
            els.iter()
                .filter(|e| !e["label"].as_str().unwrap_or("").is_empty())
                .map(|e| {
                    format!(
                        "{}: {}",
                        e["type"].as_str().unwrap_or(""),
                        e["label"].as_str().unwrap_or("")
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Labels present in `after` but not `before` (as a multiset), and the reverse.
pub fn label_changes(before: &[String], after: &[String]) -> (Vec<String>, Vec<String>) {
    let minus = |a: &[String], b: &[String]| {
        let mut rest = b.to_vec();
        a.iter()
            .filter(|x| match rest.iter().position(|y| y == *x) {
                Some(p) => {
                    rest.remove(p);
                    false
                }
                None => true,
            })
            .cloned()
            .collect::<Vec<_>>()
    };
    (minus(after, before), minus(before, after))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(index: usize, hit: Option<bool>) -> Candidate {
        Candidate {
            index,
            type_name: "Text".into(),
            label: "x".into(),
            key: None,
            rect: [0.0; 4],
            hittable: hit,
        }
    }
    #[test]
    fn records_surface_app_errors() {
        let err = records("ERR\u{1f}no root").unwrap_err();
        assert_eq!(err.to_string(), "no root");
        assert!(records("garbage").is_err());
    }

    #[test]
    fn parse_candidates_reads_rect_and_hittable() {
        let recs = records("OK\u{1e}Text\u{1f}Hi\u{1f}1\u{1f}2\u{1f}3\u{1f}4\u{1f}0").unwrap();
        let c = parse_candidates(&recs);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].rect, [1.0, 2.0, 3.0, 4.0]);
        assert_eq!(c[0].hittable, Some(false));
    }

    #[test]
    fn choose_refuses_ambiguity_and_empty() {
        let f = Finder::default();
        assert!(choose(&[], None, &f)
            .unwrap_err()
            .to_string()
            .contains("No widget"));
        let two = [cand(0, Some(true)), cand(1, Some(true))];
        assert!(choose(&two, None, &f)
            .unwrap_err()
            .to_string()
            .contains("Pass `index`"));
        assert_eq!(choose(&two, Some(1), &f).unwrap(), 1);
        assert!(choose(&two, Some(5), &f).is_err());
    }

    #[test]
    fn choose_refuses_covered_widgets() {
        let err = choose(&[cand(0, Some(false))], None, &Finder::default()).unwrap_err();
        assert!(err.to_string().contains("covered"));
    }

    #[test]
    fn choose_prefers_the_only_hittable_match() {
        let f = Finder::default();
        let c = [cand(0, Some(false)), cand(1, Some(true))];
        assert_eq!(choose(&c, None, &f).unwrap(), 1);
    }

    #[test]
    fn visible_only_drops_covered_elements_and_renumbers() {
        let (kept, covered) = visible_only(vec![
            cand(0, Some(false)),
            cand(1, Some(true)),
            cand(2, None),
        ]);
        assert_eq!(covered, 1);
        assert_eq!(kept.iter().map(|c| c.index).collect::<Vec<_>>(), vec![0, 1]);
    }

    #[test]
    fn label_changes_is_a_multiset_difference() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let (added, removed) =
            label_changes(&s(&["Text: 0", "Text: a"]), &s(&["Text: 1", "Text: a"]));
        assert_eq!(added, s(&["Text: 1"]));
        assert_eq!(removed, s(&["Text: 0"]));
    }
}
