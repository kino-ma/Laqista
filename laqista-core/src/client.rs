use std::{ops::AsyncFnMut, time::Duration};

const SLEEP_DURATION: Duration = Duration::from_millis(1000);

#[cfg(feature = "tokio")]
pub async fn retry<F, T, E>(mut func: F) -> Result<T, E>
where
    F: AsyncFnMut() -> Result<T, E>,
{
    let mut last_err = None;

    for _ in 0..3 {
        match func().await {
            Ok(out) => return Ok(out),
            Err(e) => {
                last_err = Some(e);
            }
        }

        tokio::time::sleep(SLEEP_DURATION).await;
    }

    Err(last_err.unwrap())
}
