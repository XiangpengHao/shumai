# Shumai

Shumai is a rust benchmark framework that empowers efficient and correct multi-thread benchmarks.

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
	let config = Foo::from_config(Path::new("benchmark.toml"))
        .expect("Failed to parse config!");
    let repeat = 3;

    for c in config.iter() {
        let benchmark = TestBench::default();
        let result = shumai::run(&benchmark, c, repeat);
		result.to_json() // save results to a json file
	}
}

```

With the above setup, Shumai will write the benchmark results to json files to allow easy data integration:
```json
{
  "config": {
    "name": "foo-foo-2",
    "threads": [
      1,
      2,
      3
    ],
    "time": 1,
    "a": 2
  },
  "thread_cnt": 1,
  "env": {
    "os_release": "5.10.60.1-microsoft-standard-WSL2",
    "rustc_version": "1.58.0",
    "hostname": "DESKTOP-DPOIAG6",
    "cpu_num": 16,
    "cpu_speed": 2894
  },
  "pcm": null,
  "results": [
    259603732,
    264783286,
    263255806
  ]
}
```

### Notable features
- The `flamegraph` feature generates the flamegraph of the benchmark function (instead of the whole program) with zero config.

- The `pcm` feature collects `pcm` related data, such as l3 cache hit/miss, memory bandwidth (including DRAM and PM), UPI bandwidth etc. It requires a pcm-server running on the target host.


