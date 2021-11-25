use shumai::bench_config;

#[bench_config]
mod test_config {
    use serde::Serialize;
    use shumai::ShumaiConfig;

    #[derive(ShumaiConfig, Serialize, Clone, Debug)]
    pub struct Foo {
        name: String,
        threads: Vec<u64>,
        time: usize,
        #[matrix]
        a: usize,
    }
}

fn main() {}
