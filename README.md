# Shumai

Shumai is a rust benchmark framework that helps to benchmark multi-thread code correctly and efficiently.

Shumai focus on reproducibility, easy-to-analyze and easy-to-use.

### Example

```toml
# benchmark.toml

[[foo]]
name = "foo"
threads = [1, 2, 3]
time = 1
a = [1, 2]
```

```rust
// bench_config.rs

#[derive(ShumaiConfig, Serialize, Clone, Debug)]
pub struct Foo {
	pub name: String,
	pub threads: Vec<u64>,
	pub time: usize,
	#[matrix]
	pub a: usize,
}

impl MultiThreadBench for TestBench {
    type Result = usize;
    type Config = Foo;

    fn load(&self) {}

    fn run(&self, context: BenchContext<Foo>) -> Self::Result {
		// Barrier to ensure all threads start at the same time
        context.wait_for_start(); 

		// start benchmark
    }

    fn cleanup(&self) {}
}


fn main() {
	let config = test_config::Foo::from_config(std::path::Path::new("tests/benchmark.toml"))
        .expect("Failed to parse config!");
    let repeat = 3;

    for c in config.iter() {
        let benchmark = TestBench::default();
        let result = shumai::run(&benchmark, c, repeat);
		result.to_json() // save results to a json file
	}
}



```

