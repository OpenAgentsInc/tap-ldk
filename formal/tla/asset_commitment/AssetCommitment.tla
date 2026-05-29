---- MODULE AssetCommitment ----
EXTENDS Naturals, FiniteSets

TotalAsset == 100

VARIABLES commitment, localBalance, remoteBalance, usedNonce, latestRevoked,
          durable, signatureDomain, proofReplay, persistedProofCommitment,
          restartAccepted

vars == <<commitment, localBalance, remoteBalance, usedNonce, latestRevoked,
          durable, signatureDomain, proofReplay, persistedProofCommitment,
          restartAccepted>>

Init ==
    /\ commitment = 0
    /\ localBalance = 70
    /\ remoteBalance = 30
    /\ usedNonce = {}
    /\ latestRevoked = {}
    /\ durable = TRUE
    /\ signatureDomain = "asset"
    /\ proofReplay = 0
    /\ persistedProofCommitment = 0
    /\ restartAccepted = TRUE

MoveLocalToRemote ==
    /\ commitment = 0
    /\ localBalance >= 10
    /\ signatureDomain = "asset"
    /\ proofReplay = commitment
    /\ persistedProofCommitment = commitment
    /\ 1 \notin usedNonce
    /\ commitment' = 1
    /\ localBalance' = localBalance - 10
    /\ remoteBalance' = remoteBalance + 10
    /\ usedNonce' = usedNonce \cup {1}
    /\ latestRevoked' = latestRevoked \cup {commitment}
    /\ durable' = TRUE
    /\ proofReplay' = 1
    /\ persistedProofCommitment' = 1
    /\ restartAccepted' = TRUE
    /\ UNCHANGED signatureDomain

MoveRemoteToLocal ==
    /\ commitment = 0
    /\ remoteBalance >= 5
    /\ signatureDomain = "asset"
    /\ proofReplay = commitment
    /\ persistedProofCommitment = commitment
    /\ 2 \notin usedNonce
    /\ commitment' = 1
    /\ localBalance' = localBalance + 5
    /\ remoteBalance' = remoteBalance - 5
    /\ usedNonce' = usedNonce \cup {2}
    /\ latestRevoked' = latestRevoked \cup {commitment}
    /\ durable' = TRUE
    /\ proofReplay' = 1
    /\ persistedProofCommitment' = 1
    /\ restartAccepted' = TRUE
    /\ UNCHANGED signatureDomain

RejectBtcDomain ==
    /\ signatureDomain = "btc"
    /\ UNCHANGED vars

RefuseRestartWithoutProofReplay ==
    /\ commitment = 0
    /\ proofReplay = 0
    /\ persistedProofCommitment = 0
    /\ 3 \notin usedNonce
    /\ commitment' = 1
    /\ localBalance' = localBalance - 10
    /\ remoteBalance' = remoteBalance + 10
    /\ usedNonce' = usedNonce \cup {3}
    /\ latestRevoked' = latestRevoked \cup {commitment}
    /\ durable' = FALSE
    /\ proofReplay' = 0
    /\ persistedProofCommitment' = 0
    /\ restartAccepted' = FALSE
    /\ UNCHANGED signatureDomain

TerminalStutter ==
    /\ \/ commitment = 1
       \/ restartAccepted = FALSE
    /\ UNCHANGED vars

Next ==
    \/ MoveLocalToRemote
    \/ MoveRemoteToLocal
    \/ RejectBtcDomain
    \/ RefuseRestartWithoutProofReplay
    \/ TerminalStutter

Spec == Init /\ [][Next]_vars

AssetConserved ==
    localBalance + remoteBalance = TotalAsset

CommitmentMonotonic ==
    commitment \in {0, 1}

LatestNotRevoked ==
    commitment \notin latestRevoked

NonceUsedAtMostOnce ==
    Cardinality(usedNonce) <= commitment

DurableLatestState ==
    restartAccepted /\ commitment = 1 => durable = TRUE

AssetDomainOnly ==
    signatureDomain = "asset"

AcceptedRestartHasMatchingProofReplay ==
    restartAccepted => proofReplay = commitment /\ persistedProofCommitment = commitment

StaleProofRestartIsRefused ==
    ~restartAccepted => durable = FALSE /\ persistedProofCommitment < commitment

====
