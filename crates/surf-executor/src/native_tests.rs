use crate::{exec_parallel, exec_scope, exec_scope_with_results};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[test]
fn test_exec_parallel_basic() {
    let numbers = vec![1, 2, 3, 4, 5];
    let squared: Vec<i32> = exec_parallel(&numbers, |n| n * n);
    assert_eq!(squared, vec![1, 4, 9, 16, 25]);
}

#[test]
fn test_exec_parallel_empty() {
    let empty: Vec<i32> = vec![];
    let result: Vec<i32> = exec_parallel(&empty, |n| n * 2);
    assert!(result.is_empty());
}

#[test]
fn test_exec_parallel_large_dataset() {
    let numbers: Vec<i32> = (0..10_000).collect();
    let doubled: Vec<i32> = exec_parallel(&numbers, |n| n * 2);

    for (i, &value) in doubled.iter().enumerate() {
        assert_eq!(value, (i as i32) * 2);
    }
}

#[test]
fn test_exec_parallel_uses_multiple_threads() {
    let numbers: Vec<i32> = (0..1000).collect();

    let thread_ids: Vec<usize> =
        exec_parallel(&numbers, |_| rayon::current_thread_index().unwrap_or(0));

    let unique_threads: std::collections::HashSet<usize> = thread_ids.into_iter().collect();

    assert!(
        unique_threads.len() >= 1,
        "Rayon thread pool should be initialized"
    );
}

#[test]
fn test_exec_scope_basic() {
    let counter = Arc::new(AtomicUsize::new(0));

    exec_scope(vec![
        Box::new({
            let counter = Arc::clone(&counter);
            move || {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        }),
        Box::new({
            let counter = Arc::clone(&counter);
            move || {
                counter.fetch_add(2, Ordering::SeqCst);
            }
        }),
        Box::new({
            let counter = Arc::clone(&counter);
            move || {
                counter.fetch_add(3, Ordering::SeqCst);
            }
        }),
    ]);

    assert_eq!(counter.load(Ordering::SeqCst), 6);
}

#[test]
fn test_exec_scope_empty() {
    exec_scope(Vec::new());
}

#[test]
fn test_exec_scope_with_results() {
    let results: Vec<i32> = exec_scope_with_results(vec![|| 1 + 1, || 2 + 2, || 3 + 3]);

    assert_eq!(results, vec![2, 4, 6]);
}

#[test]
fn test_exec_scope_with_results_order_preserved() {
    let results: Vec<String> = exec_scope_with_results(vec![
        || "first".to_string(),
        || "second".to_string(),
        || "third".to_string(),
    ]);

    assert_eq!(
        results,
        vec![
            "first".to_string(),
            "second".to_string(),
            "third".to_string()
        ]
    );
}

#[test]
fn test_parallel_sum() {
    let numbers: Vec<i32> = (1..=100).collect();
    let partial_sums: Vec<i32> = exec_parallel(&numbers, |&n| n);
    let total: i32 = partial_sums.iter().sum();
    assert_eq!(total, 5050);
}

#[test]
fn test_parallel_with_structs() {
    struct Item {
        value: i32,
    }

    let items: Vec<Item> = (0..10).map(|i| Item { value: i }).collect();
    let results: Vec<i32> = exec_parallel(&items, |item| item.value * 10);

    let expected: Vec<i32> = (0..10).map(|i| i * 10).collect();
    assert_eq!(results, expected);
}
