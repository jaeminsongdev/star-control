use super::{Decision, Severity};
use crate::constants::P0_RULE_IDS;
use crate::json_fields::{optional_string, required_array, required_string};
use crate::SentinelError;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleDefinition {
    pub rule_id: String,
    pub severity: Severity,
    pub description: String,
    pub decision_effect: Decision,
}

impl RuleDefinition {
    fn from_value(value: &serde_json::Value, artifact: &str) -> Result<Self, SentinelError> {
        let rule_id = required_string(value, "rule_id", artifact)?;
        let severity = Severity::parse(
            &required_string(value, "severity", artifact)?,
            artifact,
            "severity",
        )?;
        let decision_effect = match optional_string(value, "decision_effect", artifact)? {
            Some(value) => Decision::parse(&value, artifact, "decision_effect")?,
            None => Decision::default_for_severity(severity),
        };

        Ok(Self {
            rule_id,
            severity,
            description: required_string(value, "description", artifact)?,
            decision_effect,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct P0RuleRegistry {
    pub profile: String,
    pub rules: Vec<RuleDefinition>,
    by_id: HashMap<String, RuleDefinition>,
}

impl P0RuleRegistry {
    pub fn from_value(value: &serde_json::Value) -> Result<Self, SentinelError> {
        let profile = required_string(value, "profile", "P0RuleRegistry")?;
        let supported: HashSet<&str> = P0_RULE_IDS.into_iter().collect();
        let mut seen = HashSet::new();
        let mut rules = Vec::new();
        let mut by_id = HashMap::new();

        for (index, rule_value) in required_array(value, "rules", "P0RuleRegistry")?
            .iter()
            .enumerate()
        {
            let artifact = format!("P0RuleRegistry.rules[{}]", index);
            let rule = RuleDefinition::from_value(rule_value, &artifact)?;
            if !supported.contains(rule.rule_id.as_str()) {
                return Err(SentinelError::Registry {
                    message: format!("unsupported P0 rule id {}", rule.rule_id),
                });
            }
            if !seen.insert(rule.rule_id.clone()) {
                return Err(SentinelError::Registry {
                    message: format!("duplicate rule id {}", rule.rule_id),
                });
            }
            by_id.insert(rule.rule_id.clone(), rule.clone());
            rules.push(rule);
        }

        for expected in P0_RULE_IDS {
            if !seen.contains(expected) {
                return Err(SentinelError::Registry {
                    message: format!("missing required P0 rule id {}", expected),
                });
            }
        }

        Ok(Self {
            profile,
            rules,
            by_id,
        })
    }

    pub fn rule(&self, rule_id: &str) -> Option<&RuleDefinition> {
        self.by_id.get(rule_id)
    }
}
