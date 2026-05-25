---- MODULE AssetChannel ----
EXTENDS Naturals

Peers == {"local", "remote"}
MaxAssetAmount == 100

VARIABLES support, proofState, status, durableState, balance

vars == <<support, proofState, status, durableState, balance>>

Init ==
    /\ support = [p \in Peers |-> FALSE]
    /\ proofState = "none"
    /\ status = "init"
    /\ durableState = FALSE
    /\ balance = 0

Negotiate ==
    /\ status = "init"
    /\ support' = [p \in Peers |-> TRUE]
    /\ status' = "negotiated"
    /\ UNCHANGED <<proofState, durableState, balance>>

ReceiveProofs ==
    /\ status = "negotiated"
    /\ proofState = "none"
    /\ proofState' = "complete"
    /\ UNCHANGED <<support, status, durableState, balance>>

ValidateProofs ==
    /\ status = "negotiated"
    /\ proofState = "complete"
    /\ proofState' = "valid"
    /\ status' = "proofs_received"
    /\ UNCHANGED <<support, durableState, balance>>

RejectInvalidProofs ==
    /\ status = "negotiated"
    /\ proofState = "complete"
    /\ proofState' = "invalid"
    /\ status' = "rejected"
    /\ UNCHANGED <<support, durableState, balance>>

OpenChannel ==
    /\ status = "proofs_received"
    /\ proofState = "valid"
    /\ \A p \in Peers: support[p] = TRUE
    /\ status' = "open"
    /\ durableState' = TRUE
    /\ balance' = MaxAssetAmount
    /\ UNCHANGED <<support, proofState>>

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

DurableOnlyWhenOpen ==
    durableState = TRUE => status = "open"

NoAssetInflation ==
    balance <= MaxAssetAmount

====
