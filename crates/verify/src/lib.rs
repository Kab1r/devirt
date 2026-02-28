use vstd::prelude::*;

verus! {

/// Smoke test: Verus can verify a trivial assertion.
fn smoke_test() {
    assert(1u64 == 0u64 + 1u64);
}

} // verus!
