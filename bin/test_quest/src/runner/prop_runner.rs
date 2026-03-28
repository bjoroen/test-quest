use std::sync::Arc;

use flume::Sender;

use crate::runner::RunnerError;
use crate::runner::test_runner::RunnerResult;
use crate::setup::database::any_db::AnyDbPool;
use crate::validator::ValidatedTests;
use crate::validator::prop_tests::PropTests;

pub async fn run_prop_tests(
    tests: Vec<PropTests>,
    tx: Sender<RunnerResult>,
    pool: Arc<AnyDbPool>,
) -> Result<(), RunnerError> {
    for mut test in tests {
        let steps = test
            .steps
            .iter_mut()
            .map(|step| {
                if step.url.as_str().contains("${") {}

                todo!()
            })
            .collect::<Vec<ValidatedTests>>();
    }

    todo!()
}
