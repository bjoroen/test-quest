use std::sync::Arc;

use console::Style;
use flume::Receiver;

use crate::asserter::AssertResult;
use crate::asserter::TestResult;

pub struct OutPutter;

impl OutPutter {
    pub async fn start(
        rx: Receiver<(String, Arc<[AssertResult]>)>,
        test_path: &str,
        n_tests: usize,
    ) {
        let style = Style::new().bold().cyan();
        let open_text =
            &format!("Running test file: {test_path} Found {n_tests} tests: Running...");
        let open_text = style.apply_to(open_text);

        println!("{open_text}");
        let mut i = 1;
        let mut failed_tests: Vec<(String, AssertResult)> = vec![];
        while let Ok((name, result)) = rx.recv_async().await {
            for r in result.iter() {
                match r.status {
                    TestResult::Pass => {
                        println!(
                            "[{i}/{n_tests}] {}  {name}: {} {}",
                            console::style("✔").green().bold(),
                            r.actual,
                            console::style("PASS!").green().bold(),
                        )
                    }
                    TestResult::Fail => {
                        failed_tests.push((name.clone(), r.clone()));
                        println!(
                            "[{i}/{n_tests}] {}  {name}: {} {}",
                            console::style("╳").red().bold(),
                            r.expected,
                            console::style("FAILED!").red().bold(),
                        )
                    }
                }
            }

            i += 1;
        }

        if !failed_tests.is_empty() {
            println!();
            println!(
                "{}",
                console::style("Summary of Failed Tests:").bold().red()
            );
            for (idx, result) in failed_tests.iter().enumerate() {
                println!("\n{} {}. {}", idx + 1, result.0, result.1);
            }
        } else {
            println!();
            println!("{}", console::style("All tests passed! 🎉").bold().green());
        }
    }
}
