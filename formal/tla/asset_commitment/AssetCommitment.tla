---- MODULE AssetCommitment ----
EXTENDS Naturals, FiniteSets

TotalAsset == 100

VARIABLES commitment, localBalance, remoteBalance, usedNonce, latestRevoked, durable, signatureDomain

vars == <<commitment, localBalance, remoteBalance, usedNonce, latestRevoked, durable, signatureDomain>>

Init ==
    /\ commitment = 0
    /\ localBalance = 70
    /\ remoteBalance = 30
    /\ usedNonce = {}
    /\ latestRevoked = {}
    /\ durable = TRUE
    /\ signatureDomain = "asset"

MoveLocalToRemote ==
    /\ commitment = 0
    /\ localBalance >= 10
    /\ signatureDomain = "asset"
    /\ 1 \notin usedNonce
    /\ commitment' = 1
    /\ localBalance' = localBalance - 10
    /\ remoteBalance' = remoteBalance + 10
    /\ usedNonce' = usedNonce \cup {1}
    /\ latestRevoked' = latestRevoked \cup {commitment}
    /\ durable' = TRUE
    /\ UNCHANGED signatureDomain

MoveRemoteToLocal ==
    /\ commitment = 0
    /\ remoteBalance >= 5
    /\ signatureDomain = "asset"
    /\ 2 \notin usedNonce
    /\ commitment' = 1
    /\ localBalance' = localBalance + 5
    /\ remoteBalance' = remoteBalance - 5
    /\ usedNonce' = usedNonce \cup {2}
    /\ latestRevoked' = latestRevoked \cup {commitment}
    /\ durable' = TRUE
    /\ UNCHANGED signatureDomain

RejectBtcDomain ==
    /\ signatureDomain = "btc"
    /\ UNCHANGED vars

TerminalStutter ==
    /\ commitment = 1
    /\ UNCHANGED vars

Next ==
    \/ MoveLocalToRemote
    \/ MoveRemoteToLocal
    \/ RejectBtcDomain
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
    commitment = 1 => durable = TRUE

AssetDomainOnly ==
    signatureDomain = "asset"

====
