use rust_mirror_select::{Error, PingSelect, SelectStrategy};

use crate::mock::{Behavior, FASTEST, Farm, graded, rotations};

#[tokio::test]
async fn selects_among_the_five_responding_servers() {
    let farm = Farm::start(graded()).await;
    let set = farm.set();

    let best = PingSelect.get_best(&set).await.unwrap();

    assert!(farm.responders().any(|mirror| mirror == best));
}

#[tokio::test]
async fn selects_a_responder_wherever_the_fastest_sits() {
    for behaviors in rotations() {
        let farm = Farm::start(behaviors).await;
        let set = farm.set();

        let best = PingSelect.get_best(&set).await.unwrap();

        assert!(
            farm.responders().any(|mirror| mirror == best),
            "{behaviors:?}"
        );
    }
}

#[tokio::test]
async fn selects_the_only_reachable_server() {
    let mut behaviors = [Behavior::Refuse; 5];
    behaviors[FASTEST] = graded()[FASTEST];
    let farm = Farm::start(behaviors).await;
    let set = farm.set();

    let best = PingSelect.get_best(&set).await.unwrap();

    assert_eq!(best, farm.mirror(FASTEST));
}

#[tokio::test]
async fn skips_servers_that_refuse_connections() {
    let mut behaviors = graded();
    behaviors[FASTEST] = Behavior::Refuse;
    behaviors[0] = Behavior::Refuse;
    let farm = Farm::start(behaviors).await;
    let set = farm.set();

    let best = PingSelect.get_best(&set).await.unwrap();

    assert!(farm.responders().any(|mirror| mirror == best));
    assert_ne!(best, farm.mirror(FASTEST));
    assert_ne!(best, farm.mirror(0));
}

#[tokio::test]
async fn tolerates_servers_that_interrupt_after_connecting() {
    let farm = Farm::start([Behavior::Interrupt; 5]).await;
    let set = farm.set();

    let best = PingSelect.get_best(&set).await.unwrap();

    assert!(farm.responders().any(|mirror| mirror == best));
}

#[tokio::test]
async fn fails_when_every_server_refuses() {
    let farm = Farm::start([Behavior::Refuse; 5]).await;
    let set = farm.set();

    let result = PingSelect.get_best(&set).await;

    assert!(
        matches!(result, Err(Error::NoReachableMirror)),
        "{result:?}"
    );
}
