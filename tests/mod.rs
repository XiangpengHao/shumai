use serde_json::{json, Value};
use shumai::{config, Context, ShumaiBench, ShumaiResult};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Workload {
    A,
    B,
}

#[config(path = "tests/benchmark.toml")]
pub struct Foo {
    pub name: String,
    pub threads: Vec<usize>,
    pub time: usize,
    #[matrix]
    pub a: usize,
}

#[config(path = "tests/benchmark.toml")]
pub struct Bar {
    pub name: String,
    pub threads: Vec<usize>,
    pub time: usize,
    pub workload: Workload,
}

#[derive(Debug, PartialEq)]
enum ExecutionSeq {
    Load,
    Run,
    IterationFinished,
    ThreadFinished,
    Cleanup,
}

#[derive(Default)]
struct TestBench {
    execution_queue: crossbeam::queue::SegQueue<ExecutionSeq>,
}

impl ShumaiBench for TestBench {
    type Result = usize;
    type Config = Foo;

    fn load(&mut self) -> Option<Value> {
        self.execution_queue.push(ExecutionSeq::Load);
        Some(json!({"load_finished": true}))
    }

    fn run(&self, context: Context<Foo>) -> Self::Result {
        context.wait_for_start();
        let mut sum = 0;
        while context.is_running() {
            sum += context.config.a;
        }
        self.execution_queue.push(ExecutionSeq::Run);
        sum
    }

    fn on_iteration_finished(&mut self, _cur_iter: usize) {
        self.execution_queue.push(ExecutionSeq::IterationFinished);
    }

    fn on_thread_finished(&mut self, _cur_thread: usize) {
        self.execution_queue.push(ExecutionSeq::ThreadFinished);
    }

    fn cleanup(&mut self) -> Option<Value> {
        self.execution_queue.push(ExecutionSeq::Cleanup);
        Some(json!({"cleanup_finished": true}))
    }
}

#[test]
fn config() {
    let config = Foo::load().expect("Failed to parse config!");

    assert_eq!(config.len(), 2);
    for (i, c) in config.iter().enumerate() {
        assert_eq!(c.threads, vec![1, 2, 3]);
        assert_eq!(c.time, 1);
        assert_eq!(c.a, i + 1);
    }

    let config = Foo::load_with_filter("foo-2").expect("Failed to parse config");
    assert_eq!(config.len(), 1);
}

#[test]
#[should_panic(expected = "Failed to parse config!")]
fn empty_config() {
    Bar::load().expect("Failed to parse config!");
}

#[test]
#[cfg_attr(miri, ignore)]
fn runner() {
    let config = Foo::load().expect("Failed to parse config!");
    let repeat = 2;

    for c in config.iter() {
        let mut benchmark = TestBench::default();
        let _result = shumai::run(&mut benchmark, c, repeat);

        let mut gt = Vec::new();
        gt.push(ExecutionSeq::Load);
        for thread in c.threads.iter() {
            for _j in 0..repeat {
                for _i in 0..*thread {
                    gt.push(ExecutionSeq::Run);
                }
                gt.push(ExecutionSeq::IterationFinished);
            }
            gt.push(ExecutionSeq::ThreadFinished);
        }
        gt.push(ExecutionSeq::Cleanup);

        let mut rv_seq = Vec::new();

        while benchmark.execution_queue.len() > 0 {
            rv_seq.push(benchmark.execution_queue.pop().unwrap())
        }

        assert_eq!(rv_seq.len(), gt.len());
        for i in 0..rv_seq.len() {
            assert_eq!(gt[i], rv_seq[i]);
        }
    }
}

#[test]
fn check_load_cleanup_result() {
    let config = Foo::load().expect("Failed to parse config!");
    let repeat = 1;

    for c in config.iter() {
        let mut benchmark = TestBench::default();
        let result = shumai::run(&mut benchmark, c, repeat);

        assert_eq!(
            "true",
            result.load_results.unwrap()["load_finished"].to_string()
        );
        assert_eq!(
            "true",
            result.cleanup_results.unwrap()["cleanup_finished"].to_string()
        );
    }
}

#[test]
#[cfg_attr(miri, ignore)]
fn write_json() {
    let config = Foo::load().expect("Failed to parse config!");
    let repeat = 1;

    for c in config.iter() {
        let mut benchmark = TestBench::default();
        let result = shumai::run(&mut benchmark, c, repeat);
        let file_path = result.write_json().unwrap();

        let written_data = std::fs::read_to_string(file_path).unwrap();
        let result: ShumaiResult<Foo, usize> = serde_json::from_str(&written_data).unwrap();
        assert_eq!(result.config.time, 1);
        assert_eq!(result.config.threads, vec![1, 2, 3]);
        assert_eq!(result.bench_results.len(), 3);
    }
}

#[test]
#[cfg(feature = "perf")]
#[cfg_attr(miri, ignore)]
fn simple_perf() {
    let config = test_config::Foo::load().expect("Failed to parse config!");
    let repeat = 1;

    let c = config.first().unwrap();
    let benchmark = TestBench::default();
    let result = shumai::run(&benchmark, c, repeat);
    let bench_results = result.bench_results;
    assert_eq!(bench_results.len(), c.threads.len());
    for rv in bench_results {
        let perf = rv.perf.expect("perf should be non-empty");
        // These counters should all be positive
        assert!(perf.cycles > 0);
        assert!(perf.inst > 0);
        assert!(perf.branch_miss > 0);
        assert!(perf.branches > 0);
        assert!(perf.cache_reference > 0);
        assert!(perf.cache_miss > 0);
    }
}
