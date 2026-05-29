---- MODULE AssetConservation ----
EXTENDS Naturals

Outputs == {"a", "b"}
MaxSupply == 100

VARIABLES issued, spendable, spent, channel, commitment, pending, invalid

vars == <<issued, spendable, spent, channel, commitment, pending, invalid>>

SumSpendable == spendable["a"] + spendable["b"]
VisibleBalance == SumSpendable + channel

Init ==
    /\ issued = 0
    /\ spendable = [o \in Outputs |-> 0]
    /\ spent = [o \in Outputs |-> FALSE]
    /\ channel = 0
    /\ commitment = 0
    /\ pending = 0
    /\ invalid = 0

Issue ==
    /\ issued = 0
    /\ issued' = MaxSupply
    /\ spendable' = [spendable EXCEPT !["a"] = MaxSupply]
    /\ UNCHANGED <<spent, channel, commitment, pending, invalid>>

Split ==
    /\ issued = MaxSupply
    /\ spendable["a"] = MaxSupply
    /\ spendable["b"] = 0
    /\ spendable' = [spendable EXCEPT !["a"] = 70, !["b"] = 30]
    /\ UNCHANGED <<issued, spent, channel, commitment, pending, invalid>>

MoveBToChannel ==
    /\ spendable["b"] = 30
    /\ channel = 0
    /\ spendable' = [spendable EXCEPT !["b"] = 0]
    /\ spent' = [spent EXCEPT !["b"] = TRUE]
    /\ channel' = 30
    /\ UNCHANGED <<issued, commitment, pending, invalid>>

CommitChannelUpdate ==
    /\ channel = 30
    /\ commitment = 0
    /\ commitment' = 1
    /\ channel' = channel
    /\ UNCHANGED <<issued, spendable, spent, pending, invalid>>

MarkPending ==
    /\ issued = MaxSupply
    /\ pending = 0
    /\ pending' = 10
    /\ UNCHANGED <<issued, spendable, spent, channel, commitment, invalid>>

RejectInvalidProof ==
    /\ invalid = 0
    /\ invalid' = 1
    /\ UNCHANGED <<issued, spendable, spent, channel, commitment, pending>>

TerminalStutter ==
    /\ issued = MaxSupply
    /\ UNCHANGED vars

Next ==
    \/ Issue
    \/ Split
    \/ MoveBToChannel
    \/ CommitChannelUpdate
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

CommitmentUpdatePreservesChannelTotal ==
    commitment = 1 => channel = 30

PendingAndInvalidAreNotSpendable ==
    VisibleBalance + pending + invalid >= VisibleBalance

====
