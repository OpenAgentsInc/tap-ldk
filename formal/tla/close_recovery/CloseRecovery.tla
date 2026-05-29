---- MODULE CloseRecovery ----
EXTENDS Naturals

LatestCommitment == 3
CloseAllocation == 100

VARIABLES status, latestDurableCommitment, proofState, recoveredBalance,
          closeOutputState, secondLevelOutputState, sweepState, exportState,
          proofExportSource

vars == <<status, latestDurableCommitment, proofState, recoveredBalance,
          closeOutputState, secondLevelOutputState, sweepState, exportState,
          proofExportSource>>

Init ==
    /\ status = "open"
    /\ latestDurableCommitment = LatestCommitment
    /\ proofState = "current"
    /\ recoveredBalance = 0
    /\ closeOutputState = "none"
    /\ secondLevelOutputState = "none"
    /\ sweepState = "none"
    /\ exportState = "none"
    /\ proofExportSource = "none"

CooperativeClose ==
    /\ status = "open"
    /\ proofState = "current"
    /\ status' = "cooperative_closed"
    /\ closeOutputState' = "closed"
    /\ recoveredBalance' = CloseAllocation
    /\ sweepState' = "not_required"
    /\ UNCHANGED <<latestDurableCommitment, proofState, secondLevelOutputState,
                  exportState, proofExportSource>>

ForceClose ==
    /\ status = "open"
    /\ proofState = "current"
    /\ status' = "force_closed"
    /\ closeOutputState' = "closed"
    /\ sweepState' = "pending"
    /\ UNCHANGED <<latestDurableCommitment, proofState, recoveredBalance,
                  secondLevelOutputState, exportState, proofExportSource>>

SecondLevelHtlcOutput ==
    /\ status = "force_closed"
    /\ closeOutputState = "closed"
    /\ secondLevelOutputState = "none"
    /\ secondLevelOutputState' = "closed"
    /\ UNCHANGED <<status, latestDurableCommitment, proofState, recoveredBalance,
                  closeOutputState, sweepState, exportState, proofExportSource>>

LoseProofState ==
    /\ status = "force_closed"
    /\ proofState = "current"
    /\ proofState' = "stale"
    /\ UNCHANGED <<status, latestDurableCommitment, recoveredBalance,
                  closeOutputState, secondLevelOutputState, sweepState,
                  exportState, proofExportSource>>

RestartWithCurrentState ==
    /\ status = "force_closed"
    /\ proofState = "current"
    /\ status' = "recovering"
    /\ UNCHANGED <<latestDurableCommitment, proofState, recoveredBalance,
                  closeOutputState, secondLevelOutputState, sweepState,
                  exportState, proofExportSource>>

RestartWithStaleState ==
    /\ status = "force_closed"
    /\ proofState = "stale"
    /\ status' = "refused"
    /\ UNCHANGED <<latestDurableCommitment, proofState, recoveredBalance,
                  closeOutputState, secondLevelOutputState, sweepState,
                  exportState, proofExportSource>>

SweepSuccess ==
    /\ status = "recovering"
    /\ sweepState = "pending"
    /\ proofState = "current"
    /\ closeOutputState = "closed"
    /\ status' = "swept"
    /\ sweepState' = "succeeded"
    /\ recoveredBalance' = CloseAllocation
    /\ UNCHANGED <<latestDurableCommitment, proofState, closeOutputState,
                  secondLevelOutputState, exportState, proofExportSource>>

SweepFailure ==
    /\ status = "recovering"
    /\ sweepState = "pending"
    /\ status' = "refused"
    /\ sweepState' = "failed"
    /\ UNCHANGED <<latestDurableCommitment, proofState, recoveredBalance,
                  closeOutputState, secondLevelOutputState, exportState,
                  proofExportSource>>

ExportProof ==
    /\ status \in {"cooperative_closed", "swept"}
    /\ proofState = "current"
    /\ recoveredBalance = CloseAllocation
    /\ exportState' = "exported"
    /\ proofExportSource' = IF status = "cooperative_closed"
                            THEN "close_output"
                            ELSE "sweep_output"
    /\ UNCHANGED <<status, latestDurableCommitment, proofState, recoveredBalance,
                  closeOutputState, secondLevelOutputState, sweepState>>

TerminalStutter ==
    /\ status \in {"cooperative_closed", "swept", "refused"}
    /\ exportState \in {"none", "exported"}
    /\ UNCHANGED vars

Next ==
    \/ CooperativeClose
    \/ ForceClose
    \/ SecondLevelHtlcOutput
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

SecondLevelRequiresCloseOutput ==
    secondLevelOutputState = "closed" => closeOutputState = "closed"

SweepRequiresCloseOutput ==
    sweepState = "succeeded" => closeOutputState = "closed" /\ proofState = "current"

ExportReferencesActualOutput ==
    exportState = "exported" =>
        \/ /\ proofExportSource = "close_output"
           /\ status = "cooperative_closed"
           /\ closeOutputState = "closed"
        \/ /\ proofExportSource = "sweep_output"
           /\ status = "swept"
           /\ sweepState = "succeeded"

====
