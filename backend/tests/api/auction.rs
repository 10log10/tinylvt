// use crate::helpers::spawn_app;
use backend::time;
use jiff::Span;

#[tokio::test]
async fn test_time_mock() -> anyhow::Result<()> {
    let initial_time = time::now();

    time::advance_mock_time(Span::new().hours(1));
    assert_eq!(time::now(), initial_time + Span::new().hours(1));

    let new_time = initial_time + Span::new().hours(7);
    time::set_mock_time(new_time);
    assert_eq!(time::now(), new_time);

    Ok(())
}
