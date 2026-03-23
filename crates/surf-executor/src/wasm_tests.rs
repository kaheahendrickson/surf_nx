#![cfg(target_arch = "wasm32")]

use crate::{exec_parallel, exec_scope_with_results};

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_worker);

#[wasm_bindgen_test::wasm_bindgen_test]
async fn test_exec_parallel_basic() {
    let numbers = vec![1, 2, 3, 4, 5];
    let squared: Vec<i32> = exec_parallel(&numbers, |n| n * n);
    assert_eq!(squared, vec![1, 4, 9, 16, 25]);
}

#[wasm_bindgen_test::wasm_bindgen_test]
async fn test_exec_parallel_empty() {
    let empty: Vec<i32> = vec![];
    let result: Vec<i32> = exec_parallel(&empty, |n| n * 2);
    assert!(result.is_empty());
}

#[wasm_bindgen_test::wasm_bindgen_test]
async fn test_exec_parallel_large_dataset() {
    let numbers: Vec<i32> = (0..1000).collect();
    let doubled: Vec<i32> = exec_parallel(&numbers, |n| n * 2);

    for (i, &value) in doubled.iter().enumerate() {
        assert_eq!(value, (i as i32) * 2);
    }
}

#[wasm_bindgen_test::wasm_bindgen_test]
async fn test_exec_scope_with_results() {
    let results: Vec<i32> = exec_scope_with_results(vec![|| 1 + 1, || 2 + 2, || 3 + 3]);

    assert_eq!(results, vec![2, 4, 6]);
}

#[wasm_bindgen_test::wasm_bindgen_test]
async fn test_exec_scope_with_results_order_preserved() {
    let results: Vec<String> = exec_scope_with_results(vec![
        || "first".to_string(),
        || "second".to_string(),
        || "third".to_string(),
    ]);

    assert_eq!(
        results,
        vec!["first".to_string(), "second".to_string(), "third".to_string()]
    );
}

#[wasm_bindgen_test::wasm_bindgen_test]
async fn test_parallel_sum() {
    let numbers: Vec<i32> = (1..=100).collect();
    let partial_sums: Vec<i32> = exec_parallel(&numbers, |&n| n);
    let total: i32 = partial_sums.iter().sum();
    assert_eq!(total, 5050);
}

#[wasm_bindgen_test::wasm_bindgen_test]
async fn test_parallel_with_structs() {
    struct Item {
        value: i32,
    }

    let items: Vec<Item> = (0..10).map(|i| Item { value: i }).collect();
    let results: Vec<i32> = exec_parallel(&items, |item| item.value * 10);

    let expected: Vec<i32> = (0..10).map(|i| i * 10).collect();
    assert_eq!(results, expected);
}
