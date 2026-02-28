//! Abstract dispatch model proving correctness for all N.
//!
//! The macro's dispatch chain has one fundamental pattern:
//! for each hot type in order, call witness; if Some, return it.
//! After all witnesses, call fallback. The four dispatch arms
//! are all instances of this pattern.
//!
//! `dispatch_spec` is defined directly recursive to mirror the
//! macro's unrolled chain, making inductive proofs natural.
//! `first_match` is a separate spec used by Property A.

use vstd::prelude::*;

verus! {

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SPEC: mathematical definition of correct dispatch
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The correct dispatch result: first Some value, or fallback.
/// Directly recursive to mirror the macro's unrolled dispatch chain.
pub open spec fn dispatch_spec(witnesses: Seq<Option<u64>>, fallback: u64) -> u64
    decreases witnesses.len(),
{
    if witnesses.len() == 0 {
        fallback
    } else if witnesses[0].is_some() {
        witnesses[0].unwrap()
    } else {
        dispatch_spec(witnesses.subrange(1, witnesses.len() as int), fallback)
    }
}

/// Index of the first witness that returns Some, if any.
pub open spec fn first_match(witnesses: Seq<Option<u64>>) -> Option<nat>
    decreases witnesses.len(),
{
    if witnesses.len() == 0 {
        None
    } else if witnesses[0].is_some() {
        Some(0)
    } else {
        match first_match(witnesses.subrange(1, witnesses.len() as int)) {
            Some(i) => Some(i + 1),
            None => None,
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PROOF: iterative dispatch equivalence
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Proof that skipping idx None witnesses from the front does not change
/// the dispatch result. This models the macro's unrolled chain: each
/// `if let Some(v) = witness { return v; }` step either returns early
/// or advances to the next witness.
pub proof fn dispatch_skip_nones(witnesses: Seq<Option<u64>>, fallback: u64, idx: nat)
    requires
        idx <= witnesses.len(),
        forall|j: int| 0 <= j < idx as int ==> witnesses[j].is_none(),
    ensures
        dispatch_spec(witnesses, fallback)
            == dispatch_spec(
                witnesses.subrange(idx as int, witnesses.len() as int),
                fallback,
            ),
    decreases idx,
{
    if idx == 0 {
        // Base case: subrange(0, len) is the same as witnesses.
        assert(witnesses.subrange(0, witnesses.len() as int) =~= witnesses);
    } else {
        let tail = witnesses.subrange(1, witnesses.len() as int);

        // Establish the precondition for the recursive call:
        // all elements of tail before idx-1 are None.
        assert forall|j: int| 0 <= j < ((idx - 1) as int)
            implies tail[j].is_none() by
        {
            // tail[j] == witnesses[j + 1], and j + 1 < idx, so it's None.
            assert(tail[j] == witnesses[j + 1]);
        }

        // Inductive hypothesis: dispatch_spec(tail, fb) ==
        // dispatch_spec(tail[(idx-1)..], fb).
        dispatch_skip_nones(tail, fallback, (idx - 1) as nat);

        // Relate the subranges: tail[(idx-1)..] == witnesses[idx..].
        assert(tail.subrange((idx - 1) as int, tail.len() as int)
            =~= witnesses.subrange(idx as int, witnesses.len() as int));

        // Unfold dispatch_spec one step: witnesses[0] is None, so
        // dispatch_spec(witnesses, fb) == dispatch_spec(tail, fb).
        assert(witnesses[0].is_none());
        assert(!witnesses[0].is_some());
    }
}

} // verus!
