//! SPEC-0040: explicit, owner-local Stop/Discard policy and durable resolution reports.
use super::*;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ConflictReaction {
    #[default]
    Stop,
    Discard,
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct ConflictPolicy {
    pub default: ConflictReaction,
    pub commands: BTreeMap<String, ConflictReaction>,
}

#[derive(Clone, PartialEq, Eq)]
pub enum Resolution {
    Completed,
    KeptCurrent {
        discarded_keys: Vec<String>,
        unmapped: Vec<String>,
        skipped_writes: usize,
    },
}

impl ConflictPolicy {
    pub(super) fn reaction(&self, name: &str) -> ConflictReaction {
        self.commands.get(name).copied().unwrap_or(self.default)
    }

    pub(super) fn validate(&self) -> Result<(), CommandError> {
        let mut commands = BTreeMap::new();
        for (name, reaction) in &self.commands {
            super::owner::validate_command_name(name)?;
            commands.insert(name.clone(), Json::String(reaction.name().into()));
        }
        Json::Object(BTreeMap::from([
            ("default".into(), Json::String(self.default.name().into())),
            ("commands".into(), Json::Object(commands)),
        ]))
        .canonical()
        .map_err(|_| CommandError::LimitExceeded)?;
        Ok(())
    }
}

impl ConflictReaction {
    fn name(self) -> &'static str {
        match self {
            Self::Stop => "stop",
            Self::Discard => "discard",
        }
    }
}

impl Resolution {
    /// API-0011's exact owned wire value, also used for precommit size validation.
    pub fn to_json(&self) -> Json {
        let text = |value: &str| Json::String(value.to_owned());
        Json::Object(match self {
            Self::Completed => BTreeMap::from([("kind".into(), text("completed"))]),
            Self::KeptCurrent {
                discarded_keys,
                unmapped,
                skipped_writes,
            } => BTreeMap::from([
                ("kind".into(), text("kept_current")),
                (
                    "discarded_keys".into(),
                    Json::Array(discarded_keys.iter().map(|s| text(s)).collect()),
                ),
                (
                    "unmapped".into(),
                    Json::Array(unmapped.iter().map(|s| text(s)).collect()),
                ),
                ("skipped_writes".into(), text(&skipped_writes.to_string())),
            ]),
        })
    }

    pub(super) fn preflight(&self) -> Result<(), CommandError> {
        // Size the complete ABI-1 success frame, including its nesting/node overhead.
        // Unit callers must not evade the same bound by dropping the report.
        Json::Object(BTreeMap::from([
            ("abi".into(), Json::String("1".into())),
            ("ok".into(), Json::Bool(true)),
            ("value".into(), self.to_json()),
        ]))
        .canonical()
        .map_err(|_| CommandError::LimitExceeded)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cp11_sizes_the_complete_success_frame() {
        let mut result = Resolution::KeptCurrent {
            discarded_keys: vec![],
            unmapped: vec![String::new()],
            skipped_writes: 0,
        };
        let overhead = result.to_json().canonical().unwrap().len();
        if let Resolution::KeptCurrent {
            unmapped,
            ..
        } = &mut result
        {
            unmapped[0] = "x".repeat(super::super::json::MAX_BYTES - overhead);
        }
        assert_eq!(
            result.to_json().canonical().unwrap().len(),
            super::super::json::MAX_BYTES
        );
        assert_eq!(result.preflight(), Err(CommandError::LimitExceeded));
    }
}
