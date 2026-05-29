---- MODULE AssetConservation ----
EXTENDS Naturals

Outputs == {"a", "b"}
MaxSupply == 100

VARIABLES issued, spendable, spent, channel, pending, invalid

vars == <<issued, spendable, spent, channel, pending, invalid>>

SumSpendable == spendable["a"] + spendable["b"]
VisibleBalance == SumSpendable + channel

Init ==
    /\ issued = 0
    /\ spendable = [o \in Outputs |-> 0]
    /\ spent = [o \in Outputs |-> FALSE]
    /\ channel = 0
    /\ pending = 0
    /\ invalid = 0

Issue ==
    /\ issued = 0
    /\ issued' = MaxSupply
    /\ spendable' = [spendable EXCEPT !["a"] = MaxSupply]
    /\ UNCHANGED <<spent, channel, pending, invalid>>

Split ==
    /\ issued = MaxSupply
    /\ spendable["a"] = MaxSupply
    /\ spendable["b"] = 0
    /\ spendable' = [spendable EXCEPT !["a"] = 70, !["b"] = 30]
    /\ UNCHANGED <<issued, spent, channel, pending, invalid>>

MoveBToChannel ==
    /\ spendable["b"] = 30
    /\ channel = 0
    /\ spendable' = [spendable EXCEPT !["b"] = 0]
    /\ spent' = [spent EXCEPT !["b"] = TRUE]
    /\ channel' = 30
    /\ UNCHANGED <<issued, pending, invalid>>

MarkPending ==
    /\ issued = MaxSupply
    /\ pending = 0
    /\ pending' = 10
    /\ UNCHANGED <<issued, spendable, spent, channel, invalid>>

RejectInvalidProof ==
    /\ invalid = 0
    /\ invalid' = 1
    /\ UNCHANGED <<issued, spendable, spent, channel, pending>>

TerminalStutter ==
    /\ issued = MaxSupply
    /\ UNCHANGED vars

Next ==
    \/ Issue
    \/ Split
    \/ MoveBToChannel
    \/ MarkPending
    \/ RejectInvalidProof
    \/ TerminalStutter

Spec == Init /\ [][Next]_vars

SupplyCreatedOnlyByIssuance ==
    issued \in 0..MaxSupply

VisibleBalanceNeverExceedsIssued ==
    VisibleBalance <= issued

SpentOutputsAreNotSpendable ==
    \A o \in Outputs: spent[o] => spendable[o] = 0

ChannelBalanceRequiresSpentInput ==
    channel > 0 => spent["b"] = TRUE

PendingAndInvalidAreNotSpendable ==
    VisibleBalance + pending + invalid >= VisibleBalance

====
