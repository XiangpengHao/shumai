use shumai::bench_config;

#[bench_config]
mod test_config {
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

#[test]
fn basic() {
    let config = test_config::Foo::from_config(std::path::Path::new("tests/benchmark.toml"))
        .expect("Failed to parse config!");

    assert_eq!(config.len(), 2);
    for (i, c) in config.iter().enumerate() {
        assert_eq!(c.threads, vec![1, 2, 3]);
        assert_eq!(c.time, 5);
        assert_eq!(c.a, i);
    }
}
