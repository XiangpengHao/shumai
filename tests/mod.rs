use serde_json::Value;
use shumai::{bench_config, BenchContext, MultiThreadBench, ShumaiResult};

#[bench_config]
pub mod test_config {
    use serde::{Deserialize, Serialize};
    use shumai::ShumaiConfig;

    #[derive(ShumaiConfig, Serialize, Deserialize, Clone, Debug)]
    pub struct Foo {
        pub name: String,
        pub threads: Vec<usize>,
        pub time: usize,
        #[matrix]
        pub a: usize,
    }
}

#[derive(Debug, PartialEq)]
enum ExecutionSeq {
    Load,
    Run,
    Cleanup,
}

#[derive(Default)]
struct TestBench {
    execution_queue: crossbeam::queue::SegQueue<ExecutionSeq>,
}

impl MultiThreadBench for TestBench {
    type Result = usize;
    type Config = Foo;

    fn load(&self) -> Option<Value> {
        self.execution_queue.push(ExecutionSeq::Load);
        None
    }

    fn run(&self, context: BenchContext<Foo>) -> Self::Result {
        context.wait_for_start();
        let mut sum = 0;
        while context.is_running() {
            sum += context.config.a;
        }
        self.execution_queue.push(ExecutionSeq::Run);
        sum
    }

    fn cleanup(&self) {
        self.execution_queue.push(ExecutionSeq::Cleanup);
    }
}

#[test]
fn config() {
    let config = test_config::Foo::from_config(std::path::Path::new("tests/benchmark.toml"))
        .expect("Failed to parse config!");

    assert_eq!(config.len(), 2);
    for (i, c) in config.iter().enumerate() {
        assert_eq!(c.threads, vec![1, 2, 3]);
        assert_eq!(c.time, 1);
        assert_eq!(c.a, i + 1);
    }
}

#[test]
fn runner() {
    let config = test_config::Foo::from_config(std::path::Path::new("tests/benchmark.toml"))
        .expect("Failed to parse config!");
    let repeat = 3;

    for c in config.iter() {
        let benchmark = TestBench::default();
        let _result = shumai::run(&benchmark, c, repeat);

        let mut gt = Vec::new();
        gt.push(ExecutionSeq::Load);
        for thread in c.threads.iter() {
            for _i in 0..*thread {
                for _j in 0..repeat {
                    gt.push(ExecutionSeq::Run);
                }
            }
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
fn write_json() {
    let config = test_config::Foo::from_config(std::path::Path::new("tests/benchmark.toml"))
        .expect("Failed to parse config!");
    let repeat = 1;

    for c in config.iter() {
        let benchmark = TestBench::default();
        let result = shumai::run(&benchmark, c, repeat);
        let file_path = result.write_json().unwrap();

        let written_data = std::fs::read_to_string(file_path).unwrap();
        let result: ShumaiResult<Foo, usize> = serde_json::from_str(&written_data).unwrap();
        assert_eq!(result.config.time, 1);
        assert_eq!(result.config.threads, vec![1, 2, 3]);
        assert_eq!(result.bench_results.len(), 3);
    }
}
