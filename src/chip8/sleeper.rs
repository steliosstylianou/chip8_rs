use log::trace;
use std::thread;
use std::time::{Duration, Instant};

// Use a helper class for accumulating delay values until they reach a certain threshold.
// This way we can compensate for the inaccuracies of sleep() for low values (< 1ms).
// We can also compensate for when the VM runs too slow, by reducing throttling.
#[derive(Debug)]
pub struct Sleeper {
    duty_cycle: Duration,
    threshold: Duration,
    debt: Duration,
    timer: Instant,
}

impl Sleeper {
    pub fn new() -> Self {
        Sleeper {
            threshold: Duration::from_millis(25),
            timer: Instant::now(),
            debt: Duration::ZERO,
            duty_cycle: Duration::ZERO,
        }
    }

    pub fn with_frequency(mut self, hz: u32) -> Self {
        let duty_cycle = Duration::from_secs_f64(1.0 / hz as f64);
        self.duty_cycle = duty_cycle;
        self
    }

    pub fn sleep(&mut self) {
        let elapsed = self.timer.elapsed();
        trace!("Current sleep debt {:?}", self.debt);
        if let Some(result) =  self.duty_cycle.checked_sub(elapsed) {
            // We were too fast, so throttle!
            self.debt += result;
            trace!(
                "Adding {:?} to sleep debt (tgt: {:?}, got: {:?})",
                result,
                self.duty_cycle,
                elapsed
            );
        } else {
            // We were too slow, reduce throttling if needed.
            let slowdown = elapsed - self.duty_cycle;
            if self.debt >= slowdown {
                // Re-adjust sleep debt to compensate for slowness
                trace!(
                    "VM running slow! Reducing sleep debt by {:?} (tgt: {:?}, got: {:?})",
                    slowdown,
                    self.duty_cycle,
                    elapsed
                );    
                self.debt -= slowdown;
            } else {
                // Too slow to compensate using sleep debt so reset to zero
                trace!("VM is REALLY SLOW! Resetting sleep debt to zero...");
                self.debt = Duration::ZERO;
            }
            self.timer = Instant::now();
            return;
        }
        // Time to sleep
        self.sleep_internal();
        self.timer = Instant::now();
    }

    fn sleep_internal(&mut self) {
        if self.debt > self.threshold {
            trace!("Sleeping for {}ms", (self.debt).as_millis());
            thread::sleep(self.debt);
            self.timer = Instant::now();
            self.debt = Duration::ZERO;
        }
    }

}
