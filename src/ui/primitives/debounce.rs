use floem::action::exec_after;
use std::sync::atomic::{AtomicU64, Ordering};
use std::{sync::Arc, time::Duration};

#[derive(Debug, Clone)]
pub struct Debounce {
	duration: Duration,
	counter: Arc<AtomicU64>,
}

impl Default for Debounce {
	fn default() -> Self {
		Debounce {
			duration: Duration::from_millis(100),
			counter: Arc::new(AtomicU64::new(0)),
		}
	}
}

impl Debounce {
	pub fn add(self, action: impl Fn() + 'static) {
		let mut call_counter = self.counter.fetch_add(1, Ordering::Relaxed);
		call_counter += 1;
		exec_after(self.duration, move |_| {
			// If the counter set in this call is the same as in Debounce then it is the latest within duration.
			if self.counter.load(Ordering::Relaxed) == call_counter {
				// Reset the counter
				self.counter.store(0, Ordering::Relaxed);
				action();
			}
		});
	}
}
