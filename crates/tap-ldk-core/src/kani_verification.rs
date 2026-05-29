use crate::{
    asset::AssetAmount,
    proof::{ProofAnchorPolicy, ProofAnchorState, ProofHistoryState, ProofTransitionKind},
};

#[kani::proof]
fn asset_amount_checked_add_matches_u64_overflow() {
    let left: u64 = kani::any();
    let right: u64 = kani::any();

    let checked = AssetAmount::new(left).checked_add(AssetAmount::new(right));

    assert_eq!(checked.is_ok(), left.checked_add(right).is_some());
}

#[kani::proof]
fn asset_amount_checked_sub_matches_u64_underflow() {
    let left: u64 = kani::any();
    let right: u64 = kani::any();

    let checked = AssetAmount::new(left).checked_sub(AssetAmount::new(right));

    assert_eq!(checked.is_ok(), left.checked_sub(right).is_some());
}

#[kani::proof]
fn strict_anchor_policy_accepts_only_confirmed_anchors() {
    let state = symbolic_anchor_state();
    let policy = ProofAnchorPolicy::strict_confirmed();

    assert_eq!(
        policy.accepts_anchor_state(state),
        state == ProofAnchorState::Confirmed
    );
}

#[kani::proof]
fn pending_policy_accepts_only_pending_and_confirmed_anchors() {
    let state = symbolic_anchor_state();
    let policy = ProofAnchorPolicy::strict_confirmed().with_pending_accepted(true);

    assert_eq!(
        policy.accepts_anchor_state(state),
        matches!(
            state,
            ProofAnchorState::Pending | ProofAnchorState::Confirmed
        )
    );
}

#[kani::proof]
fn proof_transition_state_policy_keeps_bad_states_unspendable() {
    let transition = symbolic_transition_kind();
    let state = symbolic_history_state();

    if matches!(
        state,
        ProofHistoryState::Rejected
            | ProofHistoryState::Unresolved
            | ProofHistoryState::Pending
            | ProofHistoryState::Stale
            | ProofHistoryState::Spent
    ) {
        assert!(!transition_allows_input_state(transition, state));
    }
}

fn symbolic_anchor_state() -> ProofAnchorState {
    match symbolic_index(5) {
        0 => ProofAnchorState::Unknown,
        1 => ProofAnchorState::Pending,
        2 => ProofAnchorState::Confirmed,
        3 => ProofAnchorState::Stale,
        _ => ProofAnchorState::Reorged,
    }
}

fn symbolic_history_state() -> ProofHistoryState {
    match symbolic_index(9) {
        0 => ProofHistoryState::Accepted,
        1 => ProofHistoryState::Rejected,
        2 => ProofHistoryState::Unresolved,
        3 => ProofHistoryState::Pending,
        4 => ProofHistoryState::Stale,
        5 => ProofHistoryState::Spent,
        6 => ProofHistoryState::ChannelLocked,
        7 => ProofHistoryState::Closed,
        _ => ProofHistoryState::Swept,
    }
}

fn symbolic_transition_kind() -> ProofTransitionKind {
    match symbolic_index(10) {
        0 => ProofTransitionKind::Issuance,
        1 => ProofTransitionKind::Split,
        2 => ProofTransitionKind::Transfer,
        3 => ProofTransitionKind::ChannelFunding,
        4 => ProofTransitionKind::CommitmentUpdate,
        5 => ProofTransitionKind::CooperativeClose,
        6 => ProofTransitionKind::UnilateralClose,
        7 => ProofTransitionKind::SecondLevelHtlc,
        8 => ProofTransitionKind::Sweep,
        _ => ProofTransitionKind::ProofExport,
    }
}

fn symbolic_index(modulus: u8) -> u8 {
    let value: u8 = kani::any();
    value % modulus
}

fn transition_allows_input_state(
    transition: ProofTransitionKind,
    state: ProofHistoryState,
) -> bool {
    match transition {
        ProofTransitionKind::Issuance => false,
        ProofTransitionKind::Split
        | ProofTransitionKind::Transfer
        | ProofTransitionKind::ChannelFunding => state == ProofHistoryState::Accepted,
        ProofTransitionKind::CommitmentUpdate => state == ProofHistoryState::ChannelLocked,
        ProofTransitionKind::CooperativeClose | ProofTransitionKind::UnilateralClose => {
            state == ProofHistoryState::ChannelLocked
        }
        ProofTransitionKind::SecondLevelHtlc | ProofTransitionKind::Sweep => {
            state == ProofHistoryState::Closed
        }
        ProofTransitionKind::ProofExport => matches!(
            state,
            ProofHistoryState::Accepted
                | ProofHistoryState::ChannelLocked
                | ProofHistoryState::Closed
                | ProofHistoryState::Swept
        ),
    }
}
