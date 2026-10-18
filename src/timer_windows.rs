//! Native `time` decorator accounting. Wall time is monotonic; CPU comes from
//! GetProcessTimes for fish and disjoint tracked Job Object counters for children.
//! Child deltas include concurrent background work within the sampling interval.
//! Descendants contribute only up to each launch group's explicit root-completion
//! cutoff; no POSIX wait/rusage behavior or per-command CPU isolation is implied.

use crate::io::NativeOutput;
use crate::native_accounting::{CpuTimes, current_process_times, tracked_children_cpu};
use std::fmt::Write as _;
use std::io::{self, Write as _};
use std::time::{Duration, Instant};

pub struct TimerSnapshot {
    wall_time: Instant,
    cpu_fish: CpuTimes,
    cpu_children: CpuTimes,
}

/// Same guard call shape as the Unix timer; checked query failures are diagnosed
/// when the guard drops, never panicked or replaced by fabricated counters.
pub fn push_timer() -> PrintElapsedOnDrop {
    PrintElapsedOnDrop {
        start: TimerSnapshot::take(),
    }
}

impl TimerSnapshot {
    /// Counters are sequential observations, not an atomic OS-wide snapshot.
    pub fn take() -> io::Result<Self> {
        Ok(Self {
            wall_time: Instant::now(),
            cpu_fish: current_process_times()?.cpu,
            cpu_children: tracked_children_cpu()?,
        })
    }

    pub fn get_delta(start: &Self, end: &Self, verbose: bool) -> io::Result<String> {
        let wall = end
            .wall_time
            .checked_duration_since(start.wall_time)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "native timer snapshots reversed",
                )
            })?;
        let fish = end.cpu_fish.checked_sub(start.cpu_fish)?;
        let child = end.cpu_children.checked_sub(start.cpu_children)?;
        let total = fish.checked_add(child)?;
        let (wall_time, wall_unit) = unit(wall);
        let (_, cpu_unit) = unit(total.user.max(total.kernel));
        let usr_time = convert(total.user, cpu_unit);
        let sys_time = convert(total.kernel, cpu_unit);
        let mut output = String::new();
        if verbose {
            let (_, fish_unit) = unit(fish.user.max(fish.kernel));
            let (_, child_unit) = unit(child.user.max(child.kernel));
            let width = wall_unit.len().max(cpu_unit.len());
            let fish_width = fish_unit.len() + 7;
            output += "\n________________________________________________________";
            write!(output, "\nExecuted in  {wall_time:6.2} {wall_unit:<width$}    {fish_label:<fish_width$}  external", fish_label = "fish").unwrap();
            write!(output, "\n   usr time  {usr_time:6.2} {cpu_unit:<width$}  {:6.2} {fish_unit}  {:6.2} {child_unit}", convert(fish.user, fish_unit), convert(child.user, child_unit)).unwrap();
            write!(output, "\n   sys time  {sys_time:6.2} {cpu_unit:<width$}  {:6.2} {fish_unit}  {:6.2} {child_unit}", convert(fish.kernel, fish_unit), convert(child.kernel, child_unit)).unwrap();
            output += "\nExternal scope: tracked jobs, including concurrent/nested work until each root-completion cutoff.\n";
        } else {
            output += "\n_______________________________";
            write!(output, "\nExecuted in  {wall_time:6.2} {wall_unit}\n   usr time  {usr_time:6.2} {cpu_unit}\n   sys time  {sys_time:6.2} {cpu_unit}\n").unwrap();
        }
        Ok(output)
    }
}

fn unit(duration: Duration) -> (f64, &'static str) {
    let micros = duration.as_micros();
    let name = if micros > 900_000_000 {
        "mins"
    } else if micros >= 999_995 {
        "secs"
    } else if micros >= 1_000 {
        "millis"
    } else {
        "micros"
    };
    (convert(duration, name), name)
}

fn convert(duration: Duration, unit: &str) -> f64 {
    match unit {
        "mins" => duration.as_secs_f64() / 60.0,
        "secs" => duration.as_secs_f64(),
        "millis" => duration.as_secs_f64() * 1_000.0,
        _ => duration.as_secs_f64() * 1_000_000.0,
    }
}

pub struct PrintElapsedOnDrop {
    start: io::Result<TimerSnapshot>,
}

impl Drop for PrintElapsedOnDrop {
    fn drop(&mut self) {
        let output = match &self.start {
            Ok(start) => {
                TimerSnapshot::take().and_then(|end| TimerSnapshot::get_delta(start, &end, true))
            }
            Err(error) => Err(io::Error::new(error.kind(), error.to_string())),
        };
        let result = match output {
            Ok(output) => {
                NativeOutput::stderr().and_then(|mut stderr| stderr.write_all(output.as_bytes()))
            }
            Err(error) => {
                if let Ok(mut stderr) = NativeOutput::stderr() {
                    let _ = writeln!(
                        stderr,
                        "fish: unable to query native timer accounting: {error}"
                    );
                }
                return;
            }
        };
        if let Err(error) = result {
            if let Ok(mut stderr) = NativeOutput::stderr() {
                let _ = writeln!(
                    stderr,
                    "fish: unable to write native timer accounting: {error}"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timer_deltas_include_shell_and_children_and_reject_regression() {
        let now = Instant::now();
        let start = TimerSnapshot {
            wall_time: now,
            cpu_fish: CpuTimes {
                user: Duration::from_secs(1),
                kernel: Duration::from_secs(2),
            },
            cpu_children: CpuTimes {
                user: Duration::from_secs(3),
                kernel: Duration::from_secs(4),
            },
        };
        let mut end = TimerSnapshot {
            wall_time: now + Duration::from_secs(10),
            cpu_fish: CpuTimes {
                user: Duration::from_secs(2),
                kernel: Duration::from_secs(3),
            },
            cpu_children: CpuTimes {
                user: Duration::from_secs(8),
                kernel: Duration::from_secs(6),
            },
        };
        assert_eq!(
            end.cpu_fish.checked_sub(start.cpu_fish).unwrap().user,
            Duration::from_secs(1)
        );
        assert_eq!(
            end.cpu_children
                .checked_sub(start.cpu_children)
                .unwrap()
                .user,
            Duration::from_secs(5)
        );
        assert!(TimerSnapshot::get_delta(&start, &end, true).is_ok());
        end.cpu_children.user = Duration::ZERO;
        assert!(TimerSnapshot::get_delta(&start, &end, true).is_err());
        assert!(TimerSnapshot::get_delta(&end, &start, false).is_err());
    }
}
