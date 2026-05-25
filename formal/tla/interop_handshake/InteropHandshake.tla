---- MODULE InteropHandshake ----
EXTENDS Naturals

LlRole == "independent_counterparty"

VARIABLES nativeState, llState, blobState, proofSync, channelState, rfqState, paymentState, balancesAgree, gap

vars == <<nativeState, llState, blobState, proofSync, channelState, rfqState, paymentState, balancesAgree, gap>>

Init ==
    /\ nativeState = "ready"
    /\ llState = LlRole
    /\ blobState = "none"
    /\ proofSync = FALSE
    /\ channelState = "none"
    /\ rfqState = "none"
    /\ paymentState = "none"
    /\ balancesAgree = FALSE
    /\ gap = FALSE

DecodeLightningLabsBlobs ==
    /\ blobState = "none"
    /\ blobState' = "decoded"
    /\ UNCHANGED <<nativeState, llState, proofSync, channelState, rfqState, paymentState, balancesAgree, gap>>

RejectLightningLabsBlobs ==
    /\ blobState = "none"
    /\ blobState' = "rejected"
    /\ gap' = TRUE
    /\ paymentState' = "gap"
    /\ balancesAgree' = FALSE
    /\ UNCHANGED <<nativeState, llState, proofSync, channelState, rfqState>>

SyncProofs ==
    /\ proofSync = FALSE
    /\ proofSync' = TRUE
    /\ UNCHANGED <<nativeState, llState, blobState, channelState, rfqState, paymentState, balancesAgree, gap>>

OpenInteropChannel ==
    /\ proofSync = TRUE
    /\ blobState = "decoded"
    /\ channelState = "none"
    /\ channelState' = "open"
    /\ UNCHANGED <<nativeState, llState, blobState, proofSync, rfqState, paymentState, balancesAgree, gap>>

NegotiateRfq ==
    /\ channelState = "open"
    /\ rfqState = "none"
    /\ rfqState' = "accepted"
    /\ UNCHANGED <<nativeState, llState, blobState, proofSync, channelState, paymentState, balancesAgree, gap>>

PayInteropInvoice ==
    /\ channelState = "open"
    /\ rfqState = "accepted"
    /\ paymentState = "none"
    /\ paymentState' = "settled"
    /\ balancesAgree' = TRUE
    /\ UNCHANGED <<nativeState, llState, blobState, proofSync, channelState, rfqState, gap>>

DocumentCompatibilityGap ==
    /\ paymentState = "none"
    /\ gap = FALSE
    /\ gap' = TRUE
    /\ paymentState' = "gap"
    /\ balancesAgree' = FALSE
    /\ UNCHANGED <<nativeState, llState, blobState, proofSync, channelState, rfqState>>

TerminalStutter ==
    /\ paymentState \in {"settled", "gap"}
    /\ UNCHANGED vars

Next ==
    \/ DecodeLightningLabsBlobs
    \/ RejectLightningLabsBlobs
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
        /\ blobState = "decoded"
        /\ proofSync = TRUE
        /\ channelState = "open"
        /\ rfqState = "accepted"
        /\ balancesAgree = TRUE

GapIsNotSuccess ==
    gap = TRUE => paymentState = "gap" /\ balancesAgree = FALSE

NoSidecarClaim ==
    nativeState = "ready" /\ llState = LlRole

BlobDecodeIsReadOnly ==
    blobState = "decoded" => nativeState = "ready"

RejectedBlobIsGap ==
    blobState = "rejected" => gap = TRUE /\ paymentState = "gap" /\ balancesAgree = FALSE

====
