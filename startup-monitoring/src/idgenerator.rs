use rand::{rngs, Rng};
use std::cell::RefCell;
use opentelemetry::sdk::trace;
use opentelemetry::trace::{SpanId, TraceId};

/// Generates Trace and Span ids using a random number generator
/// with at most 64 bit. leaves the top of a traceid 64bit empty. We use this,
/// as our tools do not support 128bit trace ids.
#[derive(Clone, Debug, Default)]
pub struct IdGenerator64;

impl trace::IdGenerator for IdGenerator64 {
    /// Generate new `TraceId` using thread local rng
    fn new_trace_id(&self) -> TraceId {
        CURRENT_RNG.with(|rng| {
            let mut bytes = [0u8; 16];

            // fill only the last 8 byte of the id with trace id
            rng.borrow_mut().fill(&mut bytes[8..]);

            TraceId::from(bytes)
        })
    }

    /// Generate new `SpanId` using thread local rng
    fn new_span_id(&self) -> SpanId {
        CURRENT_RNG.with(|rng| SpanId::from(rng.borrow_mut().gen::<[u8; 8]>()))
    }
}

thread_local! {
    /// Store random number generator for each thread
    static CURRENT_RNG: RefCell<rngs::ThreadRng> = RefCell::new(rngs::ThreadRng::default());
}
