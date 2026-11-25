use crate::{Entity, Subscription, TestAppContext, TestDispatcher};
use futures::StreamExt as _;
use rand::prelude::*;
use smol::channel;
use std::{
    env,
    panic::{self, RefUnwindSafe},
    pin::Pin,
};

pub fn run_test(
    num_iterations: usize,
    explicit_seeds: &[u64],
    max_retries: usize,
    test_fn: &mut (dyn RefUnwindSafe + Fn(TestDispatcher, u64)),
    on_fail_fn: Option<fn()>,
) {
    let (seeds, is_multiple_runs) = calculate_seeds(num_iterations as u64, explicit_seeds);

    for seed in seeds {
        let mut attempt = 0;
        loop {
            if is_multiple_runs {
                eprintln!("seed = {seed}");
            }
            let result = panic::catch_unwind(|| {
                let dispatcher = TestDispatcher::new(StdRng::seed_from_u64(seed));
                test_fn(dispatcher, seed);
            });

            match result {
                Ok(_) => break,
                Err(error) => {
                    if attempt < max_retries {
                        println!("attempt {} failed, retrying", attempt);
                        attempt += 1;
                        std::mem::forget(error);
                    } else {
                        if is_multiple_runs {
                            eprintln!("failing seed: {}", seed);
                        }
                        if let Some(on_fail_fn) = on_fail_fn {
                            on_fail_fn()
                        }
                        panic::resume_unwind(error);
                    }
                }
            }
        }
    }
}

fn calculate_seeds(
    iterations: u64,
    explicit_seeds: &[u64],
) -> (impl Iterator<Item = u64> + '_, bool) {
    let iterations = env::var("ITERATIONS")
        .ok()
        .map(|var| var.parse().expect("invalid ITERATIONS variable"))
        .unwrap_or(iterations);

    let env_num = env::var("SEED")
        .map(|seed| seed.parse().expect("invalid SEED variable as integer"))
        .ok();

    let empty_range = || 0..0;

    let iter = {
        let env_range = if let Some(env_num) = env_num {
            env_num..env_num + 1
        } else {
            empty_range()
        };

        let iterations_range = match (iterations, env_num) {
            (1, None) if explicit_seeds.is_empty() => 0..1,
            (1, None) | (1, Some(_)) => empty_range(),
            (iterations, Some(env)) => env..env + iterations,
            (iterations, None) => 0..iterations,
        };

        let explicit_seeds = if env_num.is_some() {
            &[]
        } else {
            explicit_seeds
        };

        env_range
            .chain(iterations_range)
            .chain(explicit_seeds.iter().copied())
    };
    let is_multiple_runs = iter.clone().nth(1).is_some();
    (iter, is_multiple_runs)
}

pub struct Observation<T> {
    rx: Pin<Box<channel::Receiver<T>>>,
    _subscription: Subscription,
}

impl<T: 'static> futures::Stream for Observation<T> {
    type Item = T;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.rx.poll_next_unpin(cx)
    }
}

pub fn observe<T: 'static>(entity: &Entity<T>, cx: &mut TestAppContext) -> Observation<()> {
    let (tx, rx) = smol::channel::unbounded();
    let _subscription = cx.update(|cx| {
        cx.observe(entity, move |_, _| {
            let _ = smol::block_on(tx.send(()));
        })
    });
    let rx = Box::pin(rx);

    Observation { rx, _subscription }
}
