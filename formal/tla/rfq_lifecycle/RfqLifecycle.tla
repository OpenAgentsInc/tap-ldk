---- MODULE RfqLifecycle ----
EXTENDS Naturals

RealScids == {1, 2}
Alias == 101
QuoteExpiry == 2
InvoiceExpiry == 2

VARIABLES status, now, quoteUsed, invoiceBound, liveAliases, alias

vars == <<status, now, quoteUsed, invoiceBound, liveAliases, alias>>

Init ==
    /\ status = "none"
    /\ now = 0
    /\ quoteUsed = FALSE
    /\ invoiceBound = FALSE
    /\ liveAliases = {}
    /\ alias = 0

RequestQuote ==
    /\ status = "none"
    /\ status' = "requested"
    /\ UNCHANGED <<now, quoteUsed, invoiceBound, liveAliases, alias>>

AcceptQuote ==
    /\ status = "requested"
    /\ status' = "accepted"
    /\ alias' = Alias
    /\ liveAliases' = {Alias}
    /\ UNCHANGED <<now, quoteUsed, invoiceBound>>

RejectQuote ==
    /\ status = "requested"
    /\ status' = "rejected"
    /\ UNCHANGED <<now, quoteUsed, invoiceBound, liveAliases, alias>>

BindInvoice ==
    /\ status = "accepted"
    /\ now <= QuoteExpiry
    /\ InvoiceExpiry <= QuoteExpiry
    /\ status' = "invoiced"
    /\ invoiceBound' = TRUE
    /\ UNCHANGED <<now, quoteUsed, liveAliases, alias>>

Tick ==
    /\ status \in {"accepted", "invoiced"}
    /\ now < 3
    /\ now' = now + 1
    /\ UNCHANGED <<status, quoteUsed, invoiceBound, liveAliases, alias>>

Pay ==
    /\ status = "invoiced"
    /\ now <= QuoteExpiry
    /\ now <= InvoiceExpiry
    /\ invoiceBound = TRUE
    /\ quoteUsed = FALSE
    /\ status' = "paid"
    /\ quoteUsed' = TRUE
    /\ liveAliases' = {}
    /\ UNCHANGED <<now, invoiceBound, alias>>

Expire ==
    /\ status \in {"accepted", "invoiced"}
    /\ now > QuoteExpiry
    /\ status' = "expired"
    /\ liveAliases' = {}
    /\ UNCHANGED <<now, quoteUsed, invoiceBound, alias>>

TerminalStutter ==
    /\ status \in {"paid", "rejected", "expired"}
    /\ UNCHANGED vars

Next ==
    \/ RequestQuote
    \/ AcceptQuote
    \/ RejectQuote
    \/ BindInvoice
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

LiveQuoteHasAlias ==
    status \in {"accepted", "invoiced"} => liveAliases = {alias}

InvoiceBoundBeforePay ==
    status = "paid" => invoiceBound = TRUE

InvoiceDoesNotOutliveQuote ==
    InvoiceExpiry <= QuoteExpiry

====
