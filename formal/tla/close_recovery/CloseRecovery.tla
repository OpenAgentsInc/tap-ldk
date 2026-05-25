---- MODULE CloseRecovery ----
EXTENDS Naturals

LatestCommitment == 3
CloseAllocation == 100

VARIABLES status, latestDurableCommitment, proofState, recoveredBalance, sweepState, exportState

vars == <<status, latestDurableCommitment, proofState, recoveredBalance, sweepState, exportState>>

Init ==
    /\ status = "open"
    /\ latestDurableCommitment = LatestCommitment
    /\ proofState = "current"
    /\ recoveredBalance = 0
    /\ sweepState = "none"
    /\ exportState = "none"

CooperativeClose ==
    /\ status = "open"
    /\ proofState = "current"
    /\ status' = "cooperative_closed"
    /\ recoveredBalance' = CloseAllocation
    /\ sweepState' = "not_required"
    /\ UNCHANGED <<latestDurableCommitment, proofState, exportState>>

ForceClose ==
    /\ status = "open"
    /\ proofState = "current"
    /\ status' = "force_closed"
    /\ sweepState' = "pending"
    /\ UNCHANGED <<latestDurableCommitment, proofState, recoveredBalance, exportState>>

LoseProofState ==
    /\ status = "force_closed"
    /\ proofState = "current"
    /\ proofState' = "stale"
    /\ UNCHANGED <<status, latestDurableCommitment, recoveredBalance, sweepState, exportState>>

RestartWithCurrentState ==
    /\ status = "force_closed"
    /\ proofState = "current"
    /\ status' = "recovering"
    /\ UNCHANGED <<latestDurableCommitment, proofState, recoveredBalance, sweepState, exportState>>

RestartWithStaleState ==
    /\ status = "force_closed"
    /\ proofState = "stale"
    /\ status' = "refused"
    /\ UNCHANGED <<latestDurableCommitment, proofState, recoveredBalance, sweepState, exportState>>

SweepSuccess ==
    /\ status = "recovering"
    /\ sweepState = "pending"
    /\ proofState = "current"
    /\ status' = "swept"
    /\ sweepState' = "succeeded"
    /\ recoveredBalance' = CloseAllocation
    /\ UNCHANGED <<latestDurableCommitment, proofState, exportState>>

SweepFailure ==
    /\ status = "recovering"
    /\ sweepState = "pending"
    /\ status' = "refused"
    /\ sweepState' = "failed"
    /\ UNCHANGED <<latestDurableCommitment, proofState, recoveredBalance, exportState>>

ExportProof ==
    /\ status \in {"cooperative_closed", "swept"}
    /\ proofState = "current"
    /\ recoveredBalance = CloseAllocation
    /\ exportState' = "exported"
    /\ UNCHANGED <<status, latestDurableCommitment, proofState, recoveredBalance, sweepState>>

TerminalStutter ==
    /\ status \in {"cooperative_closed", "swept", "refused"}
    /\ exportState \in {"none", "exported"}
    /\ UNCHANGED vars

Next ==
    \/ CooperativeClose
    \/ ForceClose
    \/ LoseProofState
    \/ RestartWithCurrentState
    \/ RestartWithStaleState
    \/ SweepSuccess
    \/ SweepFailure
    \/ ExportProof
    \/ TerminalStutter

Spec == Init /\ [][Next]_vars

RecoveredImpliesCurrentProof ==
    recoveredBalance = CloseAllocation => proofState = "current"

ExportOnlyRecoveredCurrentProof ==
    exportState = "exported" => recoveredBalance = CloseAllocation /\ proofState = "current"

FailedSweepNotRecovered ==
    sweepState = "failed" => recoveredBalance = 0

RefusedIsNotRecovered ==
    status = "refused" => recoveredBalance = 0

LatestCommitmentPreserved ==
    latestDurableCommitment = LatestCommitment

====
