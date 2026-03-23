use rayon::prelude::*;
use std::sync::Arc;

pub fn exec_parallel<T, R, F>(items: &[T], f: F) -> Vec<R>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> R + Sync + Send,
{
    items.par_iter().map(f).collect()
}

pub fn exec_scope(closures: Vec<Box<dyn FnOnce() + Send>>) {
    rayon::scope(|s| {
        for closure in closures {
            s.spawn(move |_| closure());
        }
    });
}

pub fn exec_scope_with_results<T, F>(closures: Vec<F>) -> Vec<T>
where
    T: Send,
    F: FnOnce() -> T + Send,
{
    let results: Arc<std::sync::Mutex<Vec<(usize, T)>>> =
        Arc::new(std::sync::Mutex::new(Vec::with_capacity(closures.len())));

    rayon::scope(|s| {
        for (index, closure) in closures.into_iter().enumerate() {
            let results = Arc::clone(&results);
            s.spawn(move |_| {
                let result = closure();
                results.lock().unwrap().push((index, result));
            });
        }
    });

    let mut indexed_results = Arc::try_unwrap(results)
        .unwrap_or_else(|_| panic!("All references should be dropped"))
        .into_inner()
        .unwrap_or_else(|_| panic!("Lock should not be poisoned"));

    indexed_results.sort_by_key(|(index, _)| *index);
    indexed_results
        .into_iter()
        .map(|(_, result)| result)
        .collect()
}
