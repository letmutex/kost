use dep::progress::{ProgressCoordinator, ProgressEvent, ProgressHandle, ResolveProgress};
use dep::{DependencyCoordinate, Downloader, GradleCache, RepositoryRegistry};
use smol_str::SmolStr;
use std::time::Duration;
use tokio::sync::mpsc::unbounded_channel;

#[tokio::test]
async fn progress_tracker_and_events() {
    let (tx, mut rx) = unbounded_channel();
    let handle = ProgressHandle::new(tx);
    let mut tracker = handle.create_task("test-artifact", Some(1000));
    assert_eq!(tracker.label(), "test-artifact");
    assert!(!tracker.is_started());

    tracker.start();
    assert!(tracker.is_started());

    let first = rx.recv().await.unwrap();
    match first {
        ProgressEvent::Started {
            id,
            label,
            total_bytes,
        } => {
            assert_eq!(id, 1);
            assert_eq!(label.as_str(), "test-artifact");
            assert_eq!(total_bytes, Some(1000));
        }
        _ => panic!("Expected Started event"),
    }

    tracker.update(500);
    let second = rx.recv().await.unwrap();
    match second {
        ProgressEvent::Chunk { id, bytes } => {
            assert_eq!(id, 1);
            assert_eq!(bytes, 500);
        }
        _ => panic!("Expected Chunk event"),
    }

    tracker.finish();
    let third = rx.recv().await.unwrap();
    match third {
        ProgressEvent::Finished {
            id,
            label,
            total_bytes,
            ..
        } => {
            assert_eq!(id, 1);
            assert_eq!(label.as_str(), "test-artifact");
            assert_eq!(total_bytes, 500);
        }
        _ => panic!("Expected Finished event"),
    }
}

#[tokio::test]
async fn progress_tracker_reset_on_retry() {
    let (tx, mut rx) = unbounded_channel();
    let handle = ProgressHandle::new(tx);
    let mut tracker = handle.create_task("retry-artifact", None);

    tracker.update(250); // triggers lazy start
    let _ = rx.recv().await.unwrap(); // Started
    let _ = rx.recv().await.unwrap(); // Chunk

    tracker.reset();
    let reset_event = rx.recv().await.unwrap();
    match reset_event {
        ProgressEvent::Reset { id } => assert_eq!(id, 1),
        _ => panic!("Expected Reset event"),
    }

    tracker.fail("Network failure");
    let fail_event = rx.recv().await.unwrap();
    match fail_event {
        ProgressEvent::Failed { id, label, error } => {
            assert_eq!(id, 1);
            assert_eq!(label.as_str(), "retry-artifact");
            assert_eq!(error.as_str(), "Network failure");
        }
        _ => panic!("Expected Failed event"),
    }
}

#[tokio::test]
async fn task_tracker_drop_safety_emits_failed_event() {
    let (tx, mut rx) = unbounded_channel();
    let handle = ProgressHandle::new(tx);
    {
        let _tracker = handle.create_task("cancelled-artifact", None);
        // dropped here without calling finish() or fail()
    }

    let first = rx.recv().await.unwrap();
    match first {
        ProgressEvent::Failed { id, label, error } => {
            assert_eq!(id, 1);
            assert_eq!(label.as_str(), "cancelled-artifact");
            assert_eq!(error.as_str(), "Download aborted");
        }
        _ => panic!("Expected Failed event from Drop"),
    }
}

#[tokio::test]
async fn task_tracker_lazy_start_keeps_pending_hidden() {
    let (tx, mut rx) = unbounded_channel();
    let handle = ProgressHandle::new(tx);
    let tracker = handle.create_task("pending-task", None);

    assert!(!tracker.is_started());
    assert!(rx.try_recv().is_err());
}

#[test]
fn coordinator_multi_task_progress() {
    let mut coord = ProgressCoordinator::new(3, false, false);
    coord.on_started(1, SmolStr::new("dep-a"), Some(2048));
    coord.on_started(2, SmolStr::new("dep-b"), Some(4096));
    coord.on_chunk(1, 1024);
    coord.on_chunk(2, 2048);

    assert_eq!(coord.rendered_lines, 0); // non-tty does not render dynamic lines
    coord.on_finished(1, SmolStr::new("dep-a"), 2048, Duration::from_millis(150));
    assert_eq!(coord.completed_count, 1);
    assert_eq!(coord.active_tasks_count(), 1);
}

#[test]
fn coordinator_renders_active_and_overall_lines() {
    let mut coord = ProgressCoordinator::new(10, true, false);
    coord.on_started(1, SmolStr::new("dep-a"), Some(1000));
    coord.on_started(2, SmolStr::new("dep-b"), None);
    coord.on_chunk(1, 500);

    coord.render_active_lines();
    // 2 active lines + 1 overall progress line = 3 rendered lines
    assert_eq!(coord.rendered_lines, 3);

    coord.on_finished(1, SmolStr::new("dep-a"), 1000, Duration::from_millis(100));
    // 1 active line remaining + 1 overall progress line = 2 rendered lines
    coord.render_active_lines();
    assert_eq!(coord.rendered_lines, 2);
    assert_eq!(coord.completed_count, 1);
}

#[test]
fn coordinator_caps_rendered_active_tasks_to_head() {
    let mut coord = ProgressCoordinator::new(20, true, false);
    for id in 1..=12 {
        let name = common::concat_smol_str!("dep-", &id.to_string());
        coord.on_started(id, name, None);
    }
    coord.render_active_lines();
    // Up to 8 head active lines + 1 overall progress line = 9 rendered lines
    assert_eq!(coord.rendered_lines, 9);

    // Finishing task 1 (one of the 8 head tasks) shifts task 9 into the head window
    coord.on_finished(1, SmolStr::new("dep-1"), 500, Duration::from_millis(50));
    coord.render_active_lines();
    // 11 active tasks remaining -> capped to 8 head + 1 overall = 9 lines
    assert_eq!(coord.rendered_lines, 9);
}

#[test]
fn downloader_concurrency_clamping_and_fallback() {
    assert_eq!(Downloader::DEFAULT_CONCURRENCY, 32);
    assert_eq!(Downloader::MAX_CONCURRENCY, 256);

    assert_eq!(Downloader::resolve_concurrency(Some("16")), 16);
    assert_eq!(Downloader::resolve_concurrency(Some("0")), 1);
    assert_eq!(Downloader::resolve_concurrency(Some("1000")), 256);
    assert_eq!(Downloader::resolve_concurrency(Some("invalid")), 32);
    assert_eq!(Downloader::resolve_concurrency(Some("  64  ")), 64);
    assert_eq!(Downloader::resolve_concurrency(None), 32);

    let bounded = Downloader::new(1000);
    assert_eq!(bounded.concurrency(), 256);

    let min_bounded = Downloader::new(0);
    assert_eq!(min_bounded.concurrency(), 1);
}

#[tokio::test]
async fn downloader_concurrency_limits_simultaneous_starts() {
    let temp_dir = std::env::temp_dir().join("kost_test_downloader_concurrency_limits");
    let _ = std::fs::remove_dir_all(&temp_dir);

    let downloader = Downloader::new(1);
    let cache = GradleCache::new(Some(&temp_dir)).unwrap();
    let registry = RepositoryRegistry::new();

    let (tx, mut rx) = unbounded_channel();
    let handle = ProgressHandle::new(tx);

    let coord1 = DependencyCoordinate::parse("test:a:1.0").unwrap();
    let coord2 = DependencyCoordinate::parse("test:b:1.0").unwrap();

    let tracker1 = handle.create_task("test:a:1.0", None);
    let tracker2 = handle.create_task("test:b:1.0", None);

    let dl = downloader.clone();
    let reg1 = registry.clone();
    let c1 = cache.clone();

    let task1 = tokio::spawn(async move {
        let _ = dl
            .download_artifact_with_tracker(&reg1, &coord1, false, &c1, Some(tracker1))
            .await;
    });

    let dl2 = downloader.clone();
    let reg2 = registry.clone();
    let c2 = cache.clone();
    let task2 = tokio::spawn(async move {
        let _ = dl2
            .download_artifact_with_tracker(&reg2, &coord2, false, &c2, Some(tracker2))
            .await;
    });

    let _ = tokio::join!(task1, task2);

    let mut started_count = 0;
    while let Ok(event) = rx.try_recv() {
        if matches!(event, ProgressEvent::Started { .. }) {
            started_count += 1;
        }
    }
    assert_eq!(started_count, 2);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn solve_with_progress_clears_on_completion() {
    let cache = GradleCache::new(None).unwrap();
    let downloader = Downloader::new(2);
    let registry = RepositoryRegistry::new();

    let d1 = DependencyCoordinate::parse("androidx.core:core:1.9.0").unwrap();
    let d2 = DependencyCoordinate::parse("androidx.core:core:1.12.0").unwrap();

    let mut progress = ResolveProgress::new(true).with_throttle(0);
    let graph = dep::SolvedGraph::solve_with_progress(
        &[d1, d2],
        &registry,
        &cache,
        &downloader,
        true,
        Some(&mut progress),
    )
    .await;

    assert_eq!(graph.direct_count, 1);
    assert!(!progress.is_active());
    assert_eq!(progress.last_len(), 0);
}
