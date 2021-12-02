use perf_event::events::{Hardware, Software};
use perf_event::{Builder, Counter, Group};
use serde::{Deserialize, Serialize};
use std::ops::{Add, AddAssign};

macro_rules! perf_builder {
    ($(($name:ident, $event:expr)),+) => {
        pub(crate) struct PerfStatsRaw{
            group: Group,
            $($name: Counter,)+
        }

        impl PerfStatsRaw {
            pub(crate) fn new() -> PerfStatsRaw {
                let mut group = Group::new().unwrap();
                $(let $name = Builder::new()
                    .group(&mut group)
                    .kind($event)
                    .build()
                    .unwrap();
                )+

                PerfStatsRaw{
                    group,
                    $($name,)+
                }
            }
        }

        #[derive(Debug, Copy, Clone, Serialize, Deserialize)]
        pub(crate) struct PerfCounter {
            $($name: u64,)+
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
                let raw_counts = self.group.read()?;

                let stats = PerfCounter {
                    $($name: raw_counts[&self.$name], )+
                };

                Ok(stats)
            }
        }
    };
}

impl PerfStatsRaw {
    pub(crate) fn start(&mut self) -> std::io::Result<()> {
        self.group.enable()
    }

    pub(crate) fn stop(&mut self) -> std::io::Result<()> {
        self.group.disable()
    }
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
