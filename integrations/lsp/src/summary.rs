use crate::runner::TestResult;

pub fn print_test_summary(results: &[TestResult]) -> bool {
    println!("\n{:=<110}", "");
    println!(
        "{:<20} {:<16} {:<32} {:>10}   {:<10}",
        "Project", "Feature", "Test Name", "Duration", "Status"
    );
    println!("{:-<110}", "");

    let mut passed_count = 0;
    let mut failed_count = 0;
    let mut failures = Vec::new();

    for result in results {
        let duration_ms = format!("{:.2} ms", result.duration.as_secs_f64() * 1000.0);
        let status_str = match &result.outcome {
            Ok(_) => {
                passed_count += 1;
                "PASSED"
            }
            Err(err) => {
                failed_count += 1;
                failures.push((result, err));
                "FAILED"
            }
        };

        println!(
            "{:<20} {:<16} {:<32} {:>10}   {:<10}",
            result.project_name,
            result.feature.directory_name(),
            result.test_name,
            duration_ms,
            status_str
        );
    }

    println!("{:=<110}", "");
    println!(
        "Total: {} | Passed: {} | Failed: {}\n",
        results.len(),
        passed_count,
        failed_count
    );

    if !failures.is_empty() {
        print_failure_details(&failures);
        false
    } else {
        true
    }
}

fn print_failure_details(failures: &[(&TestResult, &String)]) {
    println!("FAILURES:");
    for (result, error_msg) in failures {
        println!(
            "--------------------------------------------------------------------------------"
        );
        println!(
            "[{}] {} > {} (fixture: {})",
            result.project_name,
            result.feature.directory_name(),
            result.test_name,
            result.fixture_file
        );
        println!("{error_msg}");
    }
    println!("--------------------------------------------------------------------------------\n");
}
