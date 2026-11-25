use std::collections::{BTreeMap, HashMap};

use muxy_core::l10n::format::{Kind, Token, tokens};
use muxy_core::l10n::{Plural, PluralCategory};

use super::plist::Value;

const FORMAT_KEY: &str = "NSStringLocalizedFormatKey";
const SPEC_TYPE_KEY: &str = "NSStringFormatSpecTypeKey";
const VALUE_TYPE_KEY: &str = "NSStringFormatValueTypeKey";
const MAX_PLURAL_DEPTH: usize = 8;
const MAX_PLURAL_CHECKS: usize = 4096;

#[derive(Debug, Default)]
pub(super) struct Signature {
    kinds: BTreeMap<usize, Kind>,
    variables: Vec<(String, usize)>,
}

impl Signature {
    pub(super) fn new(format: &str, first: usize) -> Option<Self> {
        if first == 0 {
            return None;
        }
        let mut signature = Self::default();
        let mut next = first;
        for token in tokens(format)? {
            match token {
                Token::Variable(name) => {
                    signature.variables.push((name.to_owned(), next));
                    next += 1;
                }
                Token::Argument(spec) => {
                    let position = spec.position.unwrap_or(next);
                    if *signature.kinds.entry(position).or_insert(spec.kind) != spec.kind {
                        return None;
                    }
                    next = next.max(position + 1);
                }
                Token::Text(_) | Token::Percent => {}
            }
        }
        Some(signature)
    }

    fn satisfies(&self, key: &Self) -> bool {
        self.kinds
            .iter()
            .all(|(position, kind)| key.kinds.get(position) == Some(kind))
    }
}

pub(super) fn incompatible_key(catalog: &BTreeMap<String, Value>) -> Option<&str> {
    catalog
        .iter()
        .find(|(key, value)| !compatible(key, value))
        .map(|(key, _)| key.as_str())
}

fn compatible(key: &str, value: &Value) -> bool {
    match value {
        Value::String(translation) => {
            if !key.contains('%') && !translation.contains('%') {
                return true;
            }
            match (Signature::new(key, 1), Signature::new(translation, 1)) {
                (Some(key), Some(translation)) => translation.satisfies(&key),
                _ => false,
            }
        }
        Value::Dictionary(entry) => Signature::new(key, 1).is_some_and(|key| {
            matches!(entry.get(FORMAT_KEY), Some(Value::String(format))
                if Plurals::new(entry, &key).format(format, 1, 0))
        }),
        Value::Other => false,
    }
}

struct Plurals<'a> {
    entry: &'a BTreeMap<String, Value>,
    key: &'a Signature,
    checked: HashMap<(&'a str, usize, usize), bool>,
    budget: usize,
}

impl<'a> Plurals<'a> {
    fn new(entry: &'a BTreeMap<String, Value>, key: &'a Signature) -> Self {
        Self {
            entry,
            key,
            checked: HashMap::new(),
            budget: MAX_PLURAL_CHECKS,
        }
    }

    fn format(&mut self, format: &str, first: usize, depth: usize) -> bool {
        if depth > MAX_PLURAL_DEPTH || self.budget == 0 {
            return false;
        }
        self.budget -= 1;
        let Some(signature) = Signature::new(format, first).filter(|s| s.satisfies(self.key))
        else {
            return false;
        };
        signature
            .variables
            .iter()
            .all(|(name, position)| self.rule(name, *position, depth))
    }

    fn rule(&mut self, name: &str, position: usize, depth: usize) -> bool {
        let Some((name, Value::Dictionary(rule))) = self.entry.get_key_value(name) else {
            return false;
        };
        let state = (name.as_str(), position, depth);
        if let Some(&known) = self.checked.get(&state) {
            return known;
        }
        let valid = rule.iter().all(|(rule_key, value)| match value {
            Value::String(_) if rule_key == SPEC_TYPE_KEY || rule_key == VALUE_TYPE_KEY => true,
            Value::String(variant) => self.format(variant, position, depth + 1),
            _ => false,
        });
        self.checked.insert(state, valid);
        valid
    }
}

pub(super) fn split(
    catalog: BTreeMap<String, Value>,
    strings: &mut HashMap<String, String>,
    plurals: &mut HashMap<String, Plural>,
) {
    for (key, value) in catalog {
        match value {
            Value::String(text) => {
                strings.insert(key, text);
            }
            Value::Dictionary(entry) => {
                let Some(Value::String(format)) = entry.get(FORMAT_KEY) else {
                    continue;
                };
                let variables = entry
                    .iter()
                    .filter_map(|(name, value)| match value {
                        Value::Dictionary(rule) => Some((name.clone(), forms(rule))),
                        _ => None,
                    })
                    .collect();
                plurals.insert(
                    key,
                    Plural {
                        format: format.clone(),
                        variables,
                    },
                );
            }
            Value::Other => {}
        }
    }
}

fn forms(rule: &BTreeMap<String, Value>) -> HashMap<PluralCategory, String> {
    rule.iter()
        .filter_map(|(name, value)| {
            let category = match name.as_str() {
                "zero" => PluralCategory::Zero,
                "one" => PluralCategory::One,
                "two" => PluralCategory::Two,
                "few" => PluralCategory::Few,
                "many" => PluralCategory::Many,
                "other" => PluralCategory::Other,
                _ => return None,
            };
            match value {
                Value::String(form) => Some((category, form.clone())),
                _ => None,
            }
        })
        .collect()
}
