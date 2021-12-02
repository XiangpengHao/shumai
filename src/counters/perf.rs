use perf_event::events::{Hardware, Software};
use perf_event::{Builder, Counter};
use serde::{Deserialize, Serialize};
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
                    .build()
                    .expect(&format!("failed to create counter for {}", std::stringify!($name)));
                )+

                PerfStatsRaw{
                    $($name,)+
                }
            }
        }

        #[derive(Debug, Copy, Clone, Serialize, Deserialize)]
        pub struct PerfCounter {
            $(pub $name: u64,)+
        }

        impl PerfCounter{
            pub(crate) fn new() -> Self {
                Self {
                    $($name: 0,)+
                }
            }
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
