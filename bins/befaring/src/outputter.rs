use std::sync::Arc;

use console::Style;
use flume::Receiver;

use crate::asserter::AssertResult;
use crate::asserter::TestResult;

pub struct OutPutter;

impl OutPutter {
    pub async fn start(
        rx: Receiver<(String, String, String, Arc<[AssertResult]>)>,
        test_path: &str,
        n_tests: usize,
    ) {
        let style = Style::new().bold().cyan();
        let open_text = &format!("Running test file: {test_path} Found {n_tests} test groups");
        let open_text = style.apply_to(open_text);

        println!("{open_text}");
        let mut failed_tests: Vec<(String, String, String, AssertResult)> = vec![];
        let mut passed_count = 0;
        let mut failed_count = 0;
        while let Ok((name, path, method, result)) = rx.recv_async().await {
            for r in result.iter() {
                let test_type = r.expected.to_string();
                let test_type_aligned = format!("{:<12}", test_type);
                match r.status {
                    TestResult::Pass => {
                        passed_count += 1;
                        println!(
                            "{} {}  [ {test_type_aligned} ] {name} {} {path}",
                            console::style("PASS!").green().bold(),
                            console::style("✔").green().bold(),
                            console::style(method.clone()).bold().yellow(),
                        )
                    }
                    TestResult::Fail => {
                        failed_count += 1;
                        failed_tests.push((name.clone(), method.clone(), path.clone(), r.clone()));
                        println!(
                            "{} {}  [ {test_type_aligned} ] {name} {} {path}",
                            console::style("FAIL!").red().bold(),
                            console::style("✖").red().bold(),
                            console::style(method.clone()).bold().yellow(),
                        )
                    }
                }
            }
        }

        if !failed_tests.is_empty() {
            println!();
            println!(
                "{}",
                console::style("Summary of Failed Tests:").bold().red()
            );
            for (idx, result) in failed_tests.iter().enumerate() {
                println!(
                    "\n{} {} {} {} {}",
                    idx + 1,
                    result.0,
                    console::style(result.1.clone()).yellow().bold(),
                    result.2,
                    result.3
                );
            }
        }

        println!();
        println!(
            "{}",
            console::style(format!(
                "[ Test summary ] {}, {}",
                console::style(format!("passed: {passed_count} ✔"))
                    .bold()
                    .green(),
                console::style(format!("failed: {failed_count} ✖"))
                    .bold()
                    .red(),
            ))
            .cyan()
        );

        if failed_count == 0 {
            println!("{}", console::style("All tests passed! 🎉").bold().green());
        }
    }
}
