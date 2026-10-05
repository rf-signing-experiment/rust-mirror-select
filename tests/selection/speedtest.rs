use std::time::Duration;

use rust_mirror_select::consts::SPEEDTEST_SIZE;
use rust_mirror_select::{Error, SelectStrategy, SpeedtestSelect};

use crate::mock::{BODY_LEN, Behavior, FASTEST, Farm, fastest_index, graded, rotations};

fn strategy() -> SpeedtestSelect {
    SpeedtestSelect::with_timeout(Duration::from_secs(1))
}

#[tokio::test]
async fn selects_the_fastest_of_five_servers() {
    let farm = Farm::start(graded()).await;
    let set = farm.set();

    let best = strategy().get_best(&set).await.unwrap();

    assert_eq!(best, farm.mirror(FASTEST));
}

#[tokio::test]
async fn selects_the_fastest_wherever_it_sits() {
    for behaviors in rotations() {
        let farm = Farm::start(behaviors).await;
        let set = farm.set();

        let best = strategy().get_best(&set).await.unwrap();

        assert_eq!(
            best,
            farm.mirror(fastest_index(&behaviors)),
            "{behaviors:?}"
        );
    }
}

#[tokio::test]
async fn skips_servers_that_never_respond() {
    let mut behaviors = graded();
    behaviors[FASTEST] = Behavior::Silent;
    behaviors[0] = Behavior::Refuse;
    let farm = Farm::start(behaviors).await;
    let set = farm.set();

    let best = strategy().get_best(&set).await.unwrap();

    assert_eq!(best, farm.mirror(1));
}

#[tokio::test]
async fn skips_servers_that_interrupt_the_download() {
    let mut behaviors = graded();
    behaviors[FASTEST] = Behavior::Interrupt;
    let farm = Farm::start(behaviors).await;
    let set = farm.set();

    let best = strategy().get_best(&set).await.unwrap();

    assert_eq!(best, farm.mirror(1));
}

#[tokio::test]
async fn fails_when_no_server_completes_a_download() {
    let farm = Farm::start([
        Behavior::Silent,
        Behavior::Interrupt,
        Behavior::Refuse,
        Behavior::Silent,
        Behavior::Interrupt,
    ])
    .await;
    let set = farm.set();

    let result = strategy().get_best(&set).await;

    assert!(
        matches!(result, Err(Error::NoReachableMirror)),
        "{result:?}"
    );
}

#[test]
fn mock_body_matches_the_expected_size() {
    assert_eq!(BODY_LEN as u64, SPEEDTEST_SIZE);
}

#[tokio::test]
async fn skips_an_endless_response() {
    let mut behaviors = graded();
    behaviors[FASTEST] = Behavior::Infinite;
    let farm = Farm::start(behaviors).await;
    let set = farm.set();

    let best = strategy().get_best(&set).await.unwrap();

    assert_eq!(best, farm.mirror(1));
}

#[tokio::test]
async fn rejects_an_endless_response_without_reading_it() {
    let farm = Farm::start([Behavior::Infinite]).await;
    let set = farm.set();

    let started = std::time::Instant::now();
    let result = SpeedtestSelect::with_timeout(Duration::from_secs(10))
        .get_best(&set)
        .await;

    assert!(
        matches!(result, Err(Error::NoReachableMirror)),
        "{result:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "{:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn rejects_a_file_of_the_wrong_size() {
    let mut behaviors = graded();
    behaviors[FASTEST] = Behavior::Undersized;
    let farm = Farm::start(behaviors).await;
    let set = farm.set();

    let best = strategy().get_best(&set).await.unwrap();

    assert_eq!(best, farm.mirror(1));
}

#[tokio::test]
async fn fails_when_every_server_misbehaves() {
    let farm = Farm::start([
        Behavior::Infinite,
        Behavior::Undersized,
        Behavior::Interrupt,
        Behavior::Silent,
        Behavior::Refuse,
    ])
    .await;
    let set = farm.set();

    let result = strategy().get_best(&set).await;

    assert!(
        matches!(result, Err(Error::NoReachableMirror)),
        "{result:?}"
    );
}
