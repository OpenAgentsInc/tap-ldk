---- MODULE AssetChannel ----
EXTENDS Naturals

Peers == {"local", "remote"}
MaxAssetAmount == 100

VARIABLES support, proofState, proofReplay, status, durableState, balance

vars == <<support, proofState, proofReplay, status, durableState, balance>>

Init ==
    /\ support = [p \in Peers |-> FALSE]
    /\ proofState = "none"
    /\ proofReplay = "none"
    /\ status = "init"
    /\ durableState = FALSE
    /\ balance = 0

Negotiate ==
    /\ status = "init"
    /\ support' = [p \in Peers |-> TRUE]
    /\ status' = "negotiated"
    /\ UNCHANGED <<proofState, proofReplay, durableState, balance>>

ReceiveProofs ==
    /\ status = "negotiated"
    /\ proofState = "none"
    /\ proofState' = "complete"
    /\ UNCHANGED <<support, proofReplay, status, durableState, balance>>

ValidateProofs ==
    /\ status = "negotiated"
    /\ proofState = "complete"
    /\ proofState' = "valid"
    /\ proofReplay' = "accepted"
    /\ status' = "proofs_received"
    /\ UNCHANGED <<support, durableState, balance>>

RejectInvalidProofs ==
    /\ status = "negotiated"
    /\ proofState = "complete"
    /\ proofState' = "invalid"
    /\ proofReplay' = "rejected"
    /\ status' = "rejected"
    /\ UNCHANGED <<support, durableState, balance>>

OpenChannel ==
    /\ status = "proofs_received"
    /\ proofState = "valid"
    /\ proofReplay = "accepted"
    /\ \A p \in Peers: support[p] = TRUE
    /\ status' = "open"
    /\ durableState' = TRUE
    /\ balance' = MaxAssetAmount
    /\ UNCHANGED <<support, proofState, proofReplay>>

TerminalStutter ==
    /\ status \in {"open", "rejected"}
    /\ UNCHANGED vars

Next ==
    \/ Negotiate
    \/ ReceiveProofs
    \/ ValidateProofs
    \/ RejectInvalidProofs
    \/ OpenChannel
    \/ TerminalStutter

Spec == Init /\ [][Next]_vars

OpenOnlyAfterNegotiation ==
    status = "open" => \A p \in Peers: support[p] = TRUE

OpenOnlyWithValidProof ==
    status = "open" => proofState = "valid"

OpenOnlyWithReplayedProof ==
    status = "open" => proofReplay = "accepted"

DurableOnlyWhenOpen ==
    durableState = TRUE => status = "open"

NoAssetInflation ==
    balance <= MaxAssetAmount

====
