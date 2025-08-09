use vibe_golf::prelude::*;
use bevy::prelude::*;

// Helper to build a minimal app (no assets/scene) for deterministic fixed tick tests.
fn build_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(CoreSimPlugin); // provides tick_state system in FixedUpdate
    app
}

#[test]
fn ticks_advance() {
    let mut app = build_app();
    // Directly run FixedUpdate schedule 5 times (bypasses time driver).
    for _ in 0..5 { app.world_mut().run_schedule(FixedUpdate); }
    let sim = app.world().get_resource::<SimState>().unwrap();
    assert_eq!(sim.tick, 5, "expected tick to be 5 after 5 fixed steps");
    assert!((sim.elapsed_seconds - (5.0/60.0)).abs() < 1e-6);
}

#[test]
fn autoplay_resource_present() {
    let app = build_app();
    assert!(app.world().get_resource::<AutoConfig>().is_some());
}
