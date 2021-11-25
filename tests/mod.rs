use shumai::bench_config;

#[bench_config]
mod test_config {
    use serde::Serialize;
    use shumai::ShumaiConfig;

    pub trait ConfigImpl: Clone + Serialize {
        fn name(&self) -> &String;
        fn thread(&self) -> &[u64];
        fn bench_sec(&self) -> usize;
    }

    #[derive(ShumaiConfig, Serialize, Clone)]
    pub struct Foo {
        name: String,
        threads: Vec<u64>,
        time: usize,
        #[matrix]
        a: usize,
    }
}

#[test]
fn smoke() {
    println!("ok");
}
