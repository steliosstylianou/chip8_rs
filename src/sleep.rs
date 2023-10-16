use log::{debug, trace};
use std::thread;
use std::time::{Duration, Instant};

pub struct Sleeper {
    sleep_duty_cycle: Duration,
    sleep_threshold: Duration,
    sleep_debt: Duration,
    sleep_timer: Instant,
}

impl Sleeper {
    pub fn new(duty_cycle: Duration) -> Self {
        Sleeper {
            sleep_threshold: Duration::from_millis(25),
            sleep_timer: Instant::now(),
            sleep_debt: Duration::ZERO,
            sleep_duty_cycle: duty_cycle,
        }
    }

    pub fn sleep(&mut self) {
        trace!("Current sleep debt {:?}", self.sleep_debt);
        let elapsed = self.sleep_timer.elapsed();
        if let Some(result) =  self.sleep_duty_cycle.checked_sub(elapsed) {
            // We were too fast, so throttle!
            self.sleep_debt += result;
            trace!(
                "Adding {:?} to sleep debt (tgt: {:?}, got: {:?})",
                result,
                self.sleep_duty_cycle,
                elapsed
            );
        } else {
            // We were too slow, reduce throttling if needed.
            let slowdown = elapsed - self.sleep_duty_cycle;
            if self.sleep_debt >= slowdown {
                // Re-adjust sleep debt to compensate for slowness
                trace!(
                    "VM running slow! Reducing sleep debt by {:?} (tgt: {:?}, got: {:?})",
                    slowdown,
                    self.sleep_duty_cycle,
                    elapsed
                );    
                self.sleep_debt -= slowdown;
            } else {
                // Too slow to compensate using sleep debt so reset to zero
                trace!("VM is REALLY SLOW! Resetting sleep debt to zero...");
                self.sleep_debt = Duration::ZERO;
            }
            self.sleep_timer = Instant::now();
            return;
        }
        // Time to sleep
        self.sleep_internal();
        self.sleep_timer = Instant::now();
    }

    fn sleep_internal(&mut self) {
        if self.sleep_debt > self.sleep_threshold {
            debug!("Sleeping for {}ms", (self.sleep_debt).as_millis());
            thread::sleep(self.sleep_debt);
            self.sleep_timer = Instant::now();
            self.sleep_debt = Duration::ZERO;
        }
    }

}
