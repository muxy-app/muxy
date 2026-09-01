pub use muxy_terminal::offline::RecoveryState;

pub fn blocked_identity(reason: impl Into<String>) -> RecoveryState {
    RecoveryState::InvalidIdentity {
        reason: reason.into(),
    }
}

pub fn startup_state(established: bool, inventory_available: bool, found: bool) -> RecoveryState {
    if !established || found {
        RecoveryState::Reconnecting { attempt: 0 }
    } else if inventory_available {
        RecoveryState::Missing
    } else {
        RecoveryState::Unreachable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_distinguishes_transport_from_confirmed_absence() {
        assert_eq!(startup_state(true, true, false), RecoveryState::Missing);
        assert_eq!(
            startup_state(true, false, false),
            RecoveryState::Unreachable
        );
        assert_eq!(
            startup_state(false, true, false),
            RecoveryState::Reconnecting { attempt: 0 }
        );
    }
}
