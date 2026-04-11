//! Abstract dispatch model proving correctness for all N.
//!
//! The macro's dispatch chain has one fundamental pattern:
//! for each hot type in order, check a predicate; if it matches,
//! return the corresponding value. After all hot types, call
//! fallback. The four dispatch arms are all instances of this
//! pattern.
//!
//! `dispatch_spec` is defined directly recursive to mirror the
//! macro's unrolled chain, making inductive proofs natural.
//! `first_match` is a separate spec used by Property A.
//!
//! # Relation to vtable-pointer comparison
//!
//! The new dispatch mechanism compares the runtime vtable pointer
//! against compile-time-known vtable addresses for each hot type,
//! rather than calling witness methods that return `Option`. The
//! abstract model is agnostic to the comparison mechanism: whether
//! the "witness" at index `i` is `Some(v)` depends only on
//! whether the runtime vtable equals `hot_vts[i]`. The refinement
//! lemma `vtable_refines_witness` at the bottom of this file
//! proves that `vtable_dispatch_spec` (which models the new
//! implementation directly) is equivalent to `dispatch_spec`
//! (the existing abstract spec), so Properties A, B, and C
//! transfer to the new implementation without reproof.

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

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PROOF: properties that follow from the spec
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Property A: First-match-wins — the index returned by first_match is
/// within bounds, and all witnesses before it are None.
pub proof fn first_match_is_earliest(witnesses: Seq<Option<u64>>)
    ensures
        forall|i: nat| first_match(witnesses) == Some(i)
            ==> (i as int) < witnesses.len(),
        forall|i: nat| first_match(witnesses) == Some(i)
            ==> forall|j: int| 0 <= j < i as int
                ==> witnesses[j].is_none(),
    decreases witnesses.len(),
{
    if witnesses.len() > 0 && witnesses[0].is_none() {
        let tail = witnesses.subrange(1, witnesses.len() as int);
        first_match_is_earliest(tail);

        // Connect tail's property to witnesses. For first_match(witnesses)
        // == Some(k), we have k >= 1 and first_match(tail) == Some(k-1).
        // For j < k: j==0 is covered by witnesses[0].is_none(); j>0 uses
        // tail[j-1] == witnesses[j] and the inductive hypothesis.
        assert forall|i: nat, j: int|
            first_match(witnesses) == Some(i) && 0 <= j && j < i as int
            implies witnesses[j].is_none()
        by {
            if j == 0 {
                assert(witnesses[0].is_none());
            } else {
                assert(tail[j - 1] == witnesses[j]);
            }
        }
    }
}

/// Property B: Exhaustive fallback — if all witnesses are None, result is fallback.
pub proof fn fallback_always_fires(witnesses: Seq<Option<u64>>, fallback: u64)
    requires
        forall|i: int| 0 <= i < witnesses.len()
            ==> witnesses[i].is_none(),
    ensures
        dispatch_spec(witnesses, fallback) == fallback,
    decreases witnesses.len(),
{
    if witnesses.len() > 0 {
        fallback_always_fires(
            witnesses.subrange(1, witnesses.len() as int),
            fallback,
        );
    }
}

/// Property C: Hot dispatch returns the correct value — if witness i is
/// Some(val) and all prior witnesses are None, dispatch returns val.
pub proof fn hot_dispatch_correct(
    witnesses: Seq<Option<u64>>,
    fallback: u64,
    hot_idx: nat,
    val: u64,
)
    requires
        (hot_idx as int) < witnesses.len(),
        witnesses[hot_idx as int] == Some(val),
        forall|j: int| 0 <= j < hot_idx as int
            ==> witnesses[j].is_none(),
    ensures
        dispatch_spec(witnesses, fallback) == val,
    decreases witnesses.len(),
{
    if hot_idx > 0 {
        hot_dispatch_correct(
            witnesses.subrange(1, witnesses.len() as int),
            fallback,
            (hot_idx - 1) as nat,
            val,
        );
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// VTABLE-COMPARISON REFINEMENT
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//
// Model of the new dispatch implementation: instead of a sequence
// of `Option<u64>` witnesses, we have a runtime vtable pointer
// and a sequence of hot vtables with corresponding values. A
// match at index `i` fires when `vt == hot_vts[i]`, returning
// `values[i]`. No match falls through to `fallback`.
//
// The refinement lemma `vtable_refines_witness` proves this is
// equivalent to `dispatch_spec` on the "projected" witnesses
// sequence, so the existing Property A/B/C proofs apply without
// modification.

/// Direct recursive model of the vtable-comparison dispatch.
pub open spec fn vtable_dispatch_spec(
    vt: u64,
    hot_vts: Seq<u64>,
    values: Seq<u64>,
    fallback: u64,
) -> u64
    decreases hot_vts.len(),
{
    if hot_vts.len() == 0 {
        fallback
    } else if hot_vts[0] == vt {
        values[0]
    } else {
        vtable_dispatch_spec(
            vt,
            hot_vts.subrange(1, hot_vts.len() as int),
            values.subrange(1, values.len() as int),
            fallback,
        )
    }
}

/// Project a vtable lookup into the witness-sequence encoding:
/// for each hot-type index `i`, the "witness" is `Some(values[i])`
/// iff the runtime `vt` matches `hot_vts[i]`, else `None`.
pub open spec fn project_witnesses(
    vt: u64,
    hot_vts: Seq<u64>,
    values: Seq<u64>,
) -> Seq<Option<u64>>
    decreases hot_vts.len(),
{
    if hot_vts.len() == 0 {
        Seq::empty()
    } else {
        let head: Option<u64> = if hot_vts[0] == vt {
            Some(values[0])
        } else {
            None
        };
        seq![head].add(project_witnesses(
            vt,
            hot_vts.subrange(1, hot_vts.len() as int),
            values.subrange(1, values.len() as int),
        ))
    }
}

/// Refinement lemma: the vtable-comparison dispatch is equivalent
/// to the abstract witness-sequence dispatch under the projection
/// `project_witnesses`. This lets the existing Property A/B/C
/// proofs (`first_match_is_earliest`, `fallback_always_fires`,
/// `hot_dispatch_correct`) apply to the new implementation.
pub proof fn vtable_refines_witness(
    vt: u64,
    hot_vts: Seq<u64>,
    values: Seq<u64>,
    fallback: u64,
)
    requires
        hot_vts.len() == values.len(),
    ensures
        vtable_dispatch_spec(vt, hot_vts, values, fallback)
            == dispatch_spec(
                project_witnesses(vt, hot_vts, values),
                fallback,
            ),
    decreases hot_vts.len(),
{
    if hot_vts.len() == 0 {
        // Both sides evaluate to `fallback` on an empty sequence.
        assert(project_witnesses(vt, hot_vts, values) =~= Seq::empty());
    } else {
        let tail_vts = hot_vts.subrange(1, hot_vts.len() as int);
        let tail_vals = values.subrange(1, values.len() as int);

        // Inductive hypothesis on the tail.
        vtable_refines_witness(vt, tail_vts, tail_vals, fallback);

        let witnesses = project_witnesses(vt, hot_vts, values);

        // Unfold the projection one step and observe that its tail
        // matches the projection of the tail sequences.
        assert(witnesses.len() > 0);
        assert(witnesses.subrange(1, witnesses.len() as int)
            =~= project_witnesses(vt, tail_vts, tail_vals));

        if hot_vts[0] == vt {
            // Head witness is `Some(values[0])`, so the abstract
            // dispatch returns it immediately. Direct by unfolding.
            assert(witnesses[0] == Some(values[0]));
        } else {
            // Head witness is `None`, so abstract dispatch recurses
            // on the tail, which matches the IH.
            assert(witnesses[0].is_none());
        }
    }
}

} // verus!
