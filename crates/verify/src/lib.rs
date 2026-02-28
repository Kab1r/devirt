//! Abstract dispatch model proving correctness for all N.
//!
//! The macro's dispatch chain has one fundamental pattern:
//! for each hot type in order, call witness; if Some, return it.
//! After all witnesses, call fallback. The four dispatch arms
//! are all instances of this pattern.

use vstd::prelude::*;

verus! {

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SPEC: mathematical definition of correct dispatch
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Index of the first witness that returns Some, if any.
pub closed spec fn first_match(witnesses: Seq<Option<u64>>) -> Option<nat>
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

/// The correct dispatch result: first Some value, or fallback.
pub closed spec fn dispatch_spec(witnesses: Seq<Option<u64>>, fallback: u64) -> u64 {
    match first_match(witnesses) {
        Some(i) => witnesses[i as int].unwrap(),
        None => fallback,
    }
}

} // verus!
