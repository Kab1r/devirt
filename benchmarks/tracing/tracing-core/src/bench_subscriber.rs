//! A simple subscriber for devirt benchmarking.
//!
//! `BenchSubscriber` simulates a realistic subscriber (level filtering,
//! span ID generation) while remaining lightweight enough that dispatch
//! overhead is measurable.

use crate::{span, Event, LevelFilter, Metadata};
#[cfg(feature = "devirt-bench")]
use crate::subscriber::__SubscriberImpl;
use crate::subscriber::{Interest, Subscriber};
use core::sync::atomic::{AtomicU64, Ordering};

/// A subscriber that filters by level and generates span IDs.
/// Models the behavior of `tracing_subscriber::fmt::FmtSubscriber`.
#[derive(Debug)]
pub struct BenchSubscriber {
    min_level: LevelFilter,
    next_id: AtomicU64,
}

impl BenchSubscriber {
    /// Create a new `BenchSubscriber` that accepts events at or above `min_level`.
    pub fn new(min_level: LevelFilter) -> Self {
        Self {
            min_level,
            next_id: AtomicU64::new(1),
        }
    }
}

#[cfg_attr(feature = "devirt-bench", devirt::devirt)]
impl Subscriber for BenchSubscriber {
    fn register_callsite(&self, _meta: &'static Metadata<'static>) -> Interest {
        Interest::sometimes()
    }

    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        *metadata.level() <= self.min_level
    }

    fn max_level_hint(&self) -> Option<LevelFilter> {
        Some(self.min_level)
    }

    fn new_span(&self, _span: &span::Attributes<'_>) -> span::Id {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        span::Id::from_u64(id)
    }

    fn record(&self, _span: &span::Id, _values: &span::Record<'_>) {}

    fn record_follows_from(&self, _span: &span::Id, _follows: &span::Id) {}

    fn event(&self, _event: &Event<'_>) {}

    fn enter(&self, _span: &span::Id) {}

    fn exit(&self, _span: &span::Id) {}
}
