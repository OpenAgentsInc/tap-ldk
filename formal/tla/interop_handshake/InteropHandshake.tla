---- MODULE InteropHandshake ----
EXTENDS Naturals

LlRole == "independent_counterparty"

VARIABLES nativeState, llState, proofSync, channelState, rfqState, paymentState, balancesAgree, gap

vars == <<nativeState, llState, proofSync, channelState, rfqState, paymentState, balancesAgree, gap>>

Init ==
    /\ nativeState = "ready"
    /\ llState = LlRole
    /\ proofSync = FALSE
    /\ channelState = "none"
    /\ rfqState = "none"
    /\ paymentState = "none"
    /\ balancesAgree = FALSE
    /\ gap = FALSE

SyncProofs ==
    /\ proofSync = FALSE
    /\ proofSync' = TRUE
    /\ UNCHANGED <<nativeState, llState, channelState, rfqState, paymentState, balancesAgree, gap>>

OpenInteropChannel ==
    /\ proofSync = TRUE
    /\ channelState = "none"
    /\ channelState' = "open"
    /\ UNCHANGED <<nativeState, llState, proofSync, rfqState, paymentState, balancesAgree, gap>>

NegotiateRfq ==
    /\ channelState = "open"
    /\ rfqState = "none"
    /\ rfqState' = "accepted"
    /\ UNCHANGED <<nativeState, llState, proofSync, channelState, paymentState, balancesAgree, gap>>

PayInteropInvoice ==
    /\ channelState = "open"
    /\ rfqState = "accepted"
    /\ paymentState = "none"
    /\ paymentState' = "settled"
    /\ balancesAgree' = TRUE
    /\ UNCHANGED <<nativeState, llState, proofSync, channelState, rfqState, gap>>

DocumentCompatibilityGap ==
    /\ paymentState = "none"
    /\ gap = FALSE
    /\ gap' = TRUE
    /\ paymentState' = "gap"
    /\ balancesAgree' = FALSE
    /\ UNCHANGED <<nativeState, llState, proofSync, channelState, rfqState>>

TerminalStutter ==
    /\ paymentState \in {"settled", "gap"}
    /\ UNCHANGED vars

Next ==
    \/ SyncProofs
    \/ OpenInteropChannel
    \/ NegotiateRfq
    \/ PayInteropInvoice
    \/ DocumentCompatibilityGap
    \/ TerminalStutter

Spec == Init /\ [][Next]_vars

LightningLabsIsCounterparty ==
    llState = LlRole

SettledRequiresFullHandshake ==
    paymentState = "settled" =>
        /\ proofSync = TRUE
        /\ channelState = "open"
        /\ rfqState = "accepted"
        /\ balancesAgree = TRUE

GapIsNotSuccess ==
    gap = TRUE => paymentState = "gap" /\ balancesAgree = FALSE

NoSidecarClaim ==
    nativeState = "ready" /\ llState = LlRole

====
