//! Module to handle easy sending functions to tokio
//!
//! The provides two functions one function run_in_tokio creates and sends the function to tokio.
//! The second function run_in_tokio_with_cancel allows the creation of a CancellationToken which can be used to shut down the tokio async.

use crate::error::MyError;
use futures::Future;
use tracing::{error, info};

use serde::Deserialize;
use tokio::runtime::{self, Runtime};
use tokio_util::sync::CancellationToken;

#[derive(Deserialize, Debug, Clone)]
pub struct ThreadRuntime {
    pub threads: usize,
    pub stack_size: usize,
    pub name: String,
}

impl Default for ThreadRuntime {
    fn default() -> Self {
        ThreadRuntime {
            threads: 0,
            stack_size: 3_000_000,
            name: "default".into(),
        }
    }
}

pub fn rt_multithreaded(runtime: &ThreadRuntime) -> Result<Runtime, MyError> {
    if runtime.threads == 0 {
        runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .map_err(MyError::from)
    } else {
        runtime::Builder::new_multi_thread()
            .worker_threads(runtime.threads)
            .thread_name(runtime.name.clone())
            .thread_stack_size(runtime.stack_size)
            .enable_io()
            .enable_time()
            .build()
            .map_err(MyError::from)
    }
}

/// run async function inside tokio instance on current thread
pub fn run_in_tokio<F, T>(runtime: &ThreadRuntime, my_function: F) -> F::Output
where
    F: Future<Output = Result<T, MyError>>,
{
    info!("Starting Tokio: {}", runtime.name);

    rt_multithreaded(runtime)?
        // .expect("Runtime created")
        .block_on(my_function)
}

/// Run async with cancellability via CancellationToken
pub fn run_in_tokio_with_cancel<F, T>(
    runtime: &ThreadRuntime,
    cancel: CancellationToken,
    my_function: F,
) -> F::Output
where
    F: Future<Output = Result<T, MyError>>,
{
    run_in_tokio(runtime, async {
        tokio::select! {
            _ = cancel.cancelled() => {
                error!("Token cancelled");
                Err(MyError::Cancelled)
            },
            z = my_function => {
                eprintln!("Completed function");
                z
            },
        }
    })
}
