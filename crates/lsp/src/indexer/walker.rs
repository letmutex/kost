use crossbeam_channel::Sender;
use std::path::PathBuf;

pub fn spawn_directory_walker(
    root: PathBuf,
    threads: usize,
    tx: Sender<PathBuf>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let walker = ignore::WalkBuilder::new(&root)
            .standard_filters(true)
            .parents(false)
            .threads(threads)
            .build_parallel();
        walker.run(|| {
            let tx = tx.clone();
            Box::new(move |entry| {
                if let Ok(entry) = entry {
                    let path = entry.into_path();
                    if path.is_file() && path.extension().is_some_and(|e| e == "kt" || e == "kts") {
                        let _ = tx.send(path);
                    }
                }
                ignore::WalkState::Continue
            })
        });
    })
}
