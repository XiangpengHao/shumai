use super::{Measure, Measurement};
use perf_event::events::{Hardware, Software};
use perf_event::{Builder, Counter};
use serde::Serialize;
use std::ops::{Add, AddAssign};

macro_rules! perf_builder {
    ($(($name:ident, $event:expr)),+) => {
        pub(crate) struct PerfStatsRaw{
            $($name: Counter,)+
        }

        impl PerfStatsRaw {
            pub(crate) fn new() -> PerfStatsRaw {
                $(let $name = Builder::new()
                    .kind($event)
                    .inherit(true)
                    .build()
                    .expect(&format!("failed to create counter for {}", std::stringify!($name)));
                )+

                PerfStatsRaw{
                    $($name,)+
                }
            }
        }

        #[derive(Debug, Clone, Serialize)]
        pub struct PerfCounter {
            $(pub $name: u64,)+
        }

        impl AddAssign for PerfCounter {
            fn add_assign(&mut self, other: Self) {
                *self = Self {
                    $($name: self.$name + other.$name,)+
                }
            }
        }

        impl Add for PerfCounter {
            type Output = PerfCounter;
            fn add(self, other: PerfCounter)->PerfCounter{
                Self {
                    $($name: self.$name + other.$name,)+
                }
            }
        }

        impl PerfStatsRaw {
            pub(crate) fn get_stats(&mut self) -> std::io::Result<PerfCounter> {
                let stats = PerfCounter {
                    $($name: self.$name.read()?, )+
                };

                Ok(stats)
            }

            pub(crate) fn enable(&mut self) -> std::io::Result<()> {
                $(self.$name.enable()?;)+
                Ok(())
            }

            pub(crate) fn disable(&mut self) -> std::io::Result<()> {
                $(self.$name.disable()?;)+
                Ok(())
            }
        }
    };
}

perf_builder!(
    (cycles, Hardware::CPU_CYCLES),
    (inst, Hardware::INSTRUCTIONS),
    (branch_miss, Hardware::BRANCH_MISSES),
    (branches, Hardware::BRANCH_INSTRUCTIONS),
    (cache_reference, Hardware::CACHE_REFERENCES),
    (cache_miss, Hardware::CACHE_MISSES),
    (bus_cycles, Hardware::BUS_CYCLES),
    (page_faults, Software::PAGE_FAULTS),
    (context_switch, Software::CONTEXT_SWITCHES),
    (cpu_migration, Software::CPU_MIGRATIONS)
);

pub(crate) struct PerfMeasurement {
    stats: PerfStatsRaw,
}

impl PerfMeasurement {
    pub(crate) fn new() -> Self {
        Self {
            stats: PerfStatsRaw::new(),
        }
    }
}

impl Measurement for PerfMeasurement {
    fn start(&mut self) {
        self.stats.enable().expect("unable to enable perf counters");
    }

    fn stop(&mut self) {
        self.stats
            .disable()
            .expect("unable to disable perf counters");
    }

    fn result(&mut self) -> Measure {
        let stats = self.stats.get_stats().expect("unable to get perf counters");

        Measure {
            name: "perf".to_string(),
            value: serde_json::to_value(stats).unwrap(),
        }
    }
}
