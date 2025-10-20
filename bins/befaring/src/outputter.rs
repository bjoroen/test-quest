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

        while let Ok((name, path, method, result)) = rx.recv_async().await {
            for r in result.iter() {
                let test_type = r.expected.to_string();
                let test_type_aligned = format!("{:<12}", test_type);
                match r.status {
                    TestResult::Pass => {
                        println!(
                            "[ {test_type_aligned} ] {}  {name} {} {path} {}",
                            console::style("✔").green().bold(),
                            console::style(method.clone()).bold().yellow(),
                            console::style("PASS!").green().bold(),
                        )
                    }
                    TestResult::Fail => {
                        failed_tests.push((name.clone(), method.clone(), path.clone(), r.clone()));
                        println!(
                            "[ {test_type_aligned} ] {}  {name} {method} {path} {}",
                            console::style("✖").red().bold(),
                            console::style("FAILED!").red().bold(),
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
                    result.1,
                    result.2,
                    result.3
                );
            }
        } else {
            println!();
            println!("{}", console::style("All tests passed! 🎉").bold().green());
        }
    }
}
