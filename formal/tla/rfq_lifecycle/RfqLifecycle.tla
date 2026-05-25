---- MODULE RfqLifecycle ----
EXTENDS Naturals

RealScids == {1, 2}
Alias == 101
QuoteExpiry == 2
InvoiceExpiry == 2

VARIABLES status, now, quoteUsed, liveAliases, alias

vars == <<status, now, quoteUsed, liveAliases, alias>>

Init ==
    /\ status = "none"
    /\ now = 0
    /\ quoteUsed = FALSE
    /\ liveAliases = {}
    /\ alias = 0

RequestQuote ==
    /\ status = "none"
    /\ status' = "requested"
    /\ UNCHANGED <<now, quoteUsed, liveAliases, alias>>

AcceptQuote ==
    /\ status = "requested"
    /\ status' = "accepted"
    /\ alias' = Alias
    /\ liveAliases' = {Alias}
    /\ UNCHANGED <<now, quoteUsed>>

RejectQuote ==
    /\ status = "requested"
    /\ status' = "rejected"
    /\ UNCHANGED <<now, quoteUsed, liveAliases, alias>>

Tick ==
    /\ status = "accepted"
    /\ now < 3
    /\ now' = now + 1
    /\ UNCHANGED <<status, quoteUsed, liveAliases, alias>>

Pay ==
    /\ status = "accepted"
    /\ now <= QuoteExpiry
    /\ now <= InvoiceExpiry
    /\ quoteUsed = FALSE
    /\ status' = "paid"
    /\ quoteUsed' = TRUE
    /\ liveAliases' = {}
    /\ UNCHANGED <<now, alias>>

Expire ==
    /\ status = "accepted"
    /\ now > QuoteExpiry
    /\ status' = "expired"
    /\ liveAliases' = {}
    /\ UNCHANGED <<now, quoteUsed, alias>>

TerminalStutter ==
    /\ status \in {"paid", "rejected", "expired"}
    /\ UNCHANGED vars

Next ==
    \/ RequestQuote
    \/ AcceptQuote
    \/ RejectQuote
    \/ Tick
    \/ Pay
    \/ Expire
    \/ TerminalStutter

Spec == Init /\ [][Next]_vars

QuoteUsedAtMostOnce ==
    quoteUsed => status = "paid"

PaidOnlyBeforeExpiry ==
    status = "paid" => now <= QuoteExpiry /\ now <= InvoiceExpiry

AliasDoesNotCollide ==
    liveAliases \cap RealScids = {}

AcceptedQuoteHasAlias ==
    status = "accepted" => liveAliases = {alias}

InvoiceDoesNotOutliveQuote ==
    InvoiceExpiry <= QuoteExpiry

====
