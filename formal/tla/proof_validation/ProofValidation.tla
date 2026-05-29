---- MODULE ProofValidation ----
EXTENDS Naturals, FiniteSets

Unresolved == "unresolved"
Pending == "pending"
Issued == "issued"
Spendable == "spendable"
Spent == "spent"
ChannelLocked == "channel_locked"
Closed == "closed"
Swept == "swept"
Stale == "stale"
Rejected == "rejected"

States == {
    Unresolved,
    Pending,
    Issued,
    Spendable,
    Spent,
    ChannelLocked,
    Closed,
    Swept,
    Stale,
    Rejected
}

AcceptedStates == {Spendable, ChannelLocked, Closed, Swept}

ExpectedAsset == "OPENUSD"
WrongAsset == "WRONG_ASSET"
Alice == "alice"
Bob == "bob"
Owners == {Alice, Bob}

GenesisAnchor == "genesis_anchor"
ChannelAnchor == "channel_anchor"
CloseAnchor == "close_anchor"
SweepAnchor == "sweep_anchor"
WrongAnchor == "wrong_anchor"
Anchors == {GenesisAnchor, ChannelAnchor, CloseAnchor, SweepAnchor}

MaxAmount == 100
ValidAmounts == {60, 70, 100}

RootFor(amount, owner, anchor) == <<amount, owner, anchor>>

VARIABLES state, issued, assetId, amount, owner, anchor, root,
    history, badReasons, proofFile, stxoPresent, chainView

vars == <<state, issued, assetId, amount, owner, anchor, root,
    history, badReasons, proofFile, stxoPresent, chainView>>

Init ==
    /\ state = Unresolved
    /\ issued = FALSE
    /\ assetId = "none"
    /\ amount = 0
    /\ owner = "none"
    /\ anchor = "none"
    /\ root = "none"
    /\ history = {}
    /\ badReasons = {}
    /\ proofFile = "none"
    /\ stxoPresent = FALSE
    /\ chainView = "stable"

ImportWellFormedProof ==
    /\ state = Unresolved
    /\ badReasons = {}
    /\ proofFile' = "well_formed"
    /\ state' = Pending
    /\ UNCHANGED <<issued, assetId, amount, owner, anchor, root,
        history, badReasons, stxoPresent, chainView>>

IssueValid ==
    /\ state = Pending
    /\ proofFile = "well_formed"
    /\ badReasons = {}
    /\ issued = FALSE
    /\ chainView = "stable"
    /\ state' = Issued
    /\ issued' = TRUE
    /\ assetId' = ExpectedAsset
    /\ amount' = MaxAmount
    /\ owner' = Alice
    /\ anchor' = GenesisAnchor
    /\ root' = RootFor(MaxAmount, Alice, GenesisAnchor)
    /\ history' = history \cup {"issue"}
    /\ stxoPresent' = TRUE
    /\ UNCHANGED <<badReasons, proofFile, chainView>>

AcceptIssued ==
    /\ state = Issued
    /\ badReasons = {}
    /\ assetId = ExpectedAsset
    /\ amount = MaxAmount
    /\ owner = Alice
    /\ anchor = GenesisAnchor
    /\ root = RootFor(amount, owner, anchor)
    /\ stxoPresent = TRUE
    /\ state' = Spendable
    /\ history' = history \cup {"accept_issuance"}
    /\ UNCHANGED <<issued, assetId, amount, owner, anchor, root,
        badReasons, proofFile, stxoPresent, chainView>>

SplitValid ==
    /\ state = Spendable
    /\ badReasons = {}
    /\ amount = MaxAmount
    /\ root = RootFor(amount, owner, anchor)
    /\ stxoPresent = TRUE
    /\ amount' = 70
    /\ root' = RootFor(70, owner, anchor)
    /\ history' = history \cup {"split"}
    /\ UNCHANGED <<state, issued, assetId, owner, anchor, badReasons,
        proofFile, stxoPresent, chainView>>

TransferValid ==
    /\ state = Spendable
    /\ badReasons = {}
    /\ amount = 70
    /\ owner = Alice
    /\ root = RootFor(amount, owner, anchor)
    /\ stxoPresent = TRUE
    /\ owner' = Bob
    /\ root' = RootFor(amount, Bob, anchor)
    /\ history' = history \cup {"transfer"}
    /\ UNCHANGED <<state, issued, assetId, amount, anchor, badReasons,
        proofFile, stxoPresent, chainView>>

ChannelFundingValid ==
    /\ state = Spendable
    /\ badReasons = {}
    /\ amount = 70
    /\ owner = Bob
    /\ root = RootFor(amount, owner, anchor)
    /\ stxoPresent = TRUE
    /\ state' = ChannelLocked
    /\ anchor' = ChannelAnchor
    /\ root' = RootFor(amount, owner, ChannelAnchor)
    /\ history' = history \cup {"channel_funding"}
    /\ UNCHANGED <<issued, assetId, amount, owner, badReasons,
        proofFile, stxoPresent, chainView>>

CommitmentUpdateValid ==
    /\ state = ChannelLocked
    /\ badReasons = {}
    /\ root = RootFor(amount, owner, anchor)
    /\ stxoPresent = TRUE
    /\ history' = history \cup {"commitment_update"}
    /\ UNCHANGED <<state, issued, assetId, amount, owner, anchor, root,
        badReasons, proofFile, stxoPresent, chainView>>

CloseValid ==
    /\ state = ChannelLocked
    /\ badReasons = {}
    /\ root = RootFor(amount, owner, anchor)
    /\ stxoPresent = TRUE
    /\ state' = Closed
    /\ anchor' = CloseAnchor
    /\ root' = RootFor(amount, owner, CloseAnchor)
    /\ history' = history \cup {"close"}
    /\ UNCHANGED <<issued, assetId, amount, owner, badReasons,
        proofFile, stxoPresent, chainView>>

SweepValid ==
    /\ state = Closed
    /\ badReasons = {}
    /\ root = RootFor(amount, owner, anchor)
    /\ stxoPresent = TRUE
    /\ state' = Swept
    /\ anchor' = SweepAnchor
    /\ root' = RootFor(amount, owner, SweepAnchor)
    /\ history' = history \cup {"sweep"}
    /\ UNCHANGED <<issued, assetId, amount, owner, badReasons,
        proofFile, stxoPresent, chainView>>

RejectWrongGenesis ==
    /\ state \in {Unresolved, Pending}
    /\ state' = Rejected
    /\ assetId' = WrongAsset
    /\ badReasons' = badReasons \cup {"wrong_genesis"}
    /\ proofFile' = "well_formed"
    /\ UNCHANGED <<issued, amount, owner, anchor, root, history,
        stxoPresent, chainView>>

RejectWrongAnchor ==
    /\ state \in {Pending, Spendable}
    /\ state' = Rejected
    /\ anchor' = WrongAnchor
    /\ badReasons' = badReasons \cup {"wrong_anchor"}
    /\ proofFile' = "well_formed"
    /\ UNCHANGED <<issued, assetId, amount, owner, root, history,
        stxoPresent, chainView>>

RejectWrongOwner ==
    /\ state \in {Pending, Spendable}
    /\ state' = Rejected
    /\ owner' = "wrong_owner"
    /\ badReasons' = badReasons \cup {"wrong_owner"}
    /\ proofFile' = "well_formed"
    /\ UNCHANGED <<issued, assetId, amount, anchor, root, history,
        stxoPresent, chainView>>

RejectWrongAssetType ==
    /\ state \in {Unresolved, Pending}
    /\ state' = Rejected
    /\ badReasons' = badReasons \cup {"wrong_asset_type"}
    /\ proofFile' = "well_formed"
    /\ UNCHANGED <<issued, assetId, amount, owner, anchor, root, history,
        stxoPresent, chainView>>

RejectWrongAmount ==
    /\ state \in {Pending, Spendable}
    /\ state' = Rejected
    /\ amount' = MaxAmount + 2
    /\ badReasons' = badReasons \cup {"wrong_amount"}
    /\ proofFile' = "well_formed"
    /\ UNCHANGED <<issued, assetId, owner, anchor, root, history,
        stxoPresent, chainView>>

RejectWrongRootHash ==
    /\ state \in {Pending, Issued, Spendable}
    /\ state' = Rejected
    /\ root' = <<"wrong_hash", amount, owner, anchor>>
    /\ badReasons' = badReasons \cup {"wrong_root_hash"}
    /\ proofFile' = "well_formed"
    /\ UNCHANGED <<issued, assetId, amount, owner, anchor, history,
        stxoPresent, chainView>>

RejectWrongRootSum ==
    /\ state \in {Pending, Issued, Spendable}
    /\ state' = Rejected
    /\ amount' = MaxAmount + 3
    /\ badReasons' = badReasons \cup {"wrong_root_sum"}
    /\ proofFile' = "well_formed"
    /\ UNCHANGED <<issued, assetId, owner, anchor, root, history,
        stxoPresent, chainView>>

RejectMismatchedTapCommitmentOutputRoot ==
    /\ state \in {Spendable, ChannelLocked}
    /\ state' = Rejected
    /\ badReasons' = badReasons \cup {"mismatched_tap_commitment_output_root"}
    /\ UNCHANGED <<issued, assetId, amount, owner, anchor, root, history,
        proofFile, stxoPresent, chainView>>

RejectInvalidSplitSum ==
    /\ state = Spendable
    /\ amount = MaxAmount
    /\ state' = Rejected
    /\ amount' = MaxAmount + 1
    /\ badReasons' = badReasons \cup {"invalid_split_sum"}
    /\ UNCHANGED <<issued, assetId, owner, anchor, root, history,
        proofFile, stxoPresent, chainView>>

RejectMalformedProofFile ==
    /\ state \in {Unresolved, Pending}
    /\ state' = Rejected
    /\ proofFile' = "malformed"
    /\ badReasons' = badReasons \cup {"malformed_proof_file"}
    /\ UNCHANGED <<issued, assetId, amount, owner, anchor, root, history,
        stxoPresent, chainView>>

RejectMissingStxo ==
    /\ state \in {Pending, Issued, Spendable, ChannelLocked, Closed}
    /\ state' = Unresolved
    /\ stxoPresent' = FALSE
    /\ badReasons' = badReasons \cup {"missing_stxo"}
    /\ UNCHANGED <<issued, assetId, amount, owner, anchor, root, history,
        proofFile, chainView>>

RejectStaleProof ==
    /\ state \in {Pending, Issued, Spendable, ChannelLocked, Closed}
    /\ state' = Stale
    /\ badReasons' = badReasons \cup {"stale_proof"}
    /\ UNCHANGED <<issued, assetId, amount, owner, anchor, root, history,
        proofFile, stxoPresent, chainView>>

RejectReorgSensitiveHistory ==
    /\ state \in {Pending, Issued, Spendable, ChannelLocked, Closed}
    /\ state' = Stale
    /\ chainView' = "reorged"
    /\ badReasons' = badReasons \cup {"reorg_sensitive_history"}
    /\ UNCHANGED <<issued, assetId, amount, owner, anchor, root, history,
        proofFile, stxoPresent>>

TerminalStutter ==
    /\ state \in {Unresolved, Spent, Swept, Stale, Rejected}
    /\ UNCHANGED vars

Next ==
    \/ ImportWellFormedProof
    \/ IssueValid
    \/ AcceptIssued
    \/ SplitValid
    \/ TransferValid
    \/ ChannelFundingValid
    \/ CommitmentUpdateValid
    \/ CloseValid
    \/ SweepValid
    \/ RejectWrongGenesis
    \/ RejectWrongAnchor
    \/ RejectWrongOwner
    \/ RejectWrongAssetType
    \/ RejectWrongAmount
    \/ RejectWrongRootHash
    \/ RejectWrongRootSum
    \/ RejectMismatchedTapCommitmentOutputRoot
    \/ RejectInvalidSplitSum
    \/ RejectMalformedProofFile
    \/ RejectMissingStxo
    \/ RejectStaleProof
    \/ RejectReorgSensitiveHistory
    \/ TerminalStutter

Spec == Init /\ [][Next]_vars

AcceptedBalancesHaveIssuedHistory ==
    state \in AcceptedStates =>
        /\ issued = TRUE
        /\ "issue" \in history
        /\ "accept_issuance" \in history

AcceptedFieldsStayCoherent ==
    state \in AcceptedStates =>
        /\ assetId = ExpectedAsset
        /\ amount \in ValidAmounts
        /\ owner \in Owners
        /\ anchor \in Anchors
        /\ root = RootFor(amount, owner, anchor)

AcceptedProofInputsAreValid ==
    state \in AcceptedStates =>
        /\ proofFile = "well_formed"
        /\ stxoPresent = TRUE
        /\ chainView = "stable"
        /\ badReasons = {}

BadProofsCannotBecomeAccepted ==
    badReasons # {} => state \notin AcceptedStates

ReorgHistoryCannotBeAccepted ==
    chainView = "reorged" => state \notin AcceptedStates

NoAcceptedInflation ==
    state \in AcceptedStates => amount <= MaxAmount

StateAnchorMatchesAcceptedStage ==
    /\ state = Spendable => anchor = GenesisAnchor
    /\ state = ChannelLocked => anchor = ChannelAnchor
    /\ state = Closed => anchor = CloseAnchor
    /\ state = Swept => anchor = SweepAnchor

====
