use shumai::{bench_config, BenchContext, MultiThreadBench};

#[bench_config]
pub mod test_config {
    use serde::Serialize;
    use shumai::ShumaiConfig;

    #[derive(ShumaiConfig, Serialize, Clone, Debug)]
    pub struct Foo {
        pub name: String,
        pub threads: Vec<u64>,
        pub time: usize,
        #[matrix]
        pub a: usize,
    }
}

struct TestBench {}

impl MultiThreadBench for TestBench {
    type Result = usize;
    type Config = Foo;

    fn load(&self) {}

    fn run(&self, context: BenchContext<Foo>) -> Self::Result {
        context.wait_for_start();
        0
    }

    fn cleanup(&self) {}
}

#[test]
fn config() {
    let config = test_config::Foo::from_config(std::path::Path::new("tests/benchmark.toml"))
        .expect("Failed to parse config!");

    assert_eq!(config.len(), 2);
    for (i, c) in config.iter().enumerate() {
        assert_eq!(c.threads, vec![1, 2, 3]);
        assert_eq!(c.time, 5);
        assert_eq!(c.a, i);
    }
}

#[test]
fn runner() {
    let config = test_config::Foo::from_config(std::path::Path::new("tests/benchmark.toml"))
        .expect("Failed to parse config!");
    for c in config.iter() {
        let result = shumai::run(&TestBench {}, c, 3);
    }
}
