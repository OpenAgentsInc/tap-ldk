---- MODULE AssetHtlc ----
EXTENDS Naturals

TotalAsset == 100
HtlcAsset == 10
HtlcBtcMsat == 1000
QuoteBtcMsat == 1000

VARIABLES status, quoteState, htlcState, localBalance, remoteBalance, inFlightAsset, durable, revoked

vars == <<status, quoteState, htlcState, localBalance, remoteBalance, inFlightAsset, durable, revoked>>

Init ==
    /\ status = "open"
    /\ quoteState = "none"
    /\ htlcState = "none"
    /\ localBalance = TotalAsset
    /\ remoteBalance = 0
    /\ inFlightAsset = 0
    /\ durable = TRUE
    /\ revoked = FALSE

AcceptQuote ==
    /\ status = "open"
    /\ quoteState = "none"
    /\ quoteState' = "accepted"
    /\ UNCHANGED <<status, htlcState, localBalance, remoteBalance, inFlightAsset, durable, revoked>>

AddHtlc ==
    /\ status = "open"
    /\ quoteState = "accepted"
    /\ htlcState = "none"
    /\ HtlcBtcMsat = QuoteBtcMsat
    /\ localBalance >= HtlcAsset
    /\ htlcState' = "offered"
    /\ localBalance' = localBalance - HtlcAsset
    /\ inFlightAsset' = HtlcAsset
    /\ durable' = TRUE
    /\ UNCHANGED <<status, quoteState, remoteBalance, revoked>>

SettleHtlc ==
    /\ status = "open"
    /\ htlcState = "offered"
    /\ revoked = FALSE
    /\ htlcState' = "settled"
    /\ remoteBalance' = remoteBalance + inFlightAsset
    /\ inFlightAsset' = 0
    /\ quoteState' = "used"
    /\ UNCHANGED <<status, localBalance, durable, revoked>>

FailHtlc ==
    /\ status = "open"
    /\ htlcState = "offered"
    /\ htlcState' = "failed"
    /\ localBalance' = localBalance + inFlightAsset
    /\ inFlightAsset' = 0
    /\ quoteState' = "failed"
    /\ UNCHANGED <<status, remoteBalance, durable, revoked>>

RevokeOfferedState ==
    /\ status = "open"
    /\ htlcState = "offered"
    /\ revoked' = TRUE
    /\ htlcState' = "revoked"
    /\ localBalance' = localBalance + inFlightAsset
    /\ inFlightAsset' = 0
    /\ quoteState' = "failed"
    /\ UNCHANGED <<status, remoteBalance, durable>>

TerminalStutter ==
    /\ htlcState \in {"settled", "failed", "revoked"}
    /\ UNCHANGED vars

Next ==
    \/ AcceptQuote
    \/ AddHtlc
    \/ SettleHtlc
    \/ FailHtlc
    \/ RevokeOfferedState
    \/ TerminalStutter

Spec == Init /\ [][Next]_vars

AssetConserved ==
    localBalance + remoteBalance + inFlightAsset = TotalAsset

HtlcOnlyWithAcceptedQuote ==
    htlcState = "offered" => quoteState = "accepted"

SettledOnlyWithQuoteBtcAmount ==
    htlcState = "settled" => HtlcBtcMsat = QuoteBtcMsat

RevokedStateCannotSettle ==
    revoked = TRUE => htlcState # "settled"

DurableBeforeOffered ==
    htlcState = "offered" => durable = TRUE

====
